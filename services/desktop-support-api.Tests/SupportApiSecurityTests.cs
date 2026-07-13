using System.Net;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Options;
using Sapphirus.DesktopSupportApi;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests;

public sealed class SupportApiSecurityTests
{
    private const string TenantId = "11111111-1111-1111-1111-111111111111";
    private const string ApprovedClientId = "22222222-2222-2222-2222-222222222222";
    private const string OtherClientId = "33333333-3333-3333-3333-333333333333";
    private const string SubjectA = "44444444-4444-4444-4444-444444444444";
    private const string SubjectB = "55555555-5555-5555-5555-555555555555";
    private const string ValidHash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    [Fact]
    public async Task Unauthenticated_request_is_challenged_instead_of_failing_rate_partition()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await client.GetAsync("/desktop/v1/bootstrap");

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact]
    public async Task Only_the_configured_desktop_client_is_authorized()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage denied = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/bootstrap",
            SubjectA,
            OtherClientId,
            DesktopScopes.Access);
        using HttpResponseMessage accepted = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/bootstrap",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);

        Assert.Equal(HttpStatusCode.Forbidden, denied.StatusCode);
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);
    }

    [Fact]
    public async Task Registration_is_subject_bound_and_revocation_is_retained()
    {
        await using SupportApiFactory factory = new();
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);

        using HttpResponseMessage activeLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-active-device");
        Assert.Equal(HttpStatusCode.OK, activeLease.StatusCode);

        using HttpResponseMessage crossSubjectLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectB,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-cross-subject");
        Assert.Equal(HttpStatusCode.Forbidden, crossSubjectLease.StatusCode);

        using HttpResponseMessage revoke = await SendAsync(
            client,
            HttpMethod.Delete,
            $"/desktop/v1/devices/registrations/{registration.RegistrationId}",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        Assert.Equal(HttpStatusCode.NoContent, revoke.StatusCode);

        using HttpResponseMessage revokedLease = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/entitlements/leases",
            SubjectA,
            DesktopScopes.Access,
            new LeaseRequest(registration.RegistrationId),
            "idem-revoked-device");
        Assert.Equal(HttpStatusCode.Forbidden, revokedLease.StatusCode);
    }

    [Fact]
    public async Task Model_access_recomputes_content_and_manifest_hashes()
    {
        await using SupportApiFactory factory = new(consentVerified: true);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest valid = CreateModelRequest(registration.RegistrationId, "review this file");

        using HttpResponseMessage accepted = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            valid,
            "idem-model-valid");
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);

        ModelContextItem original = valid.Items[0];
        ModelAccessRequest changedBytes = valid with
        {
            Items = [original with { Content = "review that file" }],
        };
        using HttpResponseMessage contentDrift = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            changedBytes,
            "idem-model-content-drift");
        Assert.Equal(HttpStatusCode.BadRequest, contentDrift.StatusCode);

        ModelAccessRequest manifestDrift = valid with { LocalEgressManifestHash = ValidHash };
        using HttpResponseMessage changedManifest = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            manifestDrift,
            "idem-model-manifest-drift");
        Assert.Equal(HttpStatusCode.BadRequest, changedManifest.StatusCode);
    }

    [Fact]
    public void Context_manifest_hash_matches_the_purpose_separated_golden_vector()
    {
        ModelContextItem item = new(
            "context-item-1",
            "src/example.txt",
            "implementation",
            "text",
            "sha256:5c2747dcba9b399166829e0058228130e5732dbfa9c77a176cbf5bfe8ca4b46e",
            8,
            "source",
            "hello π");
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            "request_12345678",
            "windows_local",
            "dreg_aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "plan_changes",
            "planning",
            "https://schemas.sapphirus.invalid/model-plan.v1",
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ValidHash,
            ValidHash,
            [item],
            "transient_no_store",
            "interactive");

        Assert.Equal(
            "sha256:7cd6071668c47dc530620c59e21e6116ebe4746b7e262039d3f76f43dc06c3ec",
            RequestGuards.ComputeContextManifestHash(request));
    }

    [Fact]
    public async Task Opaque_consent_hash_is_not_treated_as_verified()
    {
        await using SupportApiFactory factory = new(consentVerified: false);
        using HttpClient client = factory.CreateSecureClient();
        DeviceRegistrationResponse registration = await RegisterAsync(client, SubjectA);
        ModelAccessRequest request = CreateModelRequest(registration.RegistrationId, "review this file");

        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/model-access/calls",
            SubjectA,
            DesktopScopes.ModelInvoke,
            request,
            "idem-consent-unavailable");

        Assert.Equal(HttpStatusCode.ServiceUnavailable, response.StatusCode);
    }

    [Fact]
    public async Task Configured_release_channel_is_projected()
    {
        await using SupportApiFactory factory = new(releaseChannel: "stable");
        using HttpClient client = factory.CreateSecureClient();

        using HttpResponseMessage response = await SendAsync(
            client,
            HttpMethod.Get,
            "/desktop/v1/releases/current",
            SubjectA,
            ApprovedClientId,
            DesktopScopes.Access);
        ReleaseResponse? release = await response.Content.ReadFromJsonAsync<ReleaseResponse>();

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.NotNull(release);
        Assert.Equal("stable", release!.Channel);
    }

    [Fact]
    public async Task Failed_idempotent_operation_is_removed_and_can_be_retried()
    {
        MemoryIdempotencyStore store = new(4, TimeSpan.FromMinutes(10), TimeProvider.System);
        int attempts = 0;

        await Assert.ThrowsAsync<InvalidOperationException>(() => store.ExecuteAsync(
            "subject",
            "idem-failure-retry",
            "fingerprint",
            () =>
            {
                attempts++;
                return Task.FromException<int>(new InvalidOperationException("expected"));
            },
            TestContext.Current.CancellationToken));
        int result = await store.ExecuteAsync(
            "subject",
            "idem-failure-retry",
            "fingerprint",
            () =>
            {
                attempts++;
                return Task.FromResult(42);
            },
            TestContext.Current.CancellationToken);

        Assert.Equal(42, result);
        Assert.Equal(2, attempts);
        Assert.Equal(1, store.EntryCount);
    }

    [Fact]
    public async Task Idempotency_capacity_never_evicts_an_in_flight_operation()
    {
        MemoryIdempotencyStore store = new(1, TimeSpan.FromMinutes(10), TimeProvider.System);
        TaskCompletionSource started = new(TaskCreationOptions.RunContinuationsAsynchronously);
        TaskCompletionSource release = new(TaskCreationOptions.RunContinuationsAsynchronously);
        Task<int> first = store.ExecuteAsync(
            "subject",
            "idem-first-in-flight",
            "fingerprint-1",
            async () =>
            {
                started.SetResult();
                await release.Task;
                return 1;
            },
            TestContext.Current.CancellationToken);
        await started.Task;

        await Assert.ThrowsAsync<IdempotencyCapacityException>(() => store.ExecuteAsync(
            "subject",
            "idem-second-in-flight",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));
        Assert.Equal(1, store.EntryCount);

        release.SetResult();
        Assert.Equal(1, await first);
    }

    [Fact]
    public async Task Completed_idempotency_entry_is_evicted_only_after_retention()
    {
        ManualTimeProvider time = new(new DateTimeOffset(2026, 7, 13, 12, 0, 0, TimeSpan.Zero));
        MemoryIdempotencyStore store = new(1, TimeSpan.FromMinutes(10), time);
        Assert.Equal(1, await store.ExecuteAsync(
            "subject",
            "idem-retained",
            "fingerprint-1",
            () => Task.FromResult(1),
            TestContext.Current.CancellationToken));
        await Assert.ThrowsAsync<IdempotencyCapacityException>(() => store.ExecuteAsync(
            "subject",
            "idem-before-expiry",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));

        time.Advance(TimeSpan.FromMinutes(11));
        Assert.Equal(2, await store.ExecuteAsync(
            "subject",
            "idem-after-expiry",
            "fingerprint-2",
            () => Task.FromResult(2),
            TestContext.Current.CancellationToken));
        Assert.Equal(1, store.EntryCount);
    }

    private static async Task<DeviceRegistrationResponse> RegisterAsync(
        HttpClient client,
        string subject)
    {
        DeviceRegistrationRequest request = new(
            Hash("installation-key"),
            "0.1.0-beta.1",
            "windows",
            "x64",
            "development-1");
        using HttpResponseMessage response = await SendJsonAsync(
            client,
            HttpMethod.Post,
            "/desktop/v1/devices/registrations",
            subject,
            DesktopScopes.Access,
            request,
            "idem-register-" + subject[..8]);
        DeviceRegistrationResponse? registration =
            await response.Content.ReadFromJsonAsync<DeviceRegistrationResponse>();
        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.NotNull(registration);
        return registration!;
    }

    private static ModelAccessRequest CreateModelRequest(string registrationId, string content)
    {
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            Hash(content),
            Encoding.UTF8.GetByteCount(content),
            "source",
            content);
        ModelAccessRequest unbound = new(
            "desktop-model-access-request.v1",
            "request_12345678",
            "windows_local",
            registrationId,
            "plan_changes",
            "planning",
            "https://schemas.sapphirus.invalid/model-plan.v1",
            Hash("model-plan-schema"),
            ValidHash,
            Hash("local-consent-receipt"),
            [item],
            "transient_no_store",
            "interactive");
        return unbound with
        {
            LocalEgressManifestHash = RequestGuards.ComputeContextManifestHash(unbound),
        };
    }

    private static Task<HttpResponseMessage> SendJsonAsync<T>(
        HttpClient client,
        HttpMethod method,
        string path,
        string subject,
        string scope,
        T body,
        string idempotencyKey)
    {
        HttpRequestMessage request = CreateRequest(
            method,
            path,
            subject,
            ApprovedClientId,
            scope);
        request.Headers.Add("Idempotency-Key", idempotencyKey);
        request.Content = JsonContent.Create(body);
        return client.SendAsync(request, TestContext.Current.CancellationToken);
    }

    private static Task<HttpResponseMessage> SendAsync(
        HttpClient client,
        HttpMethod method,
        string path,
        string subject,
        string clientId,
        string scope) => client.SendAsync(
            CreateRequest(method, path, subject, clientId, scope),
            TestContext.Current.CancellationToken);

    private static HttpRequestMessage CreateRequest(
        HttpMethod method,
        string path,
        string subject,
        string clientId,
        string scope)
    {
        HttpRequestMessage request = new(method, path);
        request.Headers.Add(TestAuthenticationHandler.TenantHeader, TenantId);
        request.Headers.Add(TestAuthenticationHandler.SubjectHeader, subject);
        request.Headers.Add(TestAuthenticationHandler.ClientHeader, clientId);
        request.Headers.Add(TestAuthenticationHandler.ScopeHeader, scope);
        return request;
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));

    private sealed class SupportApiFactory(
        bool consentVerified = true,
        string releaseChannel = "beta") : WebApplicationFactory<Program>
    {
        protected override void ConfigureWebHost(IWebHostBuilder builder)
        {
            builder.UseEnvironment("Development");
            builder.UseSetting("Sapphirus:Authority", $"https://login.microsoftonline.com/{TenantId}/v2.0");
            builder.UseSetting("Sapphirus:Audience", "api://66666666-6666-6666-6666-666666666666");
            builder.UseSetting("Sapphirus:ApprovedDesktopClientId", ApprovedClientId);
            builder.UseSetting("Sapphirus:Region", "test-region");
            builder.UseSetting("Sapphirus:ReleaseChannel", releaseChannel);
            builder.UseSetting("Sapphirus:DevelopmentSigningEnabled", "true");
            builder.UseSetting("Sapphirus:DevelopmentModelEnabled", "true");
            builder.ConfigureTestServices(services =>
            {
                services.AddAuthentication(authentication =>
                    {
                        authentication.DefaultAuthenticateScheme = TestAuthenticationHandler.Scheme;
                        authentication.DefaultChallengeScheme = TestAuthenticationHandler.Scheme;
                        authentication.DefaultForbidScheme = TestAuthenticationHandler.Scheme;
                    })
                    .AddScheme<AuthenticationSchemeOptions, TestAuthenticationHandler>(
                        TestAuthenticationHandler.Scheme,
                        _ => { });
                if (consentVerified)
                {
                    services.RemoveAll<IContextConsentVerifier>();
                    services.AddSingleton<IContextConsentVerifier, TestContextConsentVerifier>();
                }
            });
        }

        public HttpClient CreateSecureClient() => CreateClient(new WebApplicationFactoryClientOptions
        {
            AllowAutoRedirect = false,
            BaseAddress = new Uri("https://localhost"),
        });
    }

    private sealed class TestContextConsentVerifier : IContextConsentVerifier
    {
        public ValueTask<ContextConsentVerification> VerifyAsync(
            ContextConsentVerificationRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return ValueTask.FromResult(ContextConsentVerification.Verified);
        }
    }

    private sealed class ManualTimeProvider(DateTimeOffset utcNow) : TimeProvider
    {
        private DateTimeOffset _utcNow = utcNow;

        public override DateTimeOffset GetUtcNow() => _utcNow;

        public void Advance(TimeSpan duration) => _utcNow += duration;
    }

    private sealed class TestAuthenticationHandler(
        IOptionsMonitor<AuthenticationSchemeOptions> options,
        ILoggerFactory logger,
        UrlEncoder encoder) : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
    {
        public const string Scheme = "SupportApiTests";
        public const string TenantHeader = "X-Test-Tenant";
        public const string SubjectHeader = "X-Test-Subject";
        public const string ClientHeader = "X-Test-Client";
        public const string ScopeHeader = "X-Test-Scope";

        protected override Task<AuthenticateResult> HandleAuthenticateAsync()
        {
            if (!Request.Headers.TryGetValue(TenantHeader, out var tenant)
                || !Request.Headers.TryGetValue(SubjectHeader, out var subject)
                || !Request.Headers.TryGetValue(ClientHeader, out var client)
                || !Request.Headers.TryGetValue(ScopeHeader, out var scope))
            {
                return Task.FromResult(AuthenticateResult.NoResult());
            }
            Claim[] claims =
            [
                new("tid", tenant.ToString()),
                new("oid", subject.ToString()),
                new("azp", client.ToString()),
                new("scp", scope.ToString()),
            ];
            ClaimsIdentity identity = new(claims, Scheme);
            AuthenticationTicket ticket = new(new ClaimsPrincipal(identity), Scheme);
            return Task.FromResult(AuthenticateResult.Success(ticket));
        }
    }
}
