using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Signing;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Signing;

public sealed class PolicySigningTests
{
    private const string Prefix = "desktop-support:policy:";
    private const string ActiveKeyId =
        "https://signing.vault.azure.net/keys/model-receipt-signing/1111aaaa";

    private static readonly DateTimeOffset Now =
        DateTimeOffset.Parse("2026-07-20T10:00:00.000Z");

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_valid_snapshot_produces_a_signed_policy_bound_to_its_canonical_hash()
    {
        using EcdsaHashSigner signer = new(ActiveKeyId);
        AzureSignedPolicyService service = CreateService(signer, out _);

        SignedDesktopPolicy policy = await service.CurrentPolicyAsync(Ct);

        Assert.Equal("policy_01J00000000000000000000000", policy.PolicyId);
        Assert.Equal(ActiveKeyId, policy.KeyId);
        (SignedDesktopPolicy unsigned, byte[] digest) = CanonicalPolicyProjector.Project(
            AppConfigurationPolicyProvider.Validate(ValidSettings(), Now));
        Assert.Equal(unsigned.PolicyHash, policy.PolicyHash);
        Assert.True(signer.Verify(digest, policy.Signature));
    }

    [Theory]
    [InlineData("unknown_field", "desktop-support:policy:extraField", "1")]
    [InlineData("foreign_key", "feature-flags:beta", "true")]
    [InlineData("bad_policy_id", Prefix + "policyId", "policy!")]
    [InlineData("bad_version", Prefix + "policyVersion", "0")]
    [InlineData("over_limit_bytes", Prefix + "maximumContextBytes", "10485760")]
    [InlineData("over_limit_items", Prefix + "maximumContextItems", "4096")]
    [InlineData("empty_regions", Prefix + "allowedRegions", "")]
    [InlineData("bad_retention", Prefix + "retentionMode", "persistent")]
    [InlineData("no_deployments", Prefix + "approvedModelDeployments", "")]
    public void Invalid_snapshots_are_rejected(string label, string key, string value)
    {
        Dictionary<string, string> settings = ValidSettings();
        settings[key] = value;

        InvalidOperationException rejection = Assert.Throws<InvalidOperationException>(
            () => AppConfigurationPolicyProvider.Validate(settings, Now));
        Assert.NotNull(label);
        Assert.NotNull(rejection.Message);
    }

    [Fact]
    public void Missing_required_fields_are_rejected()
    {
        Dictionary<string, string> settings = ValidSettings();
        settings.Remove(Prefix + "retentionMode");

        Assert.Throws<InvalidOperationException>(
            () => AppConfigurationPolicyProvider.Validate(settings, Now));
    }

