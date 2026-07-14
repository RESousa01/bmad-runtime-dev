using System.Text;
using System.Text.Json;
using Sapphirus.Contracts.Generated;
using Sapphirus.GeneratorQualification.Generated;
using Xunit;
using CorvusJsonValueKind = Corvus.Text.Json.JsonValueKind;
using QualificationWire = Sapphirus.GeneratorQualification.Generated.GeneratorQualification;

namespace Sapphirus.GeneratorQualification.Tests;

public sealed class QualificationTests
{
    private static readonly JsonSerializerOptions CatalogOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    [Fact]
    public void EveryCatalogFixtureMatchesTheStrictParserAndCorvusResult()
    {
        QualificationCatalog catalog = LoadCatalog();
        int validatorInvocations = 0;
        var validator = new QualificationValidator(() => validatorInvocations++);

        Assert.Equal(25, catalog.Fixtures.Count);
        foreach (QualificationFixture fixture in catalog.Fixtures)
        {
            byte[] source = File.ReadAllBytes(QualificationPath(fixture.File));
            int before = validatorInvocations;

            QualificationResult actual = validator.Validate(
                source,
                catalog.ParserLimits,
                catalog.ReasonPriority);

            Assert.True(
                fixture.Expected == "accept" == actual.Accepted,
                $"{fixture.Id}: expected acceptance {fixture.Expected}, got {actual.Accepted}.");
            Assert.True(
                fixture.ReasonCategory == actual.ReasonCategory,
                $"{fixture.Id}: expected reason {fixture.ReasonCategory}, got {actual.ReasonCategory}.");
            Assert.True(
                fixture.RejectionStage == actual.RejectionStage,
                $"{fixture.Id}: expected stage {fixture.RejectionStage}, got {actual.RejectionStage}.");
            Assert.True(
                fixture.ValidatorInvoked == actual.ValidatorInvoked,
                $"{fixture.Id}: expected validatorInvoked {fixture.ValidatorInvoked}, got {actual.ValidatorInvoked}.");
            Assert.Equal(fixture.ValidatorInvoked ? 1 : 0, validatorInvocations - before);

            if (fixture.Expected == "accept")
            {
                using JsonDocument original = StrictJson.Parse(source, catalog.ParserLimits);
                QualificationWire generated = QualificationWire.ParseValue(source);
                Assert.True(generated.EvaluateSchema());

                byte[] serialized = Encoding.UTF8.GetBytes(generated.ToString());
                using JsonDocument roundTrip = StrictJson.Parse(serialized, catalog.ParserLimits);
                QualificationWire reparsed = QualificationWire.ParseValue(serialized);
                Assert.True(reparsed.EvaluateSchema());
                Assert.Equal(
                    CanonicalJson.Serialize(original.RootElement),
                    CanonicalJson.Serialize(roundTrip.RootElement));
            }
        }
    }

    [Fact]
    public void DuplicateMembersAreRejectedBeforeCorvusIsInvoked()
    {
        int validatorInvocations = 0;
        var validator = new QualificationValidator(() => validatorInvocations++);
        QualificationCatalog catalog = LoadCatalog();

        QualificationResult result = validator.Validate(
            Encoding.UTF8.GetBytes("{\"value\":1,\"value\":2}"),
            catalog.ParserLimits,
            catalog.ReasonPriority);

        Assert.False(result.Accepted);
        Assert.Equal("DUPLICATE_MEMBER", result.ReasonCategory);
        Assert.Equal("strict_parser", result.RejectionStage);
        Assert.False(result.ValidatorInvoked);
        Assert.Equal(0, validatorInvocations);
    }

