using System.Security.Cryptography;

namespace Sapphirus.DesktopSupportApi.Security;

/// <summary>
/// A strictly validated installation public key: base64url DER
/// SubjectPublicKeyInfo carrying exactly one uncompressed NIST P-256 point.
/// String projections expose only the key hash — never key material.
/// </summary>
public sealed class InstallationPublicKey
{
    private readonly byte[] _subjectPublicKeyInfo;

    private InstallationPublicKey(byte[] subjectPublicKeyInfo, string hash)
    {
        _subjectPublicKeyInfo = subjectPublicKeyInfo;
        Hash = hash;
    }

    /// <summary>The sha256:&lt;hex&gt; hash of the DER SubjectPublicKeyInfo.</summary>
    public string Hash { get; }

    public static bool TryParse(string? encoded, out InstallationPublicKey? key)
    {
        key = null;
        if (encoded is null
            || encoded.Length is < 80 or > 512
            || encoded.Any(character => !char.IsAsciiLetterOrDigit(character)
                && character is not '-' and not '_'))
        {
            return false;
        }
        try
        {
            byte[] subjectPublicKeyInfo = DecodeBase64Url(encoded);
            using ECDsa candidate = ECDsa.Create();
            candidate.ImportSubjectPublicKeyInfo(subjectPublicKeyInfo, out int bytesRead);
            ECParameters parameters = candidate.ExportParameters(false);
            if (bytesRead != subjectPublicKeyInfo.Length
                || parameters.Curve.Oid.Value != ECCurve.NamedCurves.nistP256.Oid.Value
                || parameters.Q.X is not { Length: 32 }
                || parameters.Q.Y is not { Length: 32 })
            {
                return false;
            }
            string hash = "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(subjectPublicKeyInfo));
            key = new InstallationPublicKey(subjectPublicKeyInfo, hash);
            return true;
        }
        catch (CryptographicException)
        {
            return false;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    /// <summary>
    /// Verifies an ES256 consent signature: raw 64-byte r||s, base64url
    /// without padding, over the ASCII bytes of the envelope-hash string.
    /// </summary>
    public bool VerifyConsentSignature(string envelopeHash, string signature)
    {
        if (!RequestGuards.IsSha256(envelopeHash) || string.IsNullOrEmpty(signature))
        {
            return false;
        }
        try
        {
            byte[] signatureBytes = DecodeBase64Url(signature);
            if (signatureBytes.Length != 64)
            {
                return false;
            }
            using ECDsa key = ECDsa.Create();
            key.ImportSubjectPublicKeyInfo(_subjectPublicKeyInfo, out _);
            return key.VerifyData(
                System.Text.Encoding.ASCII.GetBytes(envelopeHash),
                signatureBytes,
                HashAlgorithmName.SHA256,
                DSASignatureFormat.IeeeP1363FixedFieldConcatenation);
        }
        catch (CryptographicException)
        {
            return false;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    public override string ToString() => $"installation-key({Hash})";

    internal static byte[] DecodeBase64Url(string value)
    {
        string padded = value.Replace('-', '+').Replace('_', '/');
        padded += (padded.Length % 4) switch
        {
            0 => "",
            2 => "==",
            3 => "=",
            _ => throw new FormatException("Invalid base64url length."),
        };
        return Convert.FromBase64String(padded);
    }
}