    [Fact]
    public async Task Policy_downgrade_is_rejected_on_refresh()
    {
        MutableSettingsSource source = new(ValidSettings());
        FixedTimeProvider clock = new(Now);
        AppConfigurationPolicyProvider provider = new(
            source,
            clock,
            TimeSpan.FromMinutes(5),
            TimeSpan.FromHours(1));
        _ = await provider.GetSnapshotAsync(Ct);

        source.Settings[Prefix + "policyVersion"] = "6";
        clock.Advance(TimeSpan.FromMinutes(10));

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            provider.GetSnapshotAsync(Ct));
    }

    [Fact]
    public async Task Stale_refresh_serves_last_known_valid_only_within_its_lifetime()
    {
        MutableSettingsSource source = new(ValidSettings());
        FixedTimeProvider clock = new(Now);
        AppConfigurationPolicyProvider provider = new(
            source,
            clock,
            TimeSpan.FromMinutes(5),
            TimeSpan.FromHours(1));
        PolicySnapshot first = await provider.GetSnapshotAsync(Ct);

        source.Fail = true;
        clock.Advance(TimeSpan.FromMinutes(30));
        PolicySnapshot lastKnownValid = await provider.GetSnapshotAsync(Ct);
        Assert.Equal(first.PolicyVersion, lastKnownValid.PolicyVersion);

        clock.Advance(TimeSpan.FromHours(2));
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            provider.GetSnapshotAsync(Ct));
    }

    [Fact]
    public async Task Lease_proofs_bind_the_canonical_lease_hash_and_key_version()
    {
        using EcdsaHashSigner signer = new(ActiveKeyId);
        AzureSignedPolicyService service = CreateService(signer, out _);

        SignedEntitlementLease lease = await service.CreateLeaseAsync(
            "subject-a",
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            Ct);

        Assert.Equal(ActiveKeyId, lease.KeyId);
        Assert.StartsWith("lease_", lease.LeaseId);
        (_, byte[] digest) = CanonicalPolicyProjector.ProjectLease(lease with
        {
            KeyId = "",
            Signature = "",
        });
        Assert.True(signer.Verify(digest, lease.Signature));
        Assert.False(signer.Verify(
            SHA256.HashData(Encoding.UTF8.GetBytes("other-payload")),
            lease.Signature));
    }

    [Fact]
    public async Task Receipt_proofs_bind_digest_key_version_issuer_and_audience()
    {
        using EcdsaHashSigner signer = new(ActiveKeyId);
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        AzureModelReceiptSigner receiptSigner = new(signer, supportPlane);
        (ModelAccessRequest request, UnsignedModelAccessResult unsigned) = CreateUnsigned();

        ModelAccessResult result = await receiptSigner.SignAsync(
            "subject-a",
            request,
            unsigned,
            Ct);

        ModelAccessReceipt receipt = result.Receipt;
        Assert.Equal(ActiveKeyId, receipt.Proof.KeyId);
        Assert.Equal(supportPlane.Authority, receipt.Proof.Issuer);
        Assert.Equal(supportPlane.Audience, receipt.Proof.Audience);
        Assert.Equal(receipt.ReceiptHash, receipt.Proof.SignedPayloadHash);
        byte[] digest = AzureModelReceiptSigner.ComputeReceiptDigest(receipt with
        {
            ReceiptHash = "",
            Proof = new ModelAccessReceiptProof("", "", "", "", "", "", ""),
        });
        Assert.Equal(CanonicalProof.Render(digest), receipt.ReceiptHash);
        Assert.True(signer.Verify(digest, receipt.Proof.Signature));
    }

    [Fact]
    public void Key_rotation_accepts_only_the_explicit_overlap()
    {
        SigningKeyRing ring = new(
            ActiveKeyId,
            ["https://signing.vault.azure.net/keys/model-receipt-signing/0000ffff"]);

        Assert.True(ring.IsAcceptableForVerification(ActiveKeyId));
        Assert.True(ring.IsAcceptableForVerification(
            "https://signing.vault.azure.net/keys/model-receipt-signing/0000ffff"));
        Assert.False(ring.IsAcceptableForVerification(
            "https://signing.vault.azure.net/keys/model-receipt-signing/9999dead"));
        Assert.False(ring.IsAcceptableForVerification(""));
        Assert.False(ring.IsAcceptableForVerification(null));
    }

    [Fact]
    public async Task Signer_unavailability_yields_no_unsigned_artifact()
    {
        FailingHashSigner failing = new();
        AzureSignedPolicyService service = CreateService(failing, out _);
        AzureModelReceiptSigner receiptSigner = new(failing, CreateSupportPlane());
        (ModelAccessRequest request, UnsignedModelAccessResult unsigned) = CreateUnsigned();

        await Assert.ThrowsAsync<TimeoutException>(() => service.CurrentPolicyAsync(Ct));
        await Assert.ThrowsAsync<TimeoutException>(() =>
            service.CreateLeaseAsync("subject-a", "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA", Ct));
        await Assert.ThrowsAsync<TimeoutException>(() =>
            receiptSigner.SignAsync("subject-a", request, unsigned, Ct));
    }

    [Fact]
    public async Task Signing_is_invoked_only_after_semantic_validation_succeeds()
    {
        RecordingHashSigner recording = new();
        AzureSignedPolicyService service = CreateService(
            recording,
            out MutableSettingsSource source);
        source.Settings[Prefix + "retentionMode"] = "persistent";
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            service.CurrentPolicyAsync(Ct));
        Assert.Equal(0, recording.SignCalls);

        AzureModelReceiptSigner receiptSigner = new(recording, CreateSupportPlane());
        (ModelAccessRequest request, UnsignedModelAccessResult unsigned) = CreateUnsigned();
        UnsignedModelAccessResult tampered = unsigned with
        {
            PayloadHash = "sha256:" + new string('b', 64),
        };
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            receiptSigner.SignAsync("subject-a", request, tampered, Ct));
        UnsignedModelAccessResult interrupted = unsigned with
        {
            TerminalStatus = "interrupted",
        };
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            receiptSigner.SignAsync("subject-a", request, interrupted, Ct));
        Assert.Equal(0, recording.SignCalls);
    }

    private static AzureSignedPolicyService CreateService(
        IHashSigner signer,
        out MutableSettingsSource source)
    {
        source = new MutableSettingsSource(ValidSettings());
        AppConfigurationPolicyProvider provider = new(
            source,
            new FixedTimeProvider(Now),
            TimeSpan.FromMinutes(5),
            TimeSpan.FromHours(1));
        return new AzureSignedPolicyService(provider, signer, new FixedTimeProvider(Now));
    }

    private static Dictionary<string, string> ValidSettings() => new(StringComparer.Ordinal)
    {
        [Prefix + "policyId"] = "policy_01J00000000000000000000000",
        [Prefix + "policyVersion"] = "7",
        [Prefix + "systemBrowserFallbackAllowed"] = "false",
        [Prefix + "maximumContextBytes"] = "524288",
        [Prefix + "maximumContextItems"] = "64",
        [Prefix + "allowedRegions"] = "westeurope",
        [Prefix + "approvedModelDeployments"] = "desktop-planner",
        [Prefix + "retentionMode"] = "transient_no_store",
    };

    private static SupportPlaneOptions CreateSupportPlane() => new()
    {
        Authority =
            "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://22222222-2222-2222-2222-222222222222",
        ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
        Region = "westeurope",
    };

    private static (ModelAccessRequest Request, UnsignedModelAccessResult Unsigned)
        CreateUnsigned()
    {
        const string hash =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        const string payload = "{\"summary\":\"completed\"}";
        ModelContextConsent consent = new(
            "sapphirus.model-context-consent.v1",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            hash,
            hash,
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            hash,
            "lease_01J00000000000000000000000",
            hash,
            "policy_01J00000000000000000000000",
            7,
            hash,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            hash,
            hash,
            hash,
            hash,
            hash,
            hash,
            hash,
            hash,
            hash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            Now,
            Now,
            Now.AddMinutes(5),
            hash,
            hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                hash,
                hash,
                "ZXhhbXBsZQ"));
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            consent.RequestId,
            "windows_local",
            consent.RegistrationId,
            consent.Purpose,
            consent.ModelRole,
            consent.CanonicalOutputSchemaId,
            consent.CanonicalOutputSchemaHash,
            hash,
            consent,
            [],
            "transient_no_store",
            "interactive-standard");
        UnsignedModelAccessResult unsigned = new(
            request.CanonicalOutputSchemaId,
            payload,
            "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(Encoding.UTF8.GetBytes(payload))),
            hash,
            hash,
            new ModelAccessUsage(100, 50, 3000, "EUR"),
            0,
            [],
            "provider-request-opaque",
            Now,
            Now.AddSeconds(2),
            "succeeded");
        return (request, unsigned);
    }

    private sealed class MutableSettingsSource(Dictionary<string, string> settings)
        : IPolicySettingsSource
    {
        public Dictionary<string, string> Settings { get; } = settings;
        public bool Fail { get; set; }

        public Task<IReadOnlyDictionary<string, string>> LoadAsync(
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return Fail
                ? Task.FromException<IReadOnlyDictionary<string, string>>(
                    new InvalidOperationException("configuration unavailable"))
                : Task.FromResult<IReadOnlyDictionary<string, string>>(
                    new Dictionary<string, string>(Settings, StringComparer.Ordinal));
        }
    }

    private sealed class EcdsaHashSigner(string keyId) : IHashSigner, IDisposable
    {
        private readonly ECDsa _key = ECDsa.Create(ECCurve.NamedCurves.nistP256);

        public string KeyId { get; } = keyId;

        public Task<string> SignAsync(
            byte[] sha256Digest,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            byte[] signature = _key.SignHash(
                sha256Digest,
                DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
            return Task.FromResult(KeyVaultHashSigner.Base64Url(signature));
        }

        public bool Verify(byte[] sha256Digest, string signature)
        {
            byte[] raw = Convert.FromBase64String(
                signature.Replace('-', '+').Replace('_', '/')
                + new string('=', (4 - signature.Length % 4) % 4));
            return _key.VerifyHash(
                sha256Digest,
                raw,
                DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
        }

        public void Dispose() => _key.Dispose();
    }

    private sealed class FailingHashSigner : IHashSigner
    {
        public string KeyId => ActiveKeyId;

        public Task<string> SignAsync(
            byte[] sha256Digest,
            CancellationToken cancellationToken) =>
            Task.FromException<string>(new TimeoutException("vault timeout"));
    }

    private sealed class RecordingHashSigner : IHashSigner
    {
        public int SignCalls { get; private set; }

        public string KeyId => ActiveKeyId;

        public Task<string> SignAsync(
            byte[] sha256Digest,
            CancellationToken cancellationToken)
        {
            SignCalls++;
            return Task.FromResult("c2lnbmVk");
        }
    }

    private sealed class FixedTimeProvider(DateTimeOffset start) : TimeProvider
    {
        private DateTimeOffset _now = start;

        public override DateTimeOffset GetUtcNow() => _now;

        public void Advance(TimeSpan delta) => _now += delta;
    }
}
