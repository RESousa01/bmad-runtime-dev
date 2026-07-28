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
    SupportPlaneOptions supportPlane) : IModelReceiptSigner, IModelReceiptSignerV2
{
    /// <summary>
    /// The version two digest purpose. It differs from the version one purpose
    /// so a version one receipt and a version two receipt can never collide
    /// under the same preimage, and historical receipts keep verifying against
    /// the digest they were signed with.
    /// </summary>
    internal const string ReceiptV2DigestPurpose = "model-access-receipt-v2";

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

    /// <summary>
    /// Signs a version two receipt.
    ///
    /// <para>
    /// Signing runs only after the output has been validated and only for a
    /// succeeded result. A failed, refused, or cancelled run produces evidence
    /// — telemetry, a problem document, a terminal stream event — but never a
    /// signature that could be read as proof the model answered.
    /// </para>
    /// </summary>
    public async Task<ModelAccessReceiptV2> SignAsync(
        string subject,
        ModelReceiptAuthorization authorization,
        UnsignedModelAccessResultV2 result,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentNullException.ThrowIfNull(authorization);
        ArgumentNullException.ThrowIfNull(result);
        if (!string.Equals(Hash(result.PayloadJson), result.PayloadHash, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "The unsigned model result did not match its declared payload hash.");
        }
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
            // The four profile hashes below are the server's, recomputed from
            // the resolved route. Version one copied them out of the client's
            // consent envelope, which proved only what the client believed.
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

        byte[] digest = ComputeReceiptDigestV2(unsignedReceipt);
        string receiptHash = CanonicalProof.Render(digest);
        string signature = await receiptSigner
            .SignAsync(digest, cancellationToken)
            .ConfigureAwait(false);
        return unsignedReceipt with
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
    }

    /// <summary>
    /// Canonical version two receipt digest: purpose
    /// <c>model-access-receipt-v2</c> over every receipt field except
    /// <c>receiptHash</c> and <c>proof</c>.
    ///
    /// <para>
    /// Adding a field to <see cref="ModelAccessReceiptV2"/> without adding it
    /// here would leave that field unsigned, so the two must always move
    /// together — along with <c>ModelResultGuards.ValidateReceiptV2</c> and the
    /// cross-language fixture.
    /// </para>
    /// </summary>
    internal static byte[] ComputeReceiptDigestV2(ModelAccessReceiptV2 receipt)
    {
        ArgumentNullException.ThrowIfNull(receipt);
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = receipt.SchemaVersion,
            ["receiptId"] = receipt.ReceiptId,
            ["requestId"] = receipt.RequestId,
            ["runId"] = receipt.RunId,
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
            ["responseLane"] = ResponseLaneDraft(receipt.ResponseLane),
            ["requestedProfileId"] = receipt.RequestedProfileId,
            ["resolvedProfileId"] = receipt.ResolvedProfileId,
            ["resolvedProfileHash"] = receipt.ResolvedProfileHash,
            ["requestedVariantId"] = receipt.RequestedVariantId,
            ["resolvedVariantId"] = receipt.ResolvedVariantId,
            ["catalogVersion"] = receipt.CatalogVersion,
            ["catalogHash"] = receipt.CatalogHash,
            ["deploymentInventoryHash"] = receipt.DeploymentInventoryHash,
            ["resolvedDeploymentHash"] = receipt.ResolvedDeploymentHash,
            ["resolvedModel"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["publisher"] = receipt.ResolvedModel.Publisher,
                ["name"] = receipt.ResolvedModel.Name,
                ["version"] = receipt.ResolvedModel.Version,
            },
            ["reportedResponseModel"] = receipt.ReportedResponseModel,
            ["credentialBindingHash"] = receipt.CredentialBindingHash,
            ["retentionMode"] = receipt.RetentionMode,
            ["region"] = receipt.Region,
            ["inputBytes"] = receipt.InputBytes,
            ["outputBytes"] = receipt.OutputBytes,
            ["usage"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["usageProvenance"] = receipt.Usage.UsageProvenance,
                ["inputTokens"] = receipt.Usage.InputTokens,
                ["outputTokens"] = receipt.Usage.OutputTokens,
                ["reasoningTokens"] = receipt.Usage.ReasoningTokens,
                ["cacheReadTokens"] = receipt.Usage.CacheReadTokens,
                ["cacheWriteTokens"] = receipt.Usage.CacheWriteTokens,
                ["costMicrounits"] = receipt.Usage.CostMicrounits,
                ["currency"] = receipt.Usage.Currency,
                ["priceProfileVersion"] = receipt.Usage.PriceProfileVersion,
            },
            ["retryCount"] = receipt.RetryCount,
            ["fallbackEvents"] = receipt.FallbackEvents
                .Select(static fallback => (object)new SortedDictionary<string, object?>(
                    StringComparer.Ordinal)
                {
                    ["sequence"] = fallback.Sequence,
                    ["fromProfileId"] = fallback.FromProfileId,
                    ["toProfileId"] = fallback.ToProfileId,
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
        return CanonicalProof.ComputeDigest(ReceiptV2DigestPurpose, draft);
    }

    private static SortedDictionary<string, object?> ResponseLaneDraft(
        IModelResponseLane lane) => lane switch
        {
            StructuredResponseLane structured =>
                new SortedDictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["requestMode"] = structured.RequestMode,
                    ["supportRoute"] = structured.SupportRoute,
                    ["canonicalOutputSchemaId"] = structured.CanonicalOutputSchemaId,
                    ["canonicalOutputSchemaHash"] = structured.CanonicalOutputSchemaHash,
                    ["providerSchemaProjectionHash"] = structured.ProviderSchemaProjectionHash,
                },
            StreamingChatLane streaming =>
                new SortedDictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["requestMode"] = streaming.RequestMode,
                    ["supportRoute"] = streaming.SupportRoute,
                    ["eventCount"] = streaming.EventCount,
                    ["finalSequence"] = streaming.FinalSequence,
                },
            _ => throw new InvalidOperationException(
                "The response lane is not one of the two closed variants."),
        };

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
