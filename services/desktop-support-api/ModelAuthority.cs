namespace Sapphirus.DesktopSupportApi;

public interface ISignedPolicyService
{
    Task<SignedDesktopPolicy> CurrentPolicyAsync(CancellationToken cancellationToken);
    Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken);
}

public interface IModelAccessBroker
{
    Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken);
}

public sealed record UnsignedModelAccessResult(
    string OutputSchemaId,
    string PayloadJson,
    string PayloadHash,
    string SchemaProjectionHash,
    string CredentialBindingHash,
    ModelAccessUsage Usage,
    int RetryCount,
    ModelFallbackEvent[] FallbackEvents,
    string? ProviderRequestId,
    DateTimeOffset StartedAt,
    DateTimeOffset CompletedAt,
    string TerminalStatus);

public interface IModelReceiptSigner
{
    Task<ModelAccessResult> SignAsync(
        string subject,
        ModelAccessRequest request,
        UnsignedModelAccessResult result,
        CancellationToken cancellationToken);
}

public enum ContextConsentVerification
{
    Verified,
    Rejected,
    Unavailable,
}

public sealed record ContextConsentVerificationRequest(
    string Subject,
    RegisteredDevice Device,
    ModelAccessRequest Request,
    string RecomputedManifestHash);

public interface IContextConsentVerifier
{
    ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The canonical consent envelope is present and structurally bound to the exact request. The
/// default still fails closed because installation-key signature verification and durable
/// single-use consumption have no production provider until the external key resources exist.
/// </summary>
public sealed class UnavailableContextConsentVerifier : IContextConsentVerifier
{
    public ValueTask<ContextConsentVerification> VerifyAsync(
        ContextConsentVerificationRequest request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(ContextConsentVerification.Unavailable);
    }
}
