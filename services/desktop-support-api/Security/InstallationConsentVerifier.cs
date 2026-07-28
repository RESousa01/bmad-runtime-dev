using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Model;

namespace Sapphirus.DesktopSupportApi.Security;

/// <summary>
/// Verifies that the consent envelope was signed by the registered
/// installation key over the exact domain-separated canonical envelope hash
/// (see <c>crates/desktop-cloud/src/installation_identity.rs</c> for the
/// shared signature specification). All failures collapse into
/// <see cref="ContextConsentVerification.Rejected"/> — no cryptographic
/// diagnostics leave this type.
/// </summary>
public sealed class InstallationConsentVerifier(TimeProvider timeProvider)
    : IContextConsentVerifier
{
    private const string HashPurposePreimage = "sapphirus:model-context-consent:v1\n";

    public ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(
            Verify(request)
                ? ContextConsentVerification.Verified
                : ContextConsentVerification.Rejected);
    }

    private bool Verify(ContextConsentVerificationRequest request)
    {
        ModelContextConsent consent = request.Request.Consent;
        ModelContextConsentProof proof = consent.Proof;
        DateTimeOffset now = timeProvider.GetUtcNow();
        if (!string.Equals(
            consent.RegistrationId,
            request.Device.RegistrationId,
            StringComparison.Ordinal)
            || !string.Equals(
                consent.InstallationPublicKeyHash,
                request.Device.InstallationPublicKeyHash,
                StringComparison.Ordinal)
            || !string.Equals(
                consent.ManifestHash,
                request.RecomputedManifestHash,
                StringComparison.Ordinal)
            || now < consent.NotBefore
            || now >= consent.ExpiresAt
            || proof.ProofType != "installation_signature"
            || proof.Algorithm != "ES256"
            || !string.Equals(
                proof.KeyId,
                request.Device.InstallationPublicKeyHash,
                StringComparison.Ordinal))
        {
            return false;
        }

        string envelopeHash = ComputeEnvelopeHash(consent);
        if (!string.Equals(envelopeHash, consent.ConsentEnvelopeHash, StringComparison.Ordinal)
            || !string.Equals(proof.SignedPayloadHash, envelopeHash, StringComparison.Ordinal))
        {
            return false;
        }

        return InstallationPublicKey.TryParse(
            request.Device.InstallationPublicKey,
            out InstallationPublicKey? key)
            && key is not null
            && string.Equals(
                key.Hash,
                request.Device.InstallationPublicKeyHash,
                StringComparison.Ordinal)
            && key.VerifyConsentSignature(envelopeHash, proof.Signature);
    }

    /// <summary>
    /// Recomputes the canonical consent-envelope hash: the RFC 8785
    /// canonical JSON (UTF-16 lexical key order) of the envelope draft —
    /// every consent field except <c>consentEnvelopeHash</c> and
    /// <c>proof</c> — prefixed with the purpose preimage.
    /// </summary>
    internal static string ComputeEnvelopeHash(ModelContextConsent consent)
    {
        SortedDictionary<string, object> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = consent.SchemaVersion,
            ["decisionId"] = consent.DecisionId,
            ["requestId"] = consent.RequestId,
            ["invocationId"] = consent.InvocationId,
            ["deliveryModel"] = consent.DeliveryModel,
            ["tenantHash"] = consent.TenantHash,
            ["subjectHash"] = consent.SubjectHash,
            ["registrationId"] = consent.RegistrationId,
            ["installationPublicKeyHash"] = consent.InstallationPublicKeyHash,
            ["entitlementLeaseId"] = consent.EntitlementLeaseId,
            ["entitlementLeaseHash"] = consent.EntitlementLeaseHash,
            ["tenantPolicyId"] = consent.TenantPolicyId,
            ["tenantPolicyVersion"] = consent.TenantPolicyVersion,
            ["tenantPolicyHash"] = consent.TenantPolicyHash,
            ["purpose"] = consent.Purpose,
            ["modelRole"] = consent.ModelRole,
            ["canonicalOutputSchemaId"] = consent.CanonicalOutputSchemaId,
            ["canonicalOutputSchemaHash"] = consent.CanonicalOutputSchemaHash,
            ["manifestHash"] = consent.ManifestHash,
            ["invocationBindingHash"] = consent.InvocationBindingHash,
            ["consumptionHash"] = consent.ConsumptionHash,
            ["consentDisclosureHash"] = consent.ConsentDisclosureHash,
            ["providerProfileHash"] = consent.ProviderProfileHash,
            ["modelProfileHash"] = consent.ModelProfileHash,
            ["modelCapabilityHash"] = consent.ModelCapabilityHash,
            ["deploymentHash"] = consent.DeploymentHash,
            ["region"] = consent.Region,
            ["retentionMode"] = consent.RetentionMode,
            ["budgetClass"] = consent.BudgetClass,
            ["issuedAt"] = RenderInstant(consent.IssuedAt),
            ["notBefore"] = RenderInstant(consent.NotBefore),
            ["expiresAt"] = RenderInstant(consent.ExpiresAt),
            ["nonceHash"] = consent.NonceHash,
        };

        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            writer.WriteStartObject();
            foreach ((string name, object value) in draft)
            {
                switch (value)
                {
                    case string text:
                        writer.WriteString(name, text);
                        break;
                    case long number:
                        writer.WriteNumber(name, number);
                        break;
                    default:
                        throw new InvalidOperationException(
                            "Unsupported consent draft value type.");
                }
            }
            writer.WriteEndObject();
            writer.Flush();
        }

        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes(HashPurposePreimage));
        hash.AppendData(buffer.WrittenSpan);
        return "sha256:" + Convert.ToHexStringLower(hash.GetHashAndReset());
    }

    private static string RenderInstant(DateTimeOffset value) =>
        value.ToUniversalTime().ToString(
            "yyyy-MM-dd'T'HH:mm:ss.fff'Z'",
            System.Globalization.CultureInfo.InvariantCulture);
}

