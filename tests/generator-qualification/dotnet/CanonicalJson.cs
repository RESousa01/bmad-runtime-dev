using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Sapphirus.GeneratorQualification.Tests;

public static class CanonicalJson
{
    public static string Serialize(JsonElement value)
    {
        var builder = new StringBuilder();
        AppendValue(builder, value);
        return builder.ToString();
    }

    public static string Hash(string purpose, string schemaMajor, JsonElement value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(purpose);
        ArgumentException.ThrowIfNullOrWhiteSpace(schemaMajor);
        string canonical = Serialize(value);
        byte[] preimage = Encoding.UTF8.GetBytes($"sapphirus:{purpose}:{schemaMajor}\n{canonical}");
        return $"sha256:{Convert.ToHexString(SHA256.HashData(preimage)).ToLowerInvariant()}";
    }

    private static void AppendValue(StringBuilder builder, JsonElement value)
    {
        switch (value.ValueKind)
        {
            case JsonValueKind.Object:
                AppendObject(builder, value);
                break;
            case JsonValueKind.Array:
                AppendArray(builder, value);
                break;
            case JsonValueKind.String:
                AppendString(builder, value.GetString()!);
                break;
            case JsonValueKind.Number:
                builder.Append(FormatNumber(value.GetDouble()));
                break;
            case JsonValueKind.True:
                builder.Append("true");
                break;
            case JsonValueKind.False:
                builder.Append("false");
                break;
            case JsonValueKind.Null:
                builder.Append("null");
                break;
            default:
                throw new InvalidOperationException($"Unsupported JSON kind {value.ValueKind}.");
        }
    }

    private static void AppendObject(StringBuilder builder, JsonElement value)
    {
        JsonProperty[] properties = value
            .EnumerateObject()
            .OrderBy(static property => property.Name, StringComparer.Ordinal)
            .ToArray();
        builder.Append('{');
        for (int index = 0; index < properties.Length; index++)
        {
            if (index > 0)
            {
                builder.Append(',');
            }

            AppendString(builder, properties[index].Name);
            builder.Append(':');
            AppendValue(builder, properties[index].Value);
        }

        builder.Append('}');
    }

    private static void AppendArray(StringBuilder builder, JsonElement value)
    {
        builder.Append('[');
        int index = 0;
        foreach (JsonElement item in value.EnumerateArray())
        {
            if (index++ > 0)
            {
                builder.Append(',');
            }

            AppendValue(builder, item);
        }

        builder.Append(']');
    }

    private static void AppendString(StringBuilder builder, string value)
    {
        builder.Append('"');
        foreach (Rune rune in value.EnumerateRunes())
        {
            switch (rune.Value)
            {
                case '"':
                    builder.Append("\\\"");
                    break;
                case '\\':
                    builder.Append("\\\\");
                    break;
                case '\b':
                    builder.Append("\\b");
                    break;
                case '\t':
                    builder.Append("\\t");
                    break;
                case '\n':
                    builder.Append("\\n");
                    break;
                case '\f':
                    builder.Append("\\f");
                    break;
                case '\r':
                    builder.Append("\\r");
                    break;
                default:
                    if (rune.Value <= 0x1f)
                    {
                        builder.Append("\\u");
                        builder.Append(rune.Value.ToString("x4", CultureInfo.InvariantCulture));
                    }
                    else
                    {
                        builder.Append(rune.ToString());
                    }

                    break;
            }
        }

        builder.Append('"');
    }

    private static string FormatNumber(double value)
    {
        if (!double.IsFinite(value))
        {
            throw new InvalidOperationException("RFC 8785 forbids non-finite numbers.");
        }

        if (value == 0)
        {
            return "0";
        }

        bool negative = value < 0;
        string roundTrip = Math.Abs(value).ToString("R", CultureInfo.InvariantCulture);
        int exponentMarker = roundTrip.IndexOfAny(['E', 'e']);
        string mantissa = exponentMarker >= 0 ? roundTrip[..exponentMarker] : roundTrip;
        int explicitExponent = exponentMarker >= 0
            ? int.Parse(roundTrip[(exponentMarker + 1)..], CultureInfo.InvariantCulture)
            : 0;
        int decimalPoint = mantissa.IndexOf('.');
        int fractionalDigits = decimalPoint >= 0 ? mantissa.Length - decimalPoint - 1 : 0;
        string digits = decimalPoint >= 0 ? mantissa.Remove(decimalPoint, 1) : mantissa;
        int point = digits.Length + explicitExponent - fractionalDigits;
        int scientificExponent = point - 1;

        string formatted;
        if (scientificExponent >= -6 && scientificExponent < 21)
        {
            if (point <= 0)
            {
                formatted = $"0.{new string('0', -point)}{digits}";
            }
            else if (point >= digits.Length)
            {
                formatted = digits + new string('0', point - digits.Length);
            }
            else
            {
                formatted = digits.Insert(point, ".");
            }
        }
        else
        {
            string fraction = digits.Length > 1 ? $".{digits[1..]}" : string.Empty;
            string exponentSign = scientificExponent >= 0 ? "+" : string.Empty;
            formatted = $"{digits[0]}{fraction}e{exponentSign}{scientificExponent}";
        }

        return negative ? $"-{formatted}" : formatted;
    }
}
