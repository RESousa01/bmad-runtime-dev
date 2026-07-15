using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Corvus.Text.Json;
using Sapphirus.Contracts.Generated;
using Xunit;
using JsonDocument = System.Text.Json.JsonDocument;
using JsonElement = System.Text.Json.JsonElement;

namespace Sapphirus.Contracts.Conformance.Tests;

public sealed class BmadContractConformanceTests
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    [Fact]
    public void EveryBmadFixtureHasTheSameDotnetReasonCategory()
    {
        FixtureEntry[] catalog = JsonSerializer.Deserialize<FixtureEntry[]>(
            File.ReadAllText(FixturePath("catalog.json")),
            JsonOptions)
            ?? throw new InvalidOperationException("Fixture catalog did not deserialize.");
        FixtureEntry[] entries = catalog
            .Where(static entry => entry.File.Contains("/bmad/", StringComparison.Ordinal))
            .ToArray();
        Assert.Equal(103, entries.Length);

        foreach (FixtureEntry entry in entries)
        {
            byte[] source = File.ReadAllBytes(FixturePath(entry.File));
            if (entry.ReasonCode == "DUPLICATE_MEMBER")
            {
                BmadStrictJsonException exception = Assert.Throws<BmadStrictJsonException>(
                    () => StrictBmadJson.Parse(source));
                Assert.Equal(entry.ReasonCode, exception.Code);
                Assert.True(
                    entry.ReasonCodes.SequenceEqual([exception.Code], StringComparer.Ordinal),
                    entry.File);
                continue;
            }

            using JsonDocument document = StrictBmadJson.Parse(source);
            string? structuralReason = ValidateGenerated(entry.Schema!, source);
            if (structuralReason is not null)
            {
                Assert.False(entry.Valid);
                Assert.True(
                    StringComparer.Ordinal.Equals(entry.ReasonCode, structuralReason),
                    $"{entry.File}: expected {entry.ReasonCode}, got {structuralReason}.");
                Assert.True(
                    entry.ReasonCodes.SequenceEqual([structuralReason], StringComparer.Ordinal),
                    entry.File);
                continue;
            }

            using JsonDocument? descriptor = entry.ContextFile is null
                ? null
                : StrictBmadJson.Parse(File.ReadAllBytes(FixturePath(entry.ContextFile)));
            IReadOnlyList<string> semantic = BmadSemantics.Validate(
                document.RootElement,
                descriptor?.RootElement);
            if (entry.Valid)
            {
                Assert.True(semantic.Count == 0, $"{entry.File}: {string.Join(",", semantic)}");
            }
            else
            {
                Assert.Equal(entry.ReasonCodes, semantic);
            }
        }
    }

    [Fact]
    public void DotnetMatchesAllEightBmadGoldenHashVectors()
    {
        using JsonDocument document = StrictBmadJson.Parse(
            File.ReadAllBytes(FixturePath("golden/bmad/hash-vectors.json")));
        JsonElement[] vectors = document.RootElement.GetProperty("vectors"u8)
            .EnumerateArray()
            .ToArray();
        Assert.Equal(8, vectors.Length);
        foreach (JsonElement vector in vectors)
        {
            string name = vector.GetProperty("name"u8).GetString()!;
            string purpose = vector.GetProperty("purpose"u8).GetString()!;
            string schemaMajor = vector.GetProperty("schemaMajor"u8).GetString()!;
            string excludedField = vector.GetProperty("excludedFields"u8)[0].GetString()!;
            JsonElement value = vector.GetProperty("value"u8);
            string actual = BmadCanonicalJson.HashWithoutField(
                purpose,
                schemaMajor,
                value,
                excludedField);
            Assert.True(
                StringComparer.Ordinal.Equals(
                    vector.GetProperty("expectedHash"u8).GetString(),
                    actual),
                name);
            Assert.True(
                StringComparer.Ordinal.Equals(
                    vector.GetProperty("canonicalJson"u8).GetString(),
                    BmadCanonicalJson.Serialize(value, excludedField)),
                name);

            JsonObject excludedMutation = JsonNode.Parse(value.GetRawText())!.AsObject();
            excludedMutation[excludedField] = $"sha256:{new string('f', 64)}";
            using JsonDocument excludedDocument = JsonDocument.Parse(excludedMutation.ToJsonString());
            Assert.Equal(
                actual,
                BmadCanonicalJson.HashWithoutField(
                    purpose,
                    schemaMajor,
                    excludedDocument.RootElement,
                    excludedField));

            JsonObject includedMutation = JsonNode.Parse(value.GetRawText())!.AsObject();
            includedMutation["schemaVersion"] = "transplanted.v1";
            using JsonDocument includedDocument = JsonDocument.Parse(includedMutation.ToJsonString());
            Assert.NotEqual(
                actual,
                BmadCanonicalJson.HashWithoutField(
                    purpose,
                    schemaMajor,
                    includedDocument.RootElement,
                    excludedField));
        }
    }

    [Fact]
    public void EveryValidBmadRootRoundTripsThroughGeneratedDotnetTypes()
    {
        FixtureEntry[] entries = JsonSerializer.Deserialize<FixtureEntry[]>(
            File.ReadAllText(FixturePath("catalog.json")),
            JsonOptions) ?? throw new InvalidOperationException("Fixture catalog did not deserialize.");
        foreach (FixtureEntry entry in entries.Where(static entry =>
            entry.Valid && entry.File.Contains("/bmad/", StringComparison.Ordinal)))
        {
            byte[] source = File.ReadAllBytes(FixturePath(entry.File));
            using JsonDocument original = StrictBmadJson.Parse(source);
            byte[] serialized = RoundTripGenerated(entry.Schema!, source);
            using JsonDocument roundTrip = StrictBmadJson.Parse(serialized);
            Assert.True(
                JsonElement.DeepEquals(original.RootElement, roundTrip.RootElement),
                entry.File);
        }
    }

    [Fact]
    public void DotnetSealedHelpSemanticsRejectUnsafeTextHashDriftAndInvalidInstants()
    {
        using JsonDocument proposal = StrictBmadJson.Parse(
            File.ReadAllBytes(FixturePath("valid/bmad/method-help-proposal.json")));
        using JsonDocument recommendation = StrictBmadJson.Parse(
            File.ReadAllBytes(FixturePath("valid/bmad/method-help-recommendation.json")));
        using JsonDocument advanceResult = StrictBmadJson.Parse(
            File.ReadAllBytes(FixturePath("valid/bmad/method-advance-result.json")));
        Assert.Empty(BmadSemantics.ValidateMethodHelpProposal(proposal.RootElement));
        Assert.Empty(BmadSemantics.ValidateMethodHelpRecommendation(recommendation.RootElement));
        Assert.Empty(BmadSemantics.ValidateMethodAdvanceResult(advanceResult.RootElement));

        JsonObject unsafeProposal = JsonNode.Parse(proposal.RootElement.GetRawText())!.AsObject();
        unsafeProposal["rationaleSummary"] = "unsafe\u202etext";
        using JsonDocument unsafeProposalDocument = JsonDocument.Parse(unsafeProposal.ToJsonString());
        Assert.Equal(
            ["BMAD_UNSAFE_TEXT"],
            BmadSemantics.ValidateMethodHelpProposal(unsafeProposalDocument.RootElement));

        JsonObject invalidRecommendation =
            JsonNode.Parse(recommendation.RootElement.GetRawText())!.AsObject();
        invalidRecommendation["createdAt"] = "2026-02-31T10:00:00.000Z";
        using JsonDocument invalidRecommendationDocument =
            JsonDocument.Parse(invalidRecommendation.ToJsonString());
        Assert.Equal(
            ["HASH_MISMATCH", "INVALID_UTC_INSTANT"],
            BmadSemantics.ValidateMethodHelpRecommendation(
                invalidRecommendationDocument.RootElement));

        invalidRecommendation["createdAt"] = "0000-02-29T10:00:00.000Z";
        using JsonDocument yearZeroRecommendationDocument =
            JsonDocument.Parse(invalidRecommendation.ToJsonString());
        Assert.DoesNotContain(
            "INVALID_UTC_INSTANT",
            BmadSemantics.ValidateMethodHelpRecommendation(
                yearZeroRecommendationDocument.RootElement));

        JsonObject invalidAdvance = JsonNode.Parse(advanceResult.RootElement.GetRawText())!.AsObject();
        invalidAdvance["resultKind"] = "refusal";
        invalidAdvance["safeMessage"] = "unsafe\u2069text";
        using JsonDocument invalidAdvanceDocument = JsonDocument.Parse(invalidAdvance.ToJsonString());
        Assert.Equal(
            ["BMAD_UNSAFE_TEXT", "HASH_MISMATCH"],
            BmadSemantics.ValidateMethodAdvanceResult(invalidAdvanceDocument.RootElement));
    }

    [Fact]
    public void BmadStrictParserEnforcesExactByteAndDepthLimits()
    {
        byte[] exactLimit = Encoding.UTF8.GetBytes(
            $"\"{new string('a', StrictBmadJson.MaximumBytes - 2)}\"");
        Assert.Equal(StrictBmadJson.MaximumBytes, exactLimit.Length);
        using JsonDocument exactDocument = StrictBmadJson.Parse(exactLimit);

        byte[] multibyteOverLimit = Encoding.UTF8.GetBytes(
            $"\"{new string('é', 1_048_576)}\"");
        BmadStrictJsonException bytes = Assert.Throws<BmadStrictJsonException>(
            () => StrictBmadJson.Parse(multibyteOverLimit));
        Assert.Equal("MAX_BYTES_EXCEEDED", bytes.Code);

        byte[] depthSixteen = Encoding.UTF8.GetBytes(
            $"{new string('[', 16)}null{new string(']', 16)}");
        using JsonDocument depthDocument = StrictBmadJson.Parse(depthSixteen);
        byte[] depthSeventeen = Encoding.UTF8.GetBytes(
            $"{new string('[', 17)}null{new string(']', 17)}");
        BmadStrictJsonException depth = Assert.Throws<BmadStrictJsonException>(
            () => StrictBmadJson.Parse(depthSeventeen));
        Assert.Equal("MAX_DEPTH_EXCEEDED", depth.Code);
    }

    private static byte[] RoundTripGenerated(string schema, ReadOnlySpan<byte> source)
    {
        string serialized = schema switch
        {
            "bmad-package-descriptor.schema.json" =>
                SapphirusContractsCatalog.BmadPackageDescriptor.ParseValue(source).ToString(),
            "bmad-capability-catalog.schema.json" =>
                SapphirusContractsCatalog.BmadCapabilityCatalog.ParseValue(source).ToString(),
            "bmad-method-advance-result.schema.json" =>
                SapphirusContractsCatalog.MethodSessionMethodAdvanceResult.ParseValue(source).ToString(),
            "bmad-method-help-proposal.schema.json" =>
                SapphirusContractsCatalog.MethodHelpProposal.ParseValue(source).ToString(),
            "bmad-method-help-recommendation.schema.json" =>
                SapphirusContractsCatalog.MethodSessionMethodHelpRecommendation.ParseValue(source).ToString(),
            "bmad-method-session.schema.json" =>
                SapphirusContractsCatalog.MethodSession.ParseValue(source).ToString(),
            "bmad-builder-authoring.schema.json" =>
                SapphirusContractsCatalog.BuilderAuthoringObject.ParseValue(source).ToString(),
            "bmad-validation-report.schema.json" =>
                SapphirusContractsCatalog.BmadValidationReport.ParseValue(source).ToString(),
            _ => throw new InvalidOperationException($"Unsupported BMAD schema {schema}."),
        };
        return Encoding.UTF8.GetBytes(serialized);
    }

    private static string? ValidateGenerated(string schema, ReadOnlySpan<byte> source)
    {
        string? expectedRootVersion = schema switch
        {
            "bmad-package-descriptor.schema.json" => "sapphirus.bmad-package-descriptor.v1",
            "bmad-capability-catalog.schema.json" => "sapphirus.bmad-capability-catalog.v1",
            "bmad-validation-report.schema.json" => "sapphirus.bmad-validation-report.v1",
            _ => null,
        };
        if (expectedRootVersion is not null
            && !StringComparer.Ordinal.Equals(ReadRootSchemaVersion(source), expectedRootVersion))
        {
            return "CONST_MISMATCH";
        }
        if (schema == "bmad-capability-catalog.schema.json"
            && HasCapabilityKeyWithoutRequiredNullableAction(source))
        {
            return "REQUIRED_PROPERTY_MISSING";
        }
        if (schema == "bmad-method-session.schema.json"
            && HasMethodAuthorityMismatch(source))
        {
            return "CONST_MISMATCH";
        }

        using JsonSchemaResultsCollector collector = JsonSchemaResultsCollector.CreateUnrented(
            JsonSchemaResultsLevel.Verbose,
            256);
        bool valid = schema switch
        {
            "bmad-package-descriptor.schema.json" =>
                SapphirusContractsCatalog.BmadPackageDescriptor.ParseValue(source).EvaluateSchema(collector),
            "bmad-capability-catalog.schema.json" =>
                SapphirusContractsCatalog.BmadCapabilityCatalog.ParseValue(source).EvaluateSchema(collector),
            "bmad-method-advance-result.schema.json" =>
                SapphirusContractsCatalog.MethodSessionMethodAdvanceResult.ParseValue(source).EvaluateSchema(collector),
            "bmad-method-help-proposal.schema.json" =>
                SapphirusContractsCatalog.MethodHelpProposal.ParseValue(source).EvaluateSchema(collector),
            "bmad-method-help-recommendation.schema.json" =>
                SapphirusContractsCatalog.MethodSessionMethodHelpRecommendation.ParseValue(source).EvaluateSchema(collector),
            "bmad-method-session.schema.json" =>
                SapphirusContractsCatalog.MethodSession.ParseValue(source).EvaluateSchema(collector),
            "bmad-builder-authoring.schema.json" =>
                SapphirusContractsCatalog.BuilderAuthoringObject.ParseValue(source).EvaluateSchema(collector),
            "bmad-validation-report.schema.json" =>
                SapphirusContractsCatalog.BmadValidationReport.ParseValue(source).EvaluateSchema(collector),
            _ => throw new InvalidOperationException($"Unsupported BMAD schema {schema}."),
        };
        if (valid)
        {
            return null;
        }

        (string Keyword, string Reason)[] keywordReasons =
        [
            ("oneOf", "ONE_OF_MISMATCH"),
            ("minItems", "ARRAY_TOO_SHORT"),
            ("maxItems", "ARRAY_TOO_LONG"),
            ("additionalProperties", "UNKNOWN_PROPERTY"),
            ("required", "REQUIRED_PROPERTY_MISSING"),
            ("const", "CONST_MISMATCH"),
            ("enum", "ENUM_MISMATCH"),
            ("pattern", "PATTERN_MISMATCH"),
            ("type", "TYPE_MISMATCH"),
        ];
        HashSet<string> knownKeywords = keywordReasons
            .Select(static mapping => mapping.Keyword)
            .ToHashSet(StringComparer.Ordinal);
        (string Keyword, int Depth)[] failures = collector
            .EnumerateResults()
            .Where(static result => !result.IsMatch)
            .SelectMany(static result =>
            {
                int depth = LocationDepth(result.GetDocumentEvaluationLocationText());
                return new[]
                {
                    (LastSegment(result.GetEvaluationLocationText()), depth),
                    (LastSegment(result.GetSchemaEvaluationLocationText()), depth),
                };
            })
            .Where(failure => knownKeywords.Contains(failure.Item1))
            .ToArray();
        if (failures.Length == 0)
        {
            return "SCHEMA_INVALID";
        }
        if (failures.Any(static failure => failure.Keyword == "oneOf"))
        {
            return "ONE_OF_MISMATCH";
        }
        if (failures.Any(static failure => failure.Keyword == "additionalProperties"))
        {
            return "UNKNOWN_PROPERTY";
        }
        if (failures.Any(static failure => failure.Keyword == "required"))
        {
            return "REQUIRED_PROPERTY_MISSING";
        }

        int shallowestDepth = failures.Min(static failure => failure.Depth);
        HashSet<string> shallowestKeywords = failures
            .Where(failure => failure.Depth == shallowestDepth)
            .Select(static failure => failure.Keyword)
            .ToHashSet(StringComparer.Ordinal);
        foreach ((string Keyword, string Reason) in keywordReasons)
        {
            if (shallowestKeywords.Contains(Keyword))
            {
                return Reason;
            }
        }

        return "SCHEMA_INVALID";
    }

    private static string? ReadRootSchemaVersion(ReadOnlySpan<byte> source)
    {
        using JsonDocument document = JsonDocument.Parse(source.ToArray());
        return document.RootElement.TryGetProperty("schemaVersion"u8, out JsonElement version)
            ? version.GetString()
            : null;
    }

    private static bool HasCapabilityKeyWithoutRequiredNullableAction(ReadOnlySpan<byte> source)
    {
        using JsonDocument document = JsonDocument.Parse(source.ToArray());
        return Visit(document.RootElement);

        static bool Visit(JsonElement value)
        {
            if (value.ValueKind == System.Text.Json.JsonValueKind.Object)
            {
                bool looksLikeCapabilityKey = value.TryGetProperty("packageVersionId"u8, out _)
                    && value.TryGetProperty("moduleCode"u8, out _)
                    && value.TryGetProperty("skillName"u8, out _)
                    && value.EnumerateObject().Count() <= 4;
                if (looksLikeCapabilityKey
                    && !value.TryGetProperty("normalizedAction"u8, out _))
                {
                    return true;
                }
                return value.EnumerateObject().Any(property => Visit(property.Value));
            }
            return value.ValueKind == System.Text.Json.JsonValueKind.Array
                && value.EnumerateArray().Any(Visit);
        }
    }

    private static bool HasMethodAuthorityMismatch(ReadOnlySpan<byte> source)
    {
        using JsonDocument document = JsonDocument.Parse(source.ToArray());
        JsonElement root = document.RootElement;
        return root.TryGetProperty("envelope"u8, out JsonElement envelope)
            && envelope.TryGetProperty("authorityRef"u8, out JsonElement authority)
            && authority.TryGetProperty("authorityKind"u8, out JsonElement authorityKind)
            && authorityKind.GetString() != "desktop_local_store";
    }

    private static string LastSegment(string location)
    {
        int index = location.LastIndexOf('/');
        return index < 0 ? location : location[(index + 1)..];
    }

    private static int LocationDepth(string location) => location.Count(static value => value == '/');

    private static string FixturePath(string relativePath) => Path.Combine(
        AppContext.BaseDirectory,
        "fixtures",
        relativePath.Replace('/', Path.DirectorySeparatorChar));

    private sealed record FixtureEntry(
        string File,
        string? Schema,
        bool Valid,
        string? ReasonCode,
        string[] ReasonCodes,
        string? ContextFile);
}