    [Theory]
    [InlineData("9007199254740991.1", "INTEGER_OUT_OF_RANGE")]
    [InlineData("1e309", "SCHEMA_INVALID")]
    public void NonFiniteAndPrecisionLosingNumbersAreRejectedBeforeCorvusIsInvoked(
        string source,
        string expectedReason)
    {
        int validatorInvocations = 0;
        var validator = new QualificationValidator(() => validatorInvocations++);
        QualificationCatalog catalog = LoadCatalog();

        QualificationResult result = validator.Validate(
            Encoding.UTF8.GetBytes(source),
            catalog.ParserLimits,
            catalog.ReasonPriority);

        Assert.False(result.Accepted);
        Assert.Equal(expectedReason, result.ReasonCategory);
        Assert.Equal("strict_parser", result.RejectionStage);
        Assert.False(result.ValidatorInvoked);
        Assert.Equal(0, validatorInvocations);
    }

    [Fact]
    public void StrictParserEnforcesUtf8ByteAndContainerDepthLimits()
    {
        StrictJsonException bytes = Assert.Throws<StrictJsonException>(
            () => StrictJson.Parse(Encoding.UTF8.GetBytes("\"é\""), new ParserLimits(3, 8)));
        Assert.Equal("MAX_BYTES_EXCEEDED", bytes.Code);

        StrictJsonException depth = Assert.Throws<StrictJsonException>(
            () => StrictJson.Parse(Encoding.UTF8.GetBytes("{\"nested\":{}}"), new ParserLimits(100, 1)));
        Assert.Equal("MAX_DEPTH_EXCEEDED", depth.Code);
    }

    [Fact]
    public void GeneratedQualificationPreservesRequiredNullAndOptionalAbsence()
    {
        byte[] absentSource = File.ReadAllBytes(QualificationPath("fixtures/valid/text-null-empty.json"));
        QualificationWire absent = QualificationWire.ParseValue(absentSource);
        Assert.Equal(CorvusJsonValueKind.Null, absent["nullableValue"u8].ValueKind);
        Assert.False(absent.TryGetProperty("optionalValue"u8, out _));

        byte[] nullSource = File.ReadAllBytes(QualificationPath("fixtures/valid/count-optional-null.json"));
        QualificationWire explicitNull = QualificationWire.ParseValue(nullSource);
        Assert.True(explicitNull.TryGetProperty("optionalValue"u8, out Corvus.Text.Json.JsonElement optional));
        Assert.Equal(CorvusJsonValueKind.Null, optional.ValueKind);

        QualificationWire roundTrip = QualificationWire.ParseValue(explicitNull.ToString());
        Assert.True(roundTrip.TryGetProperty("optionalValue"u8, out optional));
        Assert.Equal(CorvusJsonValueKind.Null, optional.ValueKind);
        Assert.True(roundTrip.EvaluateSchema());
    }

    [Fact]
    public void ActualProductionCorvusTreeParsesAndValidatesRepresentativeRoot()
    {
        byte[] source = File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "windows-local-candidate.json"));
        SapphirusContractsCatalog.CandidateAction candidate =
            SapphirusContractsCatalog.CandidateAction.ParseValue(source);

        Assert.True(candidate.EvaluateSchema());
        Assert.Equal("sapphirus.candidate-action.v1", candidate.SchemaVersion.ToString());
    }

    private static QualificationCatalog LoadCatalog()
    {
        string json = File.ReadAllText(QualificationPath("catalog.json"));
        return JsonSerializer.Deserialize<QualificationCatalog>(json, CatalogOptions)
            ?? throw new InvalidOperationException("Qualification catalog did not deserialize.");
    }

    private static string QualificationPath(string relativePath) =>
        Path.Combine(
            AppContext.BaseDirectory,
            "qualification",
            relativePath.Replace('/', Path.DirectorySeparatorChar));
}

public sealed record QualificationCatalog(
    ParserLimits ParserLimits,
    IReadOnlyList<string> ReasonPriority,
    IReadOnlyList<QualificationFixture> Fixtures);

public sealed record ParserLimits(int MaxBytes, int MaxContainerDepth);

public sealed record QualificationFixture(
    string Id,
    string File,
    string Expected,
    string? ReasonCategory,
    string RejectionStage,
    bool ValidatorInvoked);
