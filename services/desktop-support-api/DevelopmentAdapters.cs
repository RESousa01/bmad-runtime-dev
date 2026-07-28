using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Signing;

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

/// <summary>
/// The development version two receipt signer. It exists beside
/// <see cref="DevelopmentModelReceiptSigner"/>, is gated on the same
/// <see cref="SupportPlaneOptions.DevelopmentSigningEnabled"/> flag, and is what
/// lets a local run with a real project endpoint and the development API key
/// actually complete a streaming call instead of stopping at an uncomposed
/// signer.
///
/// <para>
/// A development receipt must stay trivially distinguishable from a production
/// one, and three independent things make it so.
/// </para>
/// <para>
/// First, the credential mechanism is already folded into
/// <c>credentialBindingHash</c> by
/// <see cref="ModelReceiptBinding.ComputeCredentialBindingHash"/>. This signer
/// copies the server-computed binding through untouched and never recomputes or
/// substitutes it, so a receipt produced under the development API key carries a
/// different credential binding from a byte-identical managed-identity call.
/// </para>
/// <para>
/// Second, the proof carries a development issuer and key identifier that name
/// no Key Vault key and resolve to nothing.
/// </para>
/// <para>
/// Third, and load bearing, the proof's signature is not a signature. It is the
/// base64url of a domain-separated marker followed by the receipt digest, which
/// is <see cref="ProofMarker"/>'s length plus thirty-two bytes and therefore can
/// never decode to the sixty-byte-plus-four IEEE P-1363 <c>r||s</c> pair an
/// ES256 verification requires. Production verification of a development receipt
/// fails at the length check before any curve arithmetic happens; it cannot
/// accidentally succeed.
/// </para>
/// <para>
/// The receipt hash itself is the real canonical version two digest, so a
/// development receipt is still internally consistent and still fails
/// <c>ModelResultGuards.ValidateReceiptV2</c> if anything about it is edited.
/// Being honest about the body while being unmistakable about the proof is the
/// point.
/// </para>
/// </summary>
public sealed class DevelopmentModelReceiptSignerV2(SupportPlaneOptions options)
    : IModelReceiptSignerV2
{
    /// <summary>
    /// The domain-separated marker the development proof carries instead of a
    /// signature. It is prepended to the receipt digest, so the encoded value
    /// has neither the length nor the shape of an ES256 signature.
    /// </summary>
    internal const string ProofMarker = "development-model-receipt-unsigned:";

    /// <summary>An issuer that resolves to nothing.</summary>
    internal const string ProofIssuer = "https://development.invalid/";

    /// <summary>A key identifier that names no Key Vault key.</summary>
    internal const string ProofKeyId = "development-model-receipt-key-not-production";

    public Task<ModelAccessReceiptV2> SignAsync(
        string subject,
        ModelReceiptAuthorization authorization,
        UnsignedModelAccessResultV2 result,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentNullException.ThrowIfNull(authorization);
        ArgumentNullException.ThrowIfNull(result);
        if (!options.DevelopmentSigningEnabled)
        {
            throw new InvalidOperationException(
                "A production model-receipt signing provider is not configured.");
        }
        if (!string.Equals(Hash(result.PayloadJson), result.PayloadHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "The unsigned model result did not match its declared payload hash.");
        }

        // The same eligibility rule the vault signer applies. Only a succeeded,
        // fully bound result may carry a receipt at all, in Development too.
        if (result.TerminalStatus != ModelTerminalStatuses.Succeeded
            || !ContractIds.Is(result.RunId, "run")
            || !RequestGuards.IsSha256(result.Binding.CredentialBindingHash)
            || !RequestGuards.IsSha256(result.Binding.ProviderProfileHash)
            || !RequestGuards.IsSha256(result.Binding.ModelProfileHash)
            || !RequestGuards.IsSha256(result.Binding.ModelCapabilityHash)
            || !RequestGuards.IsSha256(result.Binding.ResolvedProfileHash)
            || !RequestGuards.IsSha256(result.Binding.CatalogHash)
            || !RequestGuards.IsSha256(result.Binding.DeploymentInventoryHash)
            || !RequestGuards.IsSha256(result.Binding.ResolvedDeploymentHash))
        {
            throw new InvalidOperationException(
                "The unsigned model result is not eligible for a signed receipt.");
        }

        ModelAccessReceiptV2 unsignedReceipt = new(
            ModelAccessReceiptV2.SchemaVersionValue,
            ContractIds.FromEntropy("receipt", RandomNumberGenerator.GetBytes(16)),
            authorization.RequestId,
            result.RunId,
            authorization.RequestHash,
            result.PayloadHash,
            "windows_local",
            authorization.TenantHash,
            authorization.SubjectHash,
            authorization.RegistrationId,
            authorization.ManifestHash,
            authorization.InvocationBindingHash,
            authorization.ConsumptionHash,
            authorization.ConsentEnvelopeHash,
            authorization.ConsentDisclosureHash,
            // Copied through, never recomputed: the credential mechanism the
            // call actually used is already inside these server-side hashes.
            result.Binding.ProviderProfileHash,
            result.Binding.ModelProfileHash,
            result.Binding.ModelCapabilityHash,
            result.ResponseLane,
            result.RequestedProfileId,
            result.Binding.ResolvedProfileId,
            result.Binding.ResolvedProfileHash,
            result.RequestedVariantId,
            result.Binding.ResolvedVariantId,
            result.Binding.CatalogVersion,
            result.Binding.CatalogHash,
            result.Binding.DeploymentInventoryHash,
            result.Binding.ResolvedDeploymentHash,
            result.Binding.ResolvedModel,
            result.ReportedResponseModel,
            result.Binding.CredentialBindingHash,
            authorization.RetentionMode,
            result.Binding.Region,
            authorization.InputBytes,
            result.OutputBytes,
            result.Usage,
            result.RetryCount,
            result.FallbackEvents,
            result.ProviderRequestId,
            result.StartedAt,
            result.CompletedAt,
            result.TerminalStatus,
            "",
            new ModelAccessReceiptProof("", "", "", "", "", "", ""));

        // The real canonical digest, computed by the production code path, so a
        // development receipt is internally consistent and any edit to it is
        // still detectable.
        byte[] digest = AzureModelReceiptSigner.ComputeReceiptDigestV2(unsignedReceipt);
        string receiptHash = CanonicalProof.Render(digest);
        return Task.FromResult(unsignedReceipt with
        {
            ReceiptHash = receiptHash,
            Proof = new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                ProofIssuer,
                options.Audience,
                ProofKeyId,
                receiptHash,
                RenderDevelopmentProof(digest)),
        });
    }

    /// <summary>
    /// The development proof value: base64url of <see cref="ProofMarker"/>
    /// followed by the receipt digest. Its decoded length is the marker length
    /// plus thirty-two, never sixty-four, so no ES256 verification can consume
    /// it as an <c>r||s</c> pair.
    /// </summary>
    internal static string RenderDevelopmentProof(byte[] digest)
    {
        ArgumentNullException.ThrowIfNull(digest);
        byte[] marker = Encoding.ASCII.GetBytes(ProofMarker);
        byte[] value = new byte[marker.Length + digest.Length];
        marker.CopyTo(value, 0);
        digest.CopyTo(value, marker.Length);
        return Convert.ToBase64String(value)
            .TrimEnd('=')
            .Replace('+', '-')
            .Replace('/', '_');
    }

    /// <summary>
    /// Whether a proof is a development marker rather than a vault signature.
    /// It exists so an audit — or a test — can assert the distinction without
    /// re-spelling the marker.
    /// </summary>
    internal static bool IsDevelopmentProof(ModelAccessReceiptProof? proof)
    {
        if (proof is null || string.IsNullOrEmpty(proof.Signature))
        {
            return false;
        }
        try
        {
            byte[] decoded = Security.InstallationPublicKey.DecodeBase64Url(proof.Signature);
            byte[] marker = Encoding.ASCII.GetBytes(ProofMarker);
            return decoded.Length == marker.Length + 32
                && decoded.AsSpan(0, marker.Length).SequenceEqual(marker);
        }
        catch (FormatException)
        {
            return false;
        }
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
