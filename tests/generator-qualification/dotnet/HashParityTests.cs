using System.Text;
using System.Text.Json;
using Xunit;

namespace Sapphirus.GeneratorQualification.Tests;

public sealed class HashParityTests
{
    [Fact]
    public void RequiredAndSupplementalVectorsMatchRfc8785AndPurposeSeparatedHashes()
    {
        using JsonDocument document = JsonDocument.Parse(
            File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "hash-vectors.json")));

        IEnumerable<JsonElement> vectors = document.RootElement
            .GetProperty("required"u8)
            .EnumerateArray()
            .Concat(document.RootElement.GetProperty("supplemental"u8).EnumerateArray());

        int count = 0;
        foreach (JsonElement vector in vectors)
        {
            count++;
            string canonical = CanonicalJson.Serialize(vector.GetProperty("value"u8));
            Assert.Equal(vector.GetProperty("canonicalJson"u8).GetString(), canonical);

            string hash = CanonicalJson.Hash(
                vector.GetProperty("purpose"u8).GetString()!,
                vector.GetProperty("schemaMajor"u8).GetString()!,
                vector.GetProperty("value"u8));
            Assert.Equal(vector.GetProperty("expectedHash"u8).GetString(), hash);
        }

        Assert.Equal(5, count);
    }

    [Fact]
    public void NfcAndNfdQualificationDocumentsRemainByteAndHashDistinct()
    {
        using JsonDocument nfc = JsonDocument.Parse(
            File.ReadAllBytes(QualificationPath("fixtures/valid/unicode-nfc.json")));
        using JsonDocument nfd = JsonDocument.Parse(
            File.ReadAllBytes(QualificationPath("fixtures/valid/unicode-nfd.json")));

        string nfcLabel = nfc.RootElement.GetProperty("label"u8).GetString()!;
        string nfdLabel = nfd.RootElement.GetProperty("label"u8).GetString()!;
        Assert.NotEqual(nfcLabel, nfdLabel);
        Assert.Equal(nfcLabel.Normalize(NormalizationForm.FormD), nfdLabel);

        string nfcCanonical = CanonicalJson.Serialize(nfc.RootElement);
        string nfdCanonical = CanonicalJson.Serialize(nfd.RootElement);
        Assert.NotEqual(nfcCanonical, nfdCanonical);
        Assert.NotEqual(
            CanonicalJson.Hash("contract-object", "v1", nfc.RootElement),
            CanonicalJson.Hash("contract-object", "v1", nfd.RootElement));
    }

    private static string QualificationPath(string relativePath) =>
        Path.Combine(
            AppContext.BaseDirectory,
            "qualification",
            relativePath.Replace('/', Path.DirectorySeparatorChar));
}
