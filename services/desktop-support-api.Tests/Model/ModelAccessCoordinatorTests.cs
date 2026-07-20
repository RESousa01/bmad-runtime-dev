using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Http;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Sql;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

public sealed class ModelAccessCoordinatorTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string InstallationPublicKey =
        "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEbAS8_dPcjzdYutR-ZVr8kwBsm8PLq3nVCufbv0IrJY_PjRzLuCE1BsBTqhuAddhYYvXJEz8kEs03YhmxFqMgpQ";
    private const string Subject = "subject-a";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task Consent_is_consumed_before_egress_and_not_restored_by_provider_failure()
    {
        Fixture fixture = await Fixture.CreateAsync();
        fixture.Broker.Failure = new ModelAccessFailedException("provider_unavailable");
        fixture.Broker.OnComplete = () =>
            Assert.Equal(1, fixture.Consumption.ConsumeCalls);

        ModelAccessCoordinationResult first = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-consume-order-1",
            Subject,
            Ct);
        Assert.Equal(StatusCodes.Status503ServiceUnavailable, first.StatusCode);
        Assert.Equal(1, fixture.Consumption.ConsumeCalls);

        fixture.Broker.Failure = null;
        ModelAccessCoordinationResult second = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-consume-order-2",
            Subject,
            Ct);
        Assert.Equal(StatusCodes.Status409Conflict, second.StatusCode);
        Assert.Contains("consent_already_consumed", JsonSerializer.Serialize(second.Body));
        Assert.Equal(0, fixture.Broker.Completions);
    }

    [Fact]
    public async Task Revocation_between_admission_and_commit_prevents_receipt_publication()
    {
        Fixture fixture = await Fixture.CreateAsync();
        fixture.Broker.OnComplete = () => fixture.Registry
            .RevokeAsync(Subject, fixture.RegistrationId, CancellationToken.None)
            .GetAwaiter()
            .GetResult();

        ModelAccessCoordinationResult outcome = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-revoked-commit",
            Subject,
            Ct);

        Assert.Equal(StatusCodes.Status403Forbidden, outcome.StatusCode);
        Assert.Null(await fixture.Registry.GetReceiptAsync(
            Subject,
            fixture.Broker.LastReceiptId!,
            Ct));
    }

    [Fact]
    public async Task Terminal_uncertainty_blocks_new_provider_calls_for_the_same_authority()
    {
        Fixture fixture = await Fixture.CreateAsync(
            idempotency: new UncertainIdempotencyStore());

        ModelAccessCoordinationResult outcome = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-uncertain",
            Subject,
            Ct);

        Assert.Equal(StatusCodes.Status409Conflict, outcome.StatusCode);
        Assert.Contains("model_call_uncertain", JsonSerializer.Serialize(outcome.Body));
        Assert.Equal(0, fixture.Broker.Completions);
        Assert.Equal(0, fixture.Consumption.ConsumeCalls);
    }

    [Fact]
    public async Task Concurrent_identical_requests_converge_to_one_provider_call()
    {
        Fixture fixture = await Fixture.CreateAsync();
        fixture.Broker.Delay = TimeSpan.FromMilliseconds(150);

        Task<ModelAccessCoordinationResult>[] calls =
        [
            fixture.Coordinator.ExecuteAsync(fixture.Request, "key-parallel", Subject, Ct),
            fixture.Coordinator.ExecuteAsync(fixture.Request, "key-parallel", Subject, Ct),
        ];
        ModelAccessCoordinationResult[] outcomes = await Task.WhenAll(calls);

        Assert.All(outcomes, outcome =>
            Assert.Equal(StatusCodes.Status200OK, outcome.StatusCode));
        Assert.Equal(1, fixture.Broker.Completions);
        Assert.Equal(1, fixture.Consumption.ConsumeCalls);
    }

    [Fact]
    public async Task Alternate_idempotency_keys_cannot_replay_one_consent()
    {
        Fixture fixture = await Fixture.CreateAsync();

        ModelAccessCoordinationResult first = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-original",
            Subject,
            Ct);
        ModelAccessCoordinationResult second = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-alternate",
            Subject,
            Ct);

        Assert.Equal(StatusCodes.Status200OK, first.StatusCode);
        Assert.Equal(StatusCodes.Status409Conflict, second.StatusCode);
        Assert.Contains("consent_already_consumed", JsonSerializer.Serialize(second.Body));
        Assert.Equal(1, fixture.Broker.Completions);
    }

    [Fact]
    public async Task Safe_replay_exposes_only_the_completion_marker()
    {
        Fixture fixture = await Fixture.CreateAsync();
        ModelAccessCoordinationResult first = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-replay",
            Subject,
            Ct);
        Assert.Equal(StatusCodes.Status200OK, first.StatusCode);

        ModelAccessCoordinationResult replay = await fixture.Coordinator.ExecuteAsync(
            fixture.Request,
            "key-replay",
            Subject,
            Ct);

        Assert.Equal(StatusCodes.Status409Conflict, replay.StatusCode);
        string body = JsonSerializer.Serialize(replay.Body);
        Assert.Contains("model_call_already_completed", body);
        Assert.Contains("receiptId", body);
        Assert.Contains("requestHash", body);
        Assert.Contains("resultHash", body);
        Assert.DoesNotContain("payload", body, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("summary", body, StringComparison.OrdinalIgnoreCase);
    }

    private sealed class Fixture
    {
        public required ModelAccessCoordinator Coordinator { get; init; }
        public required MemoryDeviceRegistry Registry { get; init; }
        public required CountingConsumptionStore Consumption { get; init; }
        public required FakeBroker Broker { get; init; }
        public required ModelAccessRequest Request { get; init; }
        public required string RegistrationId { get; init; }

        public static async Task<Fixture> CreateAsync(
            IModelCallIdempotencyStore? idempotency = null)
        {
            SupportPlaneOptions options = new()
            {
                Authority =
                    "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
                Audience = "api://22222222-2222-2222-2222-222222222222",
                ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
                Region = "westeurope",
            };
            MemoryDeviceRegistry registry = new();
            DeviceRegistrationResponse registration = await registry.RegisterAsync(
                Subject,
                new DeviceRegistrationRequest(
                    "desktop-device-registration.v1",
                    InstallationPublicKey,
                    RequestGuards.TryGetInstallationPublicKeyHash(
                        InstallationPublicKey,
                        out string keyHash)
                        ? keyHash
                        : throw new InvalidOperationException("key hash"),
                    "0.1.0-beta.1",
                    "windows",
                    "x64",
                    1),
                CancellationToken.None);
            ModelAccessRequest request = CreateBoundRequest(
                registration.RegistrationId,
                keyHash);
            FakeBroker broker = new(request, options.Region);
            CountingConsumptionStore consumption = new();
            ModelAccessCoordinator coordinator = new(
                registry,
                broker,
                new AlwaysVerifiedConsentVerifier(),
                consumption,
                idempotency ?? new MemoryModelCallIdempotencyStore(
                    128,
                    TimeSpan.FromMinutes(5),
                    TimeProvider.System),
                options,
                TimeProvider.System);
            return new Fixture
            {
                Coordinator = coordinator,
                Registry = registry,
                Consumption = consumption,
                Broker = broker,
                Request = request,
                RegistrationId = registration.RegistrationId,
            };
        }
    }

    private static ModelAccessRequest CreateBoundRequest(
        string registrationId,
        string installationPublicKeyHash)
    {
        const string content = "review this bounded context";
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            Sha256(content),
            Encoding.UTF8.GetByteCount(content),
            "source",
            content);
        ModelContextConsent consent = new(
            "sapphirus.model-context-consent.v1",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            Hash,
            Hash,
            registrationId,
            installationPublicKeyHash,
            "lease_01J00000000000000000000000",
            Hash,
            "policy_01J00000000000000000000000",
            7,
            Hash,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:05:00.000Z"),
            Hash,
            Hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                "installation-key-2026-07",
                Hash,
                "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl"));
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            consent.RequestId,
            "windows_local",
            registrationId,
            consent.Purpose,
            consent.ModelRole,
            consent.CanonicalOutputSchemaId,
            consent.CanonicalOutputSchemaHash,
            Hash,
            consent,
            [item],
            "transient_no_store",
            "interactive-standard");
        string manifestHash = RequestGuards.ComputeContextManifestHash(request);
        return request with
        {
            LocalEgressManifestHash = manifestHash,
            Consent = request.Consent with { ManifestHash = manifestHash },
        };
    }

    private static ModelAccessResult CreateBoundResult(
        ModelAccessRequest request,
        string region)
    {
        const string payload = "{\"summary\":\"completed\"}";
        string payloadHash = Sha256(payload);
        DateTimeOffset now = DateTimeOffset.Parse("2026-07-16T10:00:01.000Z");
        string receiptId = "receipt_" + Convert.ToHexString(
            System.Security.Cryptography.RandomNumberGenerator.GetBytes(16));
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            receiptId,
            request.RequestId,
            RequestGuards.Fingerprint(request),
            payloadHash,
            "windows_local",
            request.Consent.TenantHash,
            request.Consent.SubjectHash,
            request.RegistrationId,
            request.LocalEgressManifestHash,
            request.Consent.InvocationBindingHash,
            request.Consent.ConsumptionHash,
            request.Consent.ConsentEnvelopeHash,
            request.Consent.ConsentDisclosureHash,
            request.Consent.ProviderProfileHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.DeploymentHash,
            request.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaHash,
            Hash,
            Hash,
            request.RetentionMode,
            region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(payload),
            new ModelAccessUsage(2100, 300, 7200, "EUR"),
            0,
            [],
            "provider-request-opaque",
            now,
            now.AddSeconds(1),
            "succeeded",
            Hash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://support.sapphirus.example/",
                "sapphirus-desktop",
                "model-receipt-key-2026-07",
                Hash,
                "ZXhhbXBsZS1zdXBwb3J0LXBsYW5lLXNpZ25hdHVyZQ"));
        return new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            request.CanonicalOutputSchemaId,
            payload,
            payloadHash,
            receipt);
    }

    private static string Sha256(string value) => "sha256:" + Convert.ToHexStringLower(
        System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(value)));

    private sealed class AlwaysVerifiedConsentVerifier : IContextConsentVerifier
    {
        public ValueTask<ContextConsentVerification> VerifyAsync(
            ContextConsentVerificationRequest request,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(ContextConsentVerification.Verified);
    }

    private sealed class CountingConsumptionStore : IContextConsentConsumptionStore
    {
        private readonly HashSet<string> _consumed = [];
        private readonly Lock _gate = new();

        public int ConsumeCalls { get; private set; }

        public ValueTask<ContextConsentConsumption> ConsumeAsync(
            ContextConsentConsumptionRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            lock (_gate)
            {
                ConsumeCalls++;
                string authority =
                    $"{request.SubjectPartition}:{request.RegistrationId}:{request.ConsumptionHash}";
                return ValueTask.FromResult(_consumed.Add(authority)
                    ? ContextConsentConsumption.Consumed
                    : ContextConsentConsumption.AlreadyConsumed);
            }
        }
    }

    private sealed class FakeBroker(ModelAccessRequest boundRequest, string region)
        : IModelAccessBroker
    {
        private int _completions;

        public ModelAccessFailedException? Failure { get; set; }
        public TimeSpan Delay { get; set; }
        public Action? OnComplete { get; set; }
        public string? LastReceiptId { get; private set; }

        public int Completions => _completions;

        public async Task<ModelAccessResult> CompleteAsync(
            string subject,
            ModelAccessRequest request,
            CancellationToken cancellationToken)
        {
            if (Delay > TimeSpan.Zero)
            {
                await Task.Delay(Delay, cancellationToken);
            }
            OnComplete?.Invoke();
            if (Failure is not null)
            {
                throw Failure;
            }
            Interlocked.Increment(ref _completions);
            ModelAccessResult result = CreateBoundResult(boundRequest, region);
            LastReceiptId = result.Receipt.ReceiptId;
            return result;
        }
    }

    private sealed class UncertainIdempotencyStore : IModelCallIdempotencyStore
    {
        public Task<ModelCallIdempotencyResult> ExecuteAsync(
            string subject,
            string key,
            string requestFingerprint,
            Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
            Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
            CancellationToken cancellationToken) =>
            Task.FromException<ModelCallIdempotencyResult>(
                new ModelCallIdempotencyUncertainException());
    }
}
