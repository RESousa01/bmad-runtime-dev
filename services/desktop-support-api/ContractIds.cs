using System.Numerics;

namespace Sapphirus.DesktopSupportApi;

internal static class ContractIds
{
    private const string Alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    public static string FromEntropy(string prefix, ReadOnlySpan<byte> entropy)
    {
        if (entropy.Length < 16)
        {
            throw new ArgumentException("Contract identifiers require at least 128 bits.", nameof(entropy));
        }
        BigInteger value = new(entropy[..16], isUnsigned: true, isBigEndian: true);
        Span<char> suffix = stackalloc char[26];
        for (int index = suffix.Length - 1; index >= 0; index--)
        {
            suffix[index] = Alphabet[(int)(value & 31)];
            value >>= 5;
        }
        return $"{prefix}_{new string(suffix)}";
    }

    public static bool Is(string? value, string prefix)
    {
        string marker = prefix + "_";
        if (value is null
            || !value.StartsWith(marker, StringComparison.Ordinal)
            || value.Length - marker.Length is < 16 or > 64)
        {
            return false;
        }
        return value.AsSpan(marker.Length).ToArray().All(Alphabet.Contains);
    }
}
