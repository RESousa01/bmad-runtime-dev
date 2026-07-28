using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Signing;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// A streaming run is single use, and it is single use in the same order the
/// structured path already enforces: the idempotency key is reserved and the
/// consent is consumed <em>before</em> anything is sent to the provider, and
/// nothing that happens afterwards gives either back.
///
/// <para>
/// Every case here counts provider calls rather than inspecting internal state,
/// because the property that matters is not "a row was written" but "the model
/// was not asked twice".
/// </para>
/// </summary>
public sealed class ModelStreamSingleUseTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string SubjectA = "44444444-4444-4444-4444-444444444444";
    private const string Route = "/desktop/v1/model-access/streams";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_verified_stream_consumes_its_consent_exactly_once()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(client, RequestJson());

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Contains(
            "event: completed",
            await response.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
        Assert.Equal(1, factory.Consumption.ConsumeCount);
    }

    [Fact]
    public async Task A_retried_stream_under_the_same_key_makes_no_second_provider_call()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        string body = RequestJson();

        using HttpResponseMessage first = await SendAsync(client, body);
        using HttpResponseMessage retry = await SendAsync(client, body);

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, retry.StatusCode);
        Assert.Contains(
            "model_call_already_completed",
            await retry.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
        Assert.Equal(1, factory.Consumption.ConsumeCount);
    }

    /// <summary>
    /// The retry answer carries the completion marker and nothing else — no
    /// transcript, which was never stored, and no receipt body.
    /// </summary>
    [Fact]
    public async Task A_replayed_stream_exposes_only_the_completion_marker()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        string body = RequestJson();

        using HttpResponseMessage first = await SendAsync(client, body);
        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        using HttpResponseMessage retry = await SendAsync(client, body);
        string payload = await retry.Content.ReadAsStringAsync(Ct);

        Assert.Contains("receiptId", payload, StringComparison.Ordinal);
        Assert.Contains("resultHash", payload, StringComparison.Ordinal);
        Assert.DoesNotContain("proof", payload, StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.CredentialCanary,
            payload,
            StringComparison.Ordinal);
    }

    /// <summary>
    /// A second decision cannot re-spend one consumption authority. The two
    /// requests carry different identifiers, so they reserve different keys and
    /// the reservation cannot be what refuses the second one — the consumed
    /// consent is.
    /// </summary>
    [Fact]
    public async Task A_second_request_cannot_re_spend_one_consumption_authority()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage first = await SendAsync(client, RequestJson());
        using HttpResponseMessage second = await SendAsync(
            client,
            RequestJson(requestId: "request_01J0000000000000000000000B"));

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, second.StatusCode);
        Assert.Contains(
            "consent_already_consumed",
            await second.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
    }

    [Theory]
    [InlineData(TransportBehaviour.Cancels)]
    [InlineData(TransportBehaviour.Fails)]
    public async Task A_stream_that_never_completed_does_not_free_its_consumed_consent(
        TransportBehaviour behaviour)
    {
        await using SingleUseFactory factory = new(behaviour);
        using HttpClient client = factory.CreateSecureClient();
        string body = RequestJson();

        using HttpResponseMessage first = await SendAsync(client, body);
        using HttpResponseMessage retry = await SendAsync(client, body);

        // The run reached the wire and ended without a receipt, so its consent
        // is spent and its key is terminally uncertain.
        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, retry.StatusCode);
        Assert.Contains(
            SupportProblemCodes.ModelCallUncertain,
            await retry.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
        Assert.Equal(1, factory.Consumption.ConsumeCount);
    }

    /// <summary>
    /// The reservation happens before egress, so a key already held by a
    /// different request is refused without the provider being touched at all.
    /// </summary>
    [Fact]
    public async Task A_key_held_by_a_different_request_conflicts_before_any_egress()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage first = await SendAsync(client, RequestJson());
        using HttpResponseMessage conflicting = await SendAsync(
            client,
            RequestJson(content: "a different bounded context"));

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, conflicting.StatusCode);
        Assert.Contains(
            "idempotency_conflict",
            await conflicting.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
    }

    /// <summary>
    /// An explicit header is the same authority by another name, and it wins
    /// over the derived key.
    /// </summary>
    [Fact]
    public async Task An_explicit_idempotency_key_header_is_honoured()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        string body = RequestJson();
        const string key = "stream-idempotency-key-0001";

        using HttpResponseMessage first = await SendAsync(client, body, key);
        using HttpResponseMessage retry = await SendAsync(client, body, key);

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.Conflict, retry.StatusCode);
        Assert.Contains(
            "model_call_already_completed",
            await retry.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(1, factory.Transport.OpenCount);
    }

    /// <summary>
    /// A plane that cannot record consumption cannot spend consent either. It
    /// refuses before egress rather than streaming an unaccounted run.
    /// </summary>
    [Fact]
    public async Task A_plane_that_cannot_consume_consent_refuses_before_any_egress()
    {
        await using SingleUseFactory factory = new(consentConsumptionAvailable: false);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(client, RequestJson());

        Assert.Equal(HttpStatusCode.ServiceUnavailable, response.StatusCode);
        Assert.Contains(
            "consent_consumption_unavailable",
            await response.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(0, factory.Transport.OpenCount);
    }

    /// <summary>
    /// The single-use authority is not optional. A composition without it
    /// refuses the route outright rather than streaming unreserved.
    /// </summary>
    [Fact]
    public async Task A_plane_without_a_reservation_store_refuses_the_route()
    {
        await using SingleUseFactory factory = new(idempotencyComposed: false);
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(client, RequestJson());

        Assert.Equal(HttpStatusCode.ServiceUnavailable, response.StatusCode);
        Assert.Contains(
            SupportProblemCodes.FoundryProjectUnavailable,
            await response.Content.ReadAsStringAsync(Ct),
            StringComparison.Ordinal);
        Assert.Equal(0, factory.Transport.OpenCount);
    }

    /// <summary>
    /// A request refused by the admission guard never reserves a key and never
    /// consumes consent, so a corrected retry is still able to run.
    /// </summary>
    [Fact]
    public async Task A_rejected_request_spends_nothing()
    {
        await using SingleUseFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage rejected = await SendAsync(
            client,
            """{"schemaVersion":"desktop-model-access-request.v2"}""");
        using HttpResponseMessage corrected = await SendAsync(client, RequestJson());

        Assert.Equal(HttpStatusCode.BadRequest, rejected.StatusCode);
        Assert.Equal(HttpStatusCode.OK, corrected.StatusCode);
        Assert.Equal(1, factory.Transport.OpenCount);
        Assert.Equal(1, factory.Consumption.ConsumeCount);
    }

    /// <summary>
    /// The Development reservation store's retention is deliberately asymmetric.
    /// A completed marker is a convenience and may age out; an uncertain claim
    /// is the record that consent was spent, and letting it age out would hand
    /// the key back over consent that is gone.
    /// </summary>
    [Fact]
    public async Task An_uncertain_claim_never_ages_out_of_the_development_store()
    {
        MutableTimeProvider clock = new(DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"));
        MemoryModelStreamIdempotencyStore store = new(16, TimeSpan.FromMinutes(15), clock);
        await store.ReserveAsync("subject-a", "uncertain-key", "fingerprint", Ct);

        clock.Advance(TimeSpan.FromHours(6));
        ModelStreamReservation retry = await store.ReserveAsync(
            "subject-a",
            "uncertain-key",
            "fingerprint",
            Ct);

        Assert.Equal(ModelStreamReservationState.Uncertain, retry.State);
    }

    [Fact]
    public async Task A_completed_marker_ages_out_of_the_development_store()
    {
        MutableTimeProvider clock = new(DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"));
        MemoryModelStreamIdempotencyStore store = new(16, TimeSpan.FromMinutes(15), clock);
        await store.ReserveAsync("subject-a", "completed-key", "fingerprint", Ct);
        await store.CompleteAsync(
            "subject-a",
            "completed-key",
            new ModelCallCompletionMarker(
                "receipt_01J00000000000000000000000",
                FoundryResponsesTestFixtures.Hash,
                FoundryResponsesTestFixtures.OtherHash),
            Ct);

        clock.Advance(TimeSpan.FromMinutes(16));
        ModelStreamReservation reissued = await store.ReserveAsync(
            "subject-a",
            "completed-key",
            "fingerprint",
            Ct);

        Assert.Equal(ModelStreamReservationState.Reserved, reissued.State);
    }

    /// <summary>
    /// A flood of abandoned runs becomes a capacity refusal, never a fresh
    /// reservation over spent consent.
    /// </summary>
    [Fact]
    public async Task The_development_store_refuses_rather_than_evicting_uncertain_claims()
    {
        MutableTimeProvider clock = new(DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"));
        MemoryModelStreamIdempotencyStore store = new(2, TimeSpan.FromMinutes(15), clock);
        await store.ReserveAsync("subject-a", "key-one", "fingerprint", Ct);
        await store.ReserveAsync("subject-a", "key-two", "fingerprint", Ct);

        clock.Advance(TimeSpan.FromHours(6));

        await Assert.ThrowsAsync<IdempotencyCapacityException>(() =>
            store.ReserveAsync("subject-a", "key-three", "fingerprint", Ct));
    }

    private sealed class MutableTimeProvider(DateTimeOffset now) : TimeProvider
    {
        private DateTimeOffset _now = now;

        public override DateTimeOffset GetUtcNow() => _now;

        public void Advance(TimeSpan amount) => _now += amount;
    }

    private static async Task<HttpResponseMessage> SendAsync(
        HttpClient client,
        string body,
        string? idempotencyKey = null)
    {
        using HttpRequestMessage request = new(HttpMethod.Post, Route)
        {
            Content = new StringContent(body, Encoding.UTF8, "application/json"),
        };
        request.Headers.Authorization = new AuthenticationHeaderValue(
            ModelStreamEndpointTests.StreamAuthenticationHandler.AuthenticationSchemeName,
            $"{SubjectA}|{DesktopScopes.ModelInvoke}|{ApprovedClientId}");
        if (idempotencyKey is not null)
        {
            request.Headers.Add("Idempotency-Key", idempotencyKey);
        }
        return await client.SendAsync(
            request,
            HttpCompletionOption.ResponseContentRead,
            TestContext.Current.CancellationToken);
    }

    /// <summary>
    /// One canonical version two streaming request. The consumption hash is
    /// fixed across variations on purpose: two different requests that name the
    /// same consumption authority must not both reach the provider.
    /// </summary>
    private static string RequestJson(
        string requestId = "request_01J00000000000000000000000",
        string content = "review this bounded context")
    {
        string contentHash = "sha256:" + Convert.ToHexStringLower(
            SHA256.HashData(Encoding.UTF8.GetBytes(content)));
        string hash = FoundryResponsesTestFixtures.Hash;
        string other = FoundryResponsesTestFixtures.OtherHash;
        return $$"""
        {
          "schemaVersion": "desktop-model-access-request.v2",
          "requestId": "{{requestId}}",
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
            "requestId": "{{requestId}}",
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
            "expiresAt": "2126-07-28T10:05:00.000Z",
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

    public enum TransportBehaviour
    {
        Streams,
        Cancels,
        Fails,
    }

    private sealed class SingleUseFactory(
        TransportBehaviour behaviour = TransportBehaviour.Streams,
        bool consentConsumptionAvailable = true,
        bool idempotencyComposed = true) : WebApplicationFactory<Program>
    {
        public CountingTransport Transport { get; } = new(behaviour);

        public CountingConsumptionStore Consumption { get; } =
            new(consentConsumptionAvailable);

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
                            ModelStreamEndpointTests.StreamAuthenticationHandler
                                .AuthenticationSchemeName;
                        authentication.DefaultChallengeScheme =
                            ModelStreamEndpointTests.StreamAuthenticationHandler
                                .AuthenticationSchemeName;
                        authentication.DefaultForbidScheme =
                            ModelStreamEndpointTests.StreamAuthenticationHandler
                                .AuthenticationSchemeName;
                    })
                    .AddScheme<AuthenticationSchemeOptions,
                        ModelStreamEndpointTests.StreamAuthenticationHandler>(
                        ModelStreamEndpointTests.StreamAuthenticationHandler
                            .AuthenticationSchemeName,
                        _ => { });

                // The registry, the consent signature, the route, and the
                // provider are the four seams outside this task. The endpoint,
                // the reservation store, the consumption ordering, the
                // coordinator, and the receipt signer are all the real ones.
                services.RemoveAll<IDeviceRegistry>();
                services.AddSingleton<IDeviceRegistry>(new AlwaysActiveRegistry());
                services.RemoveAll<IModelStreamConsentVerifier>();
                services.AddSingleton<IModelStreamConsentVerifier>(new AlwaysVerified());
                services.RemoveAll<IContextConsentConsumptionStore>();
                services.AddSingleton<IContextConsentConsumptionStore>(Consumption);
                if (!idempotencyComposed)
                {
                    services.RemoveAll<IModelStreamIdempotencyStore>();
                }
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

    internal sealed class CountingTransport(TransportBehaviour behaviour)
        : IModelStreamTransport
    {
        private int _openCount;

        public int OpenCount => Volatile.Read(ref _openCount);

        public Task<FoundryStreamSession> OpenAsync(
            ModelProviderRequest request,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref _openCount);
            switch (behaviour)
            {
                case TransportBehaviour.Cancels:
                    // The desktop hung up, or the operation deadline elapsed,
                    // after the call was already funded.
                    throw new OperationCanceledException();
                case TransportBehaviour.Fails:
                    throw new ModelProviderException(
                        ModelProviderOutcomes.QuotaExhausted,
                        retryable: false);
                default:
                    HttpResponseMessage response = new(HttpStatusCode.OK);
                    MemoryStream body = new(Encoding.UTF8.GetBytes(
                        FoundryResponsesTestFixtures.ResponseScenarioBody(
                            "streaming_text_deltas")));
                    return Task.FromResult(new FoundryStreamSession(response, body));
            }
        }
    }

    internal sealed class CountingConsumptionStore(bool available)
        : IContextConsentConsumptionStore
    {
        private readonly Lock _gate = new();
        private readonly HashSet<string> _consumed = new(StringComparer.Ordinal);
        private int _consumeCount;

        public int ConsumeCount => Volatile.Read(ref _consumeCount);

        public ValueTask<ContextConsentConsumption> ConsumeAsync(
            ContextConsentConsumptionRequest request,
            CancellationToken cancellationToken)
        {
            ArgumentNullException.ThrowIfNull(request);
            cancellationToken.ThrowIfCancellationRequested();
            if (!available)
            {
                return ValueTask.FromResult(ContextConsentConsumption.Unavailable);
            }
            string authority =
                request.SubjectPartition + "\0" + request.ConsumptionHash;
            lock (_gate)
            {
                if (!_consumed.Add(authority))
                {
                    return ValueTask.FromResult(ContextConsentConsumption.AlreadyConsumed);
                }
            }
            Interlocked.Increment(ref _consumeCount);
            return ValueTask.FromResult(ContextConsentConsumption.Consumed);
        }
    }

    private sealed class AlwaysVerified : IModelStreamConsentVerifier
    {
        public ValueTask<ContextConsentVerification> VerifyAsync(
            string subject,
            RegisteredDevice device,
            ModelAccessRequestV2 request,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(ContextConsentVerification.Verified);
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
}
