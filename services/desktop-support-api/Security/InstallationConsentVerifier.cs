using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;

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