/// <summary>
/// Verifies that a version two consent envelope was signed by the registered
/// installation key, and that the decision it carries is the decision for the
/// request it is presented with.
///
/// <para>
/// It lives beside <see cref="InstallationConsentVerifier"/> rather than
/// replacing it. A version one decision has a different signed shape — it
/// carries a deployment hash and no profile, catalog epoch, variant, request
/// mode, or context snapshot — so the two drafts are different documents and
/// must keep their own preimages. Historical version one consent, its
/// cross-language golden vector, and its verifier are untouched.
/// </para>
/// <para>
/// Two independent things are checked, and both must hold. The envelope hash is
/// recomputed over every signed field and the installation ES256 proof is
/// verified against it exactly as version one does, so editing any field of the
/// decision invalidates the signature. Then the seven fields the decision and
/// the request both carry — request mode, profile, requested variant, catalog
/// epoch, context snapshot, canonical output schema, and the profile and
/// capability hashes — are compared, so presenting a validly signed decision
/// alongside a different request is rejected here and not only by the admission
/// guard.
/// </para>
/// <para>
/// One binding is deliberately weaker than version one's and is recorded rather
/// than hidden: <c>manifestHash</c> is compared to the request's
/// <c>localEgressManifestHash</c> but cannot be recomputed, because no
/// canonical preimage for a version two local egress manifest exists in either
/// language yet (see the <c>V2-EGRESS-MANIFEST-PREIMAGE</c> gap). That one
/// check is therefore a client-consistency check until the cross-language
/// preimage is defined; every other binding above is server-verified.
/// </para>
/// <para>
/// All failures collapse into <see cref="ContextConsentVerification.Rejected"/>
/// — no cryptographic diagnostics leave this type.
/// </para>
/// </summary>
public sealed class InstallationModelStreamConsentVerifier(TimeProvider timeProvider)
    : IModelStreamConsentVerifier
{
    /// <summary>
    /// Domain separation for the version two envelope. It differs from the
    /// version one purpose, so no version one draft can ever hash to a version
    /// two envelope or the reverse even if the two drafts were byte-identical.
    /// </summary>
    private const string HashPurposePreimage = "sapphirus:model-context-consent:v2\n";

    public ValueTask<ContextConsentVerification> VerifyAsync(
        string subject,
        RegisteredDevice device,
        ModelAccessRequestV2 request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(device);
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        // The subject partition is authorization context the caller already
        // resolved; the decision binds the registration and the installation
        // key, which is what this type verifies.
        _ = subject;
        return ValueTask.FromResult(
            Verify(device, request)
                ? ContextConsentVerification.Verified
                : ContextConsentVerification.Rejected);
    }

    private bool Verify(RegisteredDevice device, ModelAccessRequestV2 request)
    {
        ModelContextConsentV2 consent = request.Consent;
        if (consent is null || consent.Proof is null)
        {
            return false;
        }
        ModelContextConsentProof proof = consent.Proof;
        DateTimeOffset now = timeProvider.GetUtcNow();
        if (consent.SchemaVersion != ModelContextConsentV2.SchemaVersionValue
            || !string.Equals(
                consent.RegistrationId,
                device.RegistrationId,
                StringComparison.Ordinal)
            || !string.Equals(
                consent.InstallationPublicKeyHash,
                device.InstallationPublicKeyHash,
                StringComparison.Ordinal)
            || now < consent.NotBefore
            || now >= consent.ExpiresAt
            || proof.ProofType != "installation_signature"
            || proof.Algorithm != "ES256"
            || !string.Equals(
                proof.KeyId,
                device.InstallationPublicKeyHash,
                StringComparison.Ordinal))
        {
            return false;
        }
        if (!BindsTheRequest(consent, request))
        {
            return false;
        }

        string envelopeHash = ComputeEnvelopeHash(consent);
        if (!string.Equals(envelopeHash, consent.ConsentEnvelopeHash, StringComparison.Ordinal)
            || !string.Equals(proof.SignedPayloadHash, envelopeHash, StringComparison.Ordinal))
        {
            return false;
        }

        return InstallationPublicKey.TryParse(
            device.InstallationPublicKey,
            out InstallationPublicKey? key)
            && key is not null
            && string.Equals(
                key.Hash,
                device.InstallationPublicKeyHash,
                StringComparison.Ordinal)
            && key.VerifyConsentSignature(envelopeHash, proof.Signature);
    }

    /// <summary>
    /// Every field the decision and the request both carry must agree. Changing
    /// the request mode, the profile, the requested variant, the catalog epoch,
    /// the context snapshot, the canonical output schema, or either profile hash
    /// therefore produces a decision that no longer authorizes the request it
    /// arrived with, whether or not the decision itself was tampered with.
    /// </summary>
    private static bool BindsTheRequest(
        ModelContextConsentV2 consent,
        ModelAccessRequestV2 request) =>
        string.Equals(consent.RequestId, request.RequestId, StringComparison.Ordinal)
        && string.Equals(
            consent.RegistrationId,
            request.RegistrationId,
            StringComparison.Ordinal)
        && string.Equals(consent.DeliveryModel, request.DeliveryModel, StringComparison.Ordinal)
        && string.Equals(consent.Purpose, request.Purpose, StringComparison.Ordinal)
        && string.Equals(consent.ModelRole, request.ModelRole, StringComparison.Ordinal)
        && string.Equals(consent.RequestMode, request.RequestMode, StringComparison.Ordinal)
        && string.Equals(
            consent.ModelProfileId,
            request.ModelProfileId,
            StringComparison.Ordinal)
        && string.Equals(
            consent.RequestedVariantId,
            request.RequestedVariantId,
            StringComparison.Ordinal)
        && string.Equals(
            consent.CatalogVersion,
            request.CatalogVersion,
            StringComparison.Ordinal)
        && string.Equals(
            consent.ContextSnapshotId,
            request.ContextSnapshotId,
            StringComparison.Ordinal)
        && string.Equals(
            consent.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaId,
            StringComparison.Ordinal)
        && string.Equals(
            consent.CanonicalOutputSchemaHash,
            request.CanonicalOutputSchemaHash,
            StringComparison.Ordinal)
        && string.Equals(
            consent.ModelProfileHash,
            request.ModelProfileHash,
            StringComparison.Ordinal)
        && string.Equals(
            consent.ModelCapabilityHash,
            request.ModelCapabilityHash,
            StringComparison.Ordinal)
        // The one binding that cannot be recomputed server side yet. See the
        // type remarks: the version two egress-manifest preimage is an open
        // cross-language gap, so this stays a consistency check.
        && string.Equals(
            consent.ManifestHash,
            request.LocalEgressManifestHash,
            StringComparison.Ordinal)
        && string.Equals(
            consent.RetentionMode,
            request.RetentionMode,
            StringComparison.Ordinal)
        && string.Equals(consent.BudgetClass, request.BudgetClass, StringComparison.Ordinal);

    /// <summary>
    /// Recomputes the canonical version two consent-envelope hash: the RFC 8785
    /// canonical JSON (UTF-16 lexical key order) of the envelope draft — every
    /// consent field except <c>consentEnvelopeHash</c> and <c>proof</c> —
    /// prefixed with the version two purpose preimage.
    ///
    /// <para>
    /// The three nullable members are written as JSON <c>null</c> rather than
    /// omitted, so a decision that names no variant and one that names the
    /// literal string "null" are different preimages.
    /// </para>
    /// <para>
    /// Adding a field to <see cref="ModelContextConsentV2"/> without adding it
    /// here would leave that field unsigned, so the record, this draft, and
    /// <c>model-access-request-v2.schema.json</c> must move together.
    /// </para>
    /// </summary>
    internal static string ComputeEnvelopeHash(ModelContextConsentV2 consent)
    {
        ArgumentNullException.ThrowIfNull(consent);
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["schemaVersion"] = consent.SchemaVersion,
            ["decisionId"] = consent.DecisionId,
            ["requestId"] = consent.RequestId,
            ["invocationId"] = consent.InvocationId,
            ["deliveryModel"] = consent.DeliveryModel,
            ["tenantHash"] = consent.TenantHash,
            ["subjectHash"] = consent.SubjectHash,
            ["registrationId"] = consent.RegistrationId,
            ["installationPublicKeyHash"] = consent.InstallationPublicKeyHash,
            ["entitlementLeaseId"] = consent.EntitlementLeaseId,
            ["entitlementLeaseHash"] = consent.EntitlementLeaseHash,
            ["tenantPolicyId"] = consent.TenantPolicyId,
            ["tenantPolicyVersion"] = consent.TenantPolicyVersion,
            ["tenantPolicyHash"] = consent.TenantPolicyHash,
            ["purpose"] = consent.Purpose,
            ["modelRole"] = consent.ModelRole,
            ["requestMode"] = consent.RequestMode,
            ["modelProfileId"] = consent.ModelProfileId,
            ["requestedVariantId"] = consent.RequestedVariantId,
            ["catalogVersion"] = consent.CatalogVersion,
            ["contextSnapshotId"] = consent.ContextSnapshotId,
            ["canonicalOutputSchemaId"] = consent.CanonicalOutputSchemaId,
            ["canonicalOutputSchemaHash"] = consent.CanonicalOutputSchemaHash,
            ["manifestHash"] = consent.ManifestHash,
            ["invocationBindingHash"] = consent.InvocationBindingHash,
            ["consumptionHash"] = consent.ConsumptionHash,
            ["consentDisclosureHash"] = consent.ConsentDisclosureHash,
            ["providerProfileHash"] = consent.ProviderProfileHash,
            ["modelProfileHash"] = consent.ModelProfileHash,
            ["modelCapabilityHash"] = consent.ModelCapabilityHash,
            ["region"] = consent.Region,
            ["retentionMode"] = consent.RetentionMode,
            ["budgetClass"] = consent.BudgetClass,
            ["issuedAt"] = RenderInstant(consent.IssuedAt),
            ["notBefore"] = RenderInstant(consent.NotBefore),
            ["expiresAt"] = RenderInstant(consent.ExpiresAt),
            ["nonceHash"] = consent.NonceHash,
        };

        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            writer.WriteStartObject();
            foreach ((string name, object? value) in draft)
            {
                switch (value)
                {
                    case null:
                        writer.WriteNull(name);
                        break;
                    case string text:
                        writer.WriteString(name, text);
                        break;
                    case long number:
                        writer.WriteNumber(name, number);
                        break;
                    default:
                        throw new InvalidOperationException(
                            "Unsupported consent draft value type.");
                }
            }
            writer.WriteEndObject();
            writer.Flush();
        }

        using IncrementalHash hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        hash.AppendData(Encoding.UTF8.GetBytes(HashPurposePreimage));
        hash.AppendData(buffer.WrittenSpan);
        return "sha256:" + Convert.ToHexStringLower(hash.GetHashAndReset());
    }

    private static string RenderInstant(DateTimeOffset value) =>
        value.ToUniversalTime().ToString(
            "yyyy-MM-dd'T'HH:mm:ss.fff'Z'",
            System.Globalization.CultureInfo.InvariantCulture);
}
