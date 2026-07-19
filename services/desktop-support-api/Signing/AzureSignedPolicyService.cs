using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Signing;

/// <summary>
/// Signs policies and entitlement leases with the vault-held proof key.
/// Signing runs only after the snapshot and lease pass semantic validation;
/// a signer failure yields no artifact at all.
/// </summary>
public sealed class AzureSignedPolicyService(
    AppConfigurationPolicyProvider policyProvider,
    IHashSigner policySigner,
    TimeProvider timeProvider) : ISignedPolicyService
{
    public async Task<SignedDesktopPolicy> CurrentPolicyAsync(
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        PolicySnapshot snapshot = await policyProvider
            .GetSnapshotAsync(cancellationToken)
            .ConfigureAwait(false);
        (SignedDesktopPolicy unsigned, byte[] digest) = CanonicalPolicyProjector
            .Project(snapshot);
        string signature = await policySigner
            .SignAsync(digest, cancellationToken)
            .ConfigureAwait(false);
        return unsigned with
        {
            KeyId = policySigner.KeyId,
            Signature = signature,
        };
    }

    public async Task<SignedEntitlementLease> CreateLeaseAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        if (!RequestGuards.IsRegistrationId(registrationId))
        {
            throw new ArgumentException(
                "The registration identifier is invalid.",
                nameof(registrationId));
        }
        PolicySnapshot snapshot = await policyProvider
            .GetSnapshotAsync(cancellationToken)
            .ConfigureAwait(false);
        DateTimeOffset now = timeProvider.GetUtcNow();
        SignedEntitlementLease unsigned = new(
            "desktop-entitlement-lease.v1",
            ContractIds.FromEntropy("lease", RandomNumberGenerator.GetBytes(16)),
            registrationId,
            Hash(subject),
            "windows_local",
            now,
            now.AddMinutes(-2),
            now.AddHours(24),
            now.AddHours(96),
            ["local_runtime", "model_access"],
            CanonicalPolicyProjector.Project(snapshot).UnsignedPolicy.PolicyHash,
            "0.1.0-beta.1",
            "",
            "");
        (_, byte[] digest) = CanonicalPolicyProjector.ProjectLease(unsigned);
        string signature = await policySigner
            .SignAsync(digest, cancellationToken)
            .ConfigureAwait(false);
        return unsigned with
        {
            KeyId = policySigner.KeyId,
            Signature = signature,
        };
    }

    private static string Hash(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
