using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.DesktopSupportApi;

public sealed class DevelopmentSignedPolicyService(SupportPlaneOptions options) : ISignedPolicyService
{
    public Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        EnsureEnabled();
        SignedDesktopPolicy policy = new(
            "desktop-policy.v1",
            "policy_01J00000000000000000000000",
            1,
            Hash("development-policy"),
            true,
            512 * 1024,
            64,
            [options.Region],
            "development-ephemeral-not-production",
            "development-signature");
        return Task.FromResult(policy);
    }

    public Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        EnsureEnabled();
        DateTimeOffset now = DateTimeOffset.UtcNow;
        SignedEntitlementLease lease = new(
            "desktop-entitlement-lease.v1",
            "lease_" + Guid.NewGuid().ToString("N"),
            registrationId,
            Hash(subject),
            "windows_local",
            now,
            now.AddMinutes(-2),
            now.AddHours(24),
            now.AddHours(96),
            ["local_runtime", "model_access"],
            Hash("development-policy"),
            "0.1.0-beta.1",
            "development-ephemeral-not-production",
            "development-signature");
        return Task.FromResult(lease);
    }

    private void EnsureEnabled()
    {
        if (!options.DevelopmentSigningEnabled)
        {
            throw new InvalidOperationException("A production signing provider is not configured.");
        }
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}

public sealed class DevelopmentModelReceiptSigner(SupportPlaneOptions options) : IModelReceiptSigner
{
    public Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!options.DevelopmentSigningEnabled)
        {
            throw new InvalidOperationException(
                "A production model-receipt signing provider is not configured.");
        }
        string recomputedResultHash = Hash(result.PayloadJson);
        if (!string.Equals(recomputedResultHash, result.PayloadHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "The unsigned model result did not match its declared payload hash.");
        }
        string requestHash = RequestGuards.Fingerprint(request);
        string receiptHash = Hash(
            $"development-receipt:{requestHash}:{result.PayloadHash}:{request.Consent.ConsentEnvelopeHash}");
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            ContractIds.FromEntropy("receipt", RandomNumberGenerator.GetBytes(16)),
            request.RequestId,
            requestHash,
            result.PayloadHash,
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
            result.SchemaProjectionHash,
            result.CredentialBindingHash,
            request.RetentionMode,
            options.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(result.PayloadJson),
            result.Usage,
            result.RetryCount,
            result.FallbackEvents,
            result.ProviderRequestId,
            result.StartedAt,
            result.CompletedAt,
            result.TerminalStatus,
            receiptHash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://development.invalid/",
                options.Audience,
                "development-model-receipt-key",
                receiptHash,
                "ZGV2ZWxvcG1lbnQtb25seS1uby10cnVzdA"));
        return Task.FromResult(new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            result.OutputSchemaId,
            result.PayloadJson,
            result.PayloadHash,
            receipt));
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}

public sealed class DevelopmentModelAccessBroker(
    SupportPlaneOptions options,
    IModelReceiptSigner receiptSigner) : IModelAccessBroker
{
    public async Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (!options.DevelopmentModelEnabled)
        {
            throw new InvalidOperationException("A managed-identity model broker is not configured.");
        }
        DateTimeOffset startedAt = DateTimeOffset.UtcNow;
        string payload = JsonSerializer.Serialize(new
        {
            summary = "Review the selected context and prepare a bounded change proposal.",
            steps = new[] { "Understand context", "Plan changes", "Review before applying" },
            proposedChanges = Array.Empty<object>(),
        });
        string resultHash = Hash(payload);
        UnsignedModelAccessResult unsigned = new(
            request.CanonicalOutputSchemaId,
            payload,
            resultHash,
            Hash("development-schema-projection"),
            Hash($"development-credential-binding:{subject}"),
            new ModelAccessUsage(0, 0, 0, "EUR"),
            0,
            [],
            null,
            startedAt,
            DateTimeOffset.UtcNow,
            "succeeded");
        return await receiptSigner.SignAsync(subject, request, unsigned, cancellationToken)
            .ConfigureAwait(false);
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
