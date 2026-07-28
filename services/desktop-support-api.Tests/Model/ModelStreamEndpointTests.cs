using System.Net;
using System.Net.Http.Headers;
using System.Security.Claims;
using System.Text;
using System.Text.Encodings.Web;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Signing;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-029's route half. <c>POST /desktop/v1/model-access/streams</c> is the one
/// fixed streaming surface: it is never renderer-supplied, it carries the
/// model-invocation posture rather than the read posture, and it refuses every
/// request it cannot fully authorize.
/// </summary>
public sealed class ModelStreamEndpointTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string OtherClientId = "33333333-3333-3333-3333-333333333333";
    private const string SubjectA = "44444444-4444-4444-4444-444444444444";
    private const string Route = "/desktop/v1/model-access/streams";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task An_unauthenticated_request_is_challenged()
    {
        await using StreamFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await client.PostAsync(
            Route,
            EmptyBody(),
            Ct);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task The_read_scope_alone_cannot_spend_tokens()
    {
        await using StreamFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            DesktopScopes.Access);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
    }

    [Fact]
    public async Task A_request_from_an_unapproved_desktop_client_is_refused()
    {
        await using StreamFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            DesktopScopes.ModelInvoke,
            clientId: OtherClientId);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
    }

    [Fact]
    public async Task An_unverifiable_consent_envelope_fails_closed()
    {
        // The default version two consent verifier reports unavailable, and a
        // model-egress route must never read that as verified.
        await using StreamFactory factory = new(verifyConsent: false);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            DesktopScopes.ModelInvoke);

        Assert.Equal(HttpStatusCode.ServiceUnavailable, response.StatusCode);
        string body = await response.Content.ReadAsStringAsync(Ct);
        Assert.Contains("consent_binding_unavailable", body, StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_verified_request_streams_normalized_events_and_one_terminal_event()
    {
        await using StreamFactory factory = new(verifyConsent: true);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            DesktopScopes.ModelInvoke);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Equal(
            "text/event-stream",
            response.Content.Headers.ContentType?.MediaType);
        string body = await response.Content.ReadAsStringAsync(Ct);
        Assert.Contains("event: response_started", body, StringComparison.Ordinal);
        Assert.Contains("event: output_text_delta", body, StringComparison.Ordinal);
        Assert.Contains("event: completed", body, StringComparison.Ordinal);
        Assert.Equal(1, CountOccurrences(body, "event: completed"));
        // No deployment name, endpoint, or credential reaches the desktop.
        Assert.DoesNotContain("desktop-planning", body, StringComparison.Ordinal);
        Assert.DoesNotContain("forge-fake.test", body, StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.CredentialCanary,
            body,
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_malformed_request_body_is_refused_before_any_egress()
    {
        await using StreamFactory factory = new(verifyConsent: true);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            DesktopScopes.ModelInvoke,
            body: """{"schemaVersion":"desktop-model-access-request.v2"}""");

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Empty(factory.Transport.Requests);
    }

    private static int CountOccurrences(string text, string value)
    {
        int count = 0;
        int index = 0;
        while ((index = text.IndexOf(value, index, StringComparison.Ordinal)) >= 0)
        {
            count++;
            index += value.Length;
        }
        return count;
    }

    private static StringContent EmptyBody() =>
        new("{}", Encoding.UTF8, "application/json");

    private static async Task<HttpResponseMessage> SendAsync(
        HttpClient client,
        string scope,
        string clientId = ApprovedClientId,
        string? body = null)
    {
        using HttpRequestMessage request = new(HttpMethod.Post, Route)
        {
            Content = new StringContent(
                body ?? StreamRequestJson,
                Encoding.UTF8,
                "application/json"),
        };
        request.Headers.Authorization = new AuthenticationHeaderValue(
            StreamAuthenticationHandler.AuthenticationSchemeName,
            $"{SubjectA}|{scope}|{clientId}");
        return await client.SendAsync(
            request,
            HttpCompletionOption.ResponseContentRead,
            TestContext.Current.CancellationToken);
    }

    /// <summary>
    /// A canonical version two streaming request. It is written literally
    /// rather than serialized so the test proves the wire shape the endpoint
    /// admits, not the shape this assembly happens to produce.
    /// </summary>
    private static readonly string StreamRequestJson = BuildRequestJson();

    private static string BuildRequestJson()
    {
        const string content = "review this bounded context";
        string contentHash = "sha256:" + Convert.ToHexStringLower(
            System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(content)));
        string hash = FoundryResponsesTestFixtures.Hash;
        string other = FoundryResponsesTestFixtures.OtherHash;
        return $$"""
        {
          "schemaVersion": "desktop-model-access-request.v2",
          "requestId": "request_01J00000000000000000000000",
          "deliveryModel": "windows_local",
          "registrationId": "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
          "purpose": "bmad_help",
          "modelRole": "planner",
          "requestMode": "streaming_chat",
          "modelProfileId": "coding-balanced",
          "requestedVariantId": "balanced",
          "catalogVersion": "v1.0123456789abcdef",
          "modelProfileHash": "{{hash}}",
          "modelCapabilityHash": "{{other}}",
          "contextSnapshotId": "snap_AAAAAAAAAAAAAAAA",
          "canonicalOutputSchemaId": null,
          "canonicalOutputSchemaHash": null,
          "localEgressManifestHash": "{{hash}}",
          "consent": {
            "schemaVersion": "sapphirus.model-context-consent.v2",
            "decisionId": "decision_01J00000000000000000000000",
            "requestId": "request_01J00000000000000000000000",
            "invocationId": "invoke_01J00000000000000000000000",
            "deliveryModel": "windows_local",
            "tenantHash": "{{hash}}",
            "subjectHash": "{{other}}",
            "registrationId": "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            "installationPublicKeyHash": "{{hash}}",
            "entitlementLeaseId": "lease_01J00000000000000000000000",
            "entitlementLeaseHash": "{{hash}}",
            "tenantPolicyId": "policy_01J00000000000000000000000",
            "tenantPolicyVersion": 7,
            "tenantPolicyHash": "{{hash}}",
            "purpose": "bmad_help",
            "modelRole": "planner",
            "requestMode": "streaming_chat",
            "modelProfileId": "coding-balanced",
            "requestedVariantId": "balanced",
            "catalogVersion": "v1.0123456789abcdef",
            "contextSnapshotId": "snap_AAAAAAAAAAAAAAAA",
            "canonicalOutputSchemaId": null,
            "canonicalOutputSchemaHash": null,
            "manifestHash": "{{hash}}",
            "invocationBindingHash": "{{hash}}",
            "consumptionHash": "{{hash}}",
            "consentDisclosureHash": "{{hash}}",
            "providerProfileHash": "{{hash}}",
            "modelProfileHash": "{{hash}}",
            "modelCapabilityHash": "{{other}}",
            "region": "westeurope",
            "retentionMode": "transient_no_store",
            "budgetClass": "interactive-standard",
            "issuedAt": "2026-07-28T10:00:00.000Z",
            "notBefore": "2026-07-28T10:00:00.000Z",
            "expiresAt": "2026-07-28T10:05:00.000Z",
            "nonceHash": "{{hash}}",
            "consentEnvelopeHash": "{{hash}}",
            "proof": {
              "proofType": "installation_signature",
              "algorithm": "ES256",
              "keyId": "{{hash}}",
              "signedPayloadHash": "{{hash}}",
              "signature": "ZXhhbXBsZS1zaWduYXR1cmU"
            }
          },
          "items": [
            {
              "clientItemId": "context-item-1",
              "relativeLabel": "src/example.cs",
              "semanticRole": "implementation",
              "language": "csharp",
              "contentHash": "{{contentHash}}",
              "byteCount": {{Encoding.UTF8.GetByteCount(content)}},
              "classification": "internal",
              "content": "{{content}}"
            }
          ],
          "tools": [],
          "retentionMode": "transient_no_store",
          "budgetClass": "interactive-standard"
        }
        """;
    }

    private sealed class StreamFactory(bool verifyConsent = false)
        : WebApplicationFactory<Program>
    {
        public FakeTransport Transport { get; } = new();

        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Development");
            builder.ConfigureLogging(logging => logging.ClearProviders());
            builder.UseSetting(
                "Sapphirus:Authority",
                $"https://login.microsoftonline.com/{TenantId}/v2.0");
            builder.UseSetting(
                "Sapphirus:Audience",
                "api://66666666-6666-6666-6666-666666666666");
            builder.UseSetting("Sapphirus:ApprovedDesktopClientId", ApprovedClientId);
            builder.UseSetting("Sapphirus:Region", "westeurope");
            builder.UseSetting("Sapphirus:ReleaseChannel", "beta");
            builder.ConfigureTestServices(services =>
            {
                services.AddAuthentication(authentication =>
                    {
                        authentication.DefaultAuthenticateScheme =
                            StreamAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultChallengeScheme =
                            StreamAuthenticationHandler.AuthenticationSchemeName;
                        authentication.DefaultForbidScheme =
                            StreamAuthenticationHandler.AuthenticationSchemeName;
                    })
                    .AddScheme<AuthenticationSchemeOptions, StreamAuthenticationHandler>(
                        StreamAuthenticationHandler.AuthenticationSchemeName,
                        _ => { });

                // Only the four seams outside this task are replaced. The
                // endpoint, the admission guard, the normalizer, the
                // coordinator, and the receipt signer are the real ones.
                services.RemoveAll<IDeviceRegistry>();
                services.AddSingleton<IDeviceRegistry>(new AlwaysActiveRegistry());
                services.RemoveAll<IModelStreamConsentVerifier>();
                services.AddSingleton<IModelStreamConsentVerifier>(
                    new FixedConsentVerifier(verifyConsent));
                services.AddSingleton<IModelStreamRouteResolver>(new FixedRouteResolver());
                services.AddSingleton(_ => new ModelStreamingCoordinator(
                    Transport,
                    new AzureModelReceiptSigner(
                        new StubHashSigner(),
                        new SupportPlaneOptions
                        {
                            Authority = "https://login.microsoftonline.com/"
                                + $"{TenantId}/v2.0",
                            Audience = "api://66666666-6666-6666-6666-666666666666",
                            ApprovedDesktopClientId = ApprovedClientId,
                            Region = "westeurope",
                        }),
                    TimeProvider.System,
                    ModelStreamLimits.Default,
                    FoundryCredentialMechanisms.ManagedIdentity));
            });
        }

        public HttpClient CreateSecureClient() => CreateClient(
            new WebApplicationFactoryClientOptions
            {
                AllowAutoRedirect = false,
                BaseAddress = new Uri("https://localhost"),
            });
    }

    internal sealed class FakeTransport : IModelStreamTransport
    {
        public List<ModelProviderRequest> Requests { get; } = [];

        public Task<FoundryStreamSession> OpenAsync(
            ModelProviderRequest request,
            CancellationToken cancellationToken)
        {
            Requests.Add(request);
            HttpResponseMessage response = new(HttpStatusCode.OK);
            MemoryStream body = new(Encoding.UTF8.GetBytes(
                FoundryResponsesTestFixtures.ResponseScenarioBody("streaming_text_deltas")));
            return Task.FromResult(new FoundryStreamSession(response, body));
        }
    }

    private sealed class FixedRouteResolver : IModelStreamRouteResolver
    {
        public string DeploymentInventoryHash => FoundryResponsesTestFixtures.OtherHash;

        public ValueTask<ModelRouteResolution> ResolveAsync(
            string subject,
            ModelAccessRequestV2 request,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(
                new ModelRouteResolution(FoundryResponsesTestFixtures.Route(), ""));
    }

    private sealed class FixedConsentVerifier(bool verified) : IModelStreamConsentVerifier
    {
        public ValueTask<ContextConsentVerification> VerifyAsync(
            string subject,
            RegisteredDevice device,
            ModelAccessRequestV2 request,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(verified
                ? ContextConsentVerification.Verified
                : ContextConsentVerification.Unavailable);
    }

    private sealed class AlwaysActiveRegistry : IDeviceRegistry
    {
        public Task<DeviceRegistrationResponse> RegisterAsync(
            string subject,
            DeviceRegistrationRequest request,
            CancellationToken cancellationToken) =>
            throw new NotSupportedException();

        public Task<ActiveDeviceRegistration?> FindActiveAsync(
            string subject,
            string registrationId,
            CancellationToken cancellationToken) =>
            Task.FromResult<ActiveDeviceRegistration?>(new ActiveDeviceRegistration(
                new RegisteredDevice(
                    subject,
                    registrationId,
                    "public-key",
                    FoundryResponsesTestFixtures.Hash,
                    "0.1.0",
                    "windows",
                    "x64",
                    7,
                    DateTimeOffset.UnixEpoch,
                    DeviceRegistrationState.Active,
                    null),
                1,
                Guid.Empty,
                CancellationToken.None));

        public Task<SignedEntitlementLease> CommitLeaseIfActiveAsync(
            ActiveDeviceRegistration operationLease,
            SignedEntitlementLease lease,
            CancellationToken cancellationToken) =>
            throw new NotSupportedException();

        public Task<ModelAccessResult> CommitModelResultIfActiveAsync(
            ActiveDeviceRegistration operationLease,
            ModelAccessRequest request,
            ModelAccessResult result,
            string expectedRegion,
            CancellationToken cancellationToken) =>
            throw new NotSupportedException();

        public Task<ModelAccessReceipt?> GetReceiptAsync(
            string subject,
            string receiptId,
            CancellationToken cancellationToken) =>
            Task.FromResult<ModelAccessReceipt?>(null);

        public Task<DeviceRevocationOutcome> RevokeAsync(
            string subject,
            string registrationId,
            CancellationToken cancellationToken) =>
            Task.FromResult(DeviceRevocationOutcome.Unknown);
    }

    private sealed class StubHashSigner : IHashSigner
    {
        public string KeyId => "model-receipt-key-2026-07";

        public Task<string> SignAsync(byte[] sha256Digest, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return Task.FromResult(
                Convert.ToBase64String(sha256Digest)
                    .Replace('+', '-')
                    .Replace('/', '_')
                    .TrimEnd('='));
        }
    }

    internal sealed class StreamAuthenticationHandler(
        IOptionsMonitor<AuthenticationSchemeOptions> options,
        ILoggerFactory logger,
        UrlEncoder encoder) : AuthenticationHandler<AuthenticationSchemeOptions>(
            options,
            logger,
            encoder)
    {
        internal const string AuthenticationSchemeName = "StreamTest";

        protected override Task<AuthenticateResult> HandleAuthenticateAsync()
        {
            AuthenticationHeaderValue? header = Request.Headers.Authorization.Count == 1
                && AuthenticationHeaderValue.TryParse(
                    Request.Headers.Authorization[0],
                    out AuthenticationHeaderValue? parsed)
                ? parsed
                : null;
            if (header is null
                || !string.Equals(
                    header.Scheme,
                    AuthenticationSchemeName,
                    StringComparison.Ordinal)
                || header.Parameter is null)
            {
                return Task.FromResult(AuthenticateResult.NoResult());
            }
            string[] parts = header.Parameter.Split('|');
            if (parts.Length != 3)
            {
                return Task.FromResult(AuthenticateResult.NoResult());
            }
            ClaimsIdentity identity = new(AuthenticationSchemeName);
            identity.AddClaim(new Claim("tid", TenantId));
            identity.AddClaim(new Claim("oid", parts[0]));
            identity.AddClaim(new Claim("scp", parts[1]));
            identity.AddClaim(new Claim("azp", parts[2]));
            return Task.FromResult(AuthenticateResult.Success(new AuthenticationTicket(
                new ClaimsPrincipal(identity),
                AuthenticationSchemeName)));
        }
    }
}
