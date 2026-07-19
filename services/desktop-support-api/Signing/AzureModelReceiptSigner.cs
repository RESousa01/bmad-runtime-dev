using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Signing;

/// <summary>
/// Produces vault-signed model access receipts. The canonical receipt hash
/// covers every receipt field except the hash and proof themselves; the
/// proof binds that exact digest, the immutable key version, and the
/// configured issuer/audience. Signing runs only after semantic validation.
/// </summary>
public sealed class AzureModelReceiptSigner(
    IHashSigner receiptSigner,
    SupportPlaneOptions supportPlane) : IModelReceiptSigner
{
    public async Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(result);
        string recomputedResultHash = Hash(result.PayloadJson);
        if (!string.Equals(recomputedResultHash, result.PayloadHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "The unsigned model result did not match its declared payload hash.");
        }
        if (result.TerminalStatus != "succeeded"
            || !RequestGuards.IsSha256(result.SchemaProjectionHash)
            || !RequestGuards.IsSha256(result.CredentialBindingHash))
        {
            throw new InvalidOperationException(
                "The unsigned model result is not eligible for a signed receipt.");
        }

        string requestHash = RequestGuards.Fingerprint(request);
        string receiptId = ContractIds.FromEntropy(
            "receipt",
            RandomNumberGenerator.GetBytes(16));
        ModelAccessReceipt unsignedReceipt = new(
            "sapphirus.model-access-receipt.v1",
            receiptId,
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
            supportPlane.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(result.PayloadJson),
            result.Usage,
            result.RetryCount,
            result.FallbackEvents,
            result.ProviderRequestId,
            result.StartedAt,
            result.CompletedAt,
            result.TerminalStatus,
            "",
            new ModelAccessReceiptProof("", "", "", "", "", "", ""));

        byte[] digest = ComputeReceiptDigest(unsignedReceipt);
        string receiptHash = CanonicalProof.Render(digest);
        string signature = await receiptSigner
            .SignAsync(digest, cancellationToken)
            .ConfigureAwait(false);
        ModelAccessReceipt receipt = unsignedReceipt with
        {
            ReceiptHash = receiptHash,
            Proof = new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                supportPlane.Authority,
                supportPlane.Audience,
                receiptSigner.KeyId,
                receiptHash,
                signature),
        };
        return new ModelAccessResult(
            "desktop-model-access-result.v1",
            request.RequestId,
            result.OutputSchemaId,
            result.PayloadJson,
            result.PayloadHash,
            receipt);
    }

    /// <summary>
    /// Canonical receipt digest: purpose <c>model-access-receipt</c> over
    /// every receipt field except <c>receiptHash</c> and <c>proof</c>.
    /// </summary>
    internal static byte[] ComputeReceiptDigest(ModelAccessReceipt receipt)
    {
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = receipt.SchemaVersion,
            ["receiptId"] = receipt.ReceiptId,
            ["requestId"] = receipt.RequestId,
            ["requestHash"] = receipt.RequestHash,
            ["resultHash"] = receipt.ResultHash,
            ["deliveryModel"] = receipt.DeliveryModel,
            ["tenantHash"] = receipt.TenantHash,
            ["subjectHash"] = receipt.SubjectHash,
            ["registrationId"] = receipt.RegistrationId,
            ["manifestHash"] = receipt.ManifestHash,
            ["invocationBindingHash"] = receipt.InvocationBindingHash,
            ["consumptionHash"] = receipt.ConsumptionHash,
            ["consentEnvelopeHash"] = receipt.ConsentEnvelopeHash,
            ["consentDisclosureHash"] = receipt.ConsentDisclosureHash,
            ["providerProfileHash"] = receipt.ProviderProfileHash,
            ["modelProfileHash"] = receipt.ModelProfileHash,
            ["modelCapabilityHash"] = receipt.ModelCapabilityHash,
            ["deploymentHash"] = receipt.DeploymentHash,
            ["canonicalOutputSchemaId"] = receipt.CanonicalOutputSchemaId,
            ["canonicalOutputSchemaHash"] = receipt.CanonicalOutputSchemaHash,
            ["providerSchemaProjectionHash"] = receipt.ProviderSchemaProjectionHash,
            ["credentialBindingHash"] = receipt.CredentialBindingHash,
            ["retentionMode"] = receipt.RetentionMode,
            ["region"] = receipt.Region,
            ["inputBytes"] = receipt.InputBytes,
            ["outputBytes"] = receipt.OutputBytes,
            ["usage"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["inputTokens"] = receipt.Usage.InputTokens,
                ["outputTokens"] = receipt.Usage.OutputTokens,
                ["costMicrounits"] = receipt.Usage.CostMicrounits,
                ["currency"] = receipt.Usage.Currency,
            },
            ["retryCount"] = receipt.RetryCount,
            ["fallbackEvents"] = receipt.FallbackEvents
                .Select(static fallback => (object)new SortedDictionary<string, object?>(
                    StringComparer.Ordinal)
                {
                    ["sequence"] = fallback.Sequence,
                    ["fromProfileHash"] = fallback.FromProfileHash,
                    ["toProfileHash"] = fallback.ToProfileHash,
                    ["policyTransitionHash"] = fallback.PolicyTransitionHash,
                })
                .ToArray(),
            ["providerRequestId"] = receipt.ProviderRequestId,
            ["startedAt"] = CanonicalPolicyProjector.RenderInstant(receipt.StartedAt),
            ["completedAt"] = CanonicalPolicyProjector.RenderInstant(receipt.CompletedAt),
            ["terminalStatus"] = receipt.TerminalStatus,
        };
        return CanonicalProof.ComputeDigest("model-access-receipt", draft);
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
