using Azure.Security.KeyVault.Keys.Cryptography;

namespace Sapphirus.DesktopSupportApi.Signing;

/// <summary>
/// Signs canonical SHA-256 digests, emitting base64url raw r||s ES256
/// signatures. The key id must pin the exact immutable key version.
/// </summary>
public interface IHashSigner
{
    /// <summary>The immutable, versioned identifier of the signing key.</summary>
    string KeyId { get; }

    Task<string> SignAsync(byte[] sha256Digest, CancellationToken cancellationToken);
}

/// <summary>
/// The rotation policy for proof keys: exactly one active signing key plus
/// an explicit verification-only overlap. Anything else — unknown, disabled,
/// or retired keys — is outside policy.
/// </summary>
public sealed record SigningKeyRing(
    string ActiveKeyId,
    IReadOnlyList<string> VerificationOnlyKeyIds)
{
    public bool IsAcceptableForVerification(string? keyId) =>
        keyId is not null
        && (string.Equals(keyId, ActiveKeyId, StringComparison.Ordinal)
            || VerificationOnlyKeyIds.Contains(keyId, StringComparer.Ordinal));
}

/// <summary>
/// Production signer over a non-exportable P-256 Key Vault key. Any Key
/// Vault failure (timeout, throttle, unavailability) propagates — no
/// unsigned artifact can be produced from this type.
/// </summary>
public sealed class KeyVaultHashSigner : IHashSigner
{
    private readonly CryptographyClient _client;

    public KeyVaultHashSigner(CryptographyClient client, string versionedKeyId)
    {
        ArgumentNullException.ThrowIfNull(client);
        ArgumentException.ThrowIfNullOrWhiteSpace(versionedKeyId);
        _client = client;
        KeyId = versionedKeyId;
    }

    public string KeyId { get; }

    public async Task<string> SignAsync(
        byte[] sha256Digest,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(sha256Digest);
        if (sha256Digest.Length != 32)
        {
            throw new ArgumentException(
                "ES256 signs exactly one SHA-256 digest.",
                nameof(sha256Digest));
        }
        SignResult result = await _client
            .SignAsync(SignatureAlgorithm.ES256, sha256Digest, cancellationToken)
            .ConfigureAwait(false);
        if (result.Signature is not { Length: 64 })
        {
            throw new InvalidOperationException(
                "The Key Vault signature had an unexpected encoding.");
        }
        return Base64Url(result.Signature);
    }

    internal static string Base64Url(byte[] bytes) =>
        Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');
}
