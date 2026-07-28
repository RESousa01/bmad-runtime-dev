using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-029's admission half: the fixed streaming route accepts only a version
/// two streaming request whose consent still agrees with it, and the provider
/// projection it builds carries the resolved route's authority rather than the
/// request's.
/// </summary>
public sealed class ModelStreamRequestAdmissionTests
{
    [Fact]
    public void A_well_formed_streaming_request_is_admitted()
    {
        Assert.True(RequestGuards.ValidateStreamRequest(CreateRequest(), out string errorCode));
        Assert.Equal("", errorCode);
    }

    [Fact]
    public void A_structured_request_is_refused_on_the_streaming_route()
    {
        ModelAccessRequestV2 request = CreateRequest() with
        {
            RequestMode = ModelRequestModes.StructuredResponse,
        };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal(SupportProblemCodes.RequestInvalid, errorCode);
    }

    [Fact]
    public void A_streaming_request_that_also_names_an_output_schema_is_refused()
    {
        ModelAccessRequestV2 request = CreateRequest() with
        {
            CanonicalOutputSchemaId = "sapphirus.bmad-method-help-proposal.v1",
        };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal(SupportProblemCodes.ModelCapabilityMismatch, errorCode);
    }

    [Theory]
    [InlineData("profile")]
    [InlineData("variant")]
    [InlineData("catalog")]
    [InlineData("mode")]
    [InlineData("snapshot")]
    [InlineData("capability")]
    public void A_consent_that_no_longer_agrees_with_the_request_is_refused(string mutation)
    {
        ModelAccessRequestV2 request = CreateRequest();
        request = mutation switch
        {
            "profile" => request with { ModelProfileId = "another-profile" },
            "variant" => request with { RequestedVariantId = "thorough" },
            "catalog" => request with { CatalogVersion = "v1.fedcba9876543210" },
            "mode" => request with
            {
                Consent = request.Consent with
                {
                    RequestMode = ModelRequestModes.StructuredResponse,
                },
            },
            "snapshot" => request with { ContextSnapshotId = "snap_BBBBBBBBBBBBBBBB" },
            "capability" => request with
            {
                // The consent was signed over the other capability digest.
                ModelCapabilityHash = FoundryResponsesTestFixtures.Hash,
            },
            _ => request,
        };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal("consent_binding_mismatch", errorCode);
    }

    [Fact]
    public void A_context_item_whose_content_hash_does_not_match_is_refused()
    {
        ModelAccessRequestV2 request = CreateRequest();
        request = request with
        {
            Items = [request.Items[0] with { ContentHash = FoundryResponsesTestFixtures.Hash }],
        };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal("context_item_hash_mismatch", errorCode);
    }

    [Theory]
    [InlineData("/etc/passwd")]
    [InlineData("C:/repo/file.cs")]
    [InlineData("src/../secrets.txt")]
    public void An_unsafe_relative_label_is_refused_before_any_egress(string label)
    {
        ModelAccessRequestV2 request = CreateRequest();
        ModelContextItem item = request.Items[0] with { RelativeLabel = label };
        request = request with { Items = [item] };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal("context_item_invalid", errorCode);
    }

    [Fact]
    public void A_tool_name_outside_the_canonical_alphabet_is_refused()
    {
        ModelAccessRequestV2 request = CreateRequest() with
        {
            Tools =
            [
                new ModelToolDefinition(
                    "Lookup-Tool",
                    "A tool.",
                    "sapphirus.tool-parameters.v1",
                    FoundryResponsesTestFixtures.Hash),
            ],
        };

        Assert.False(RequestGuards.ValidateStreamRequest(request, out string errorCode));
        Assert.Equal(SupportProblemCodes.ModelCapabilityMismatch, errorCode);
    }

    [Fact]
    public void The_provider_projection_takes_its_authority_from_the_resolved_route()
    {
        ResolvedModelRoute route = FoundryResponsesTestFixtures.Route();

        ModelProviderRequest provider = ModelStreamEndpoint.BuildProviderRequest(
            CreateRequest(),
            route);

        Assert.Equal(route.DeploymentName, provider.Deployment);
        Assert.Equal(route.ProfileId, provider.ProfileId);
        Assert.Equal(route.VariantId, provider.VariantId);
        Assert.Equal(route.ReasoningEffort, provider.ReasoningEffort);
        Assert.Equal(route.Verbosity, provider.Verbosity);
        Assert.Equal(route.MaxOutputTokens, provider.MaximumOutputTokens);
        Assert.True(provider.IsStreaming);
        Assert.Equal("", provider.OutputSchemaJson);
        Assert.Equal(
            ModelStreamEndpoint.StreamingSystemInstruction,
            provider.SystemInstruction);
    }

    [Fact]
    public void The_projected_context_carries_a_closed_key_set()
    {
        string projected = ModelStreamEndpoint.ProjectContext(CreateRequest());

        using JsonDocument document = JsonDocument.Parse(projected);
        Assert.Equal(
            ["items", "purpose"],
            document.RootElement.EnumerateObject().Select(item => item.Name).ToArray());
        JsonElement item = Assert.Single(document.RootElement.GetProperty("items")
            .EnumerateArray());
        Assert.Equal(
            ["content", "language", "relativeLabel", "semanticRole"],
            item.EnumerateObject().Select(property => property.Name).ToArray());
    }

    [Fact]
    public void The_receipt_authorization_is_lifted_from_the_signed_decision()
    {
        ModelAccessRequestV2 request = CreateRequest();

        ModelReceiptAuthorization authorization = ModelStreamEndpoint.Authorize(request);

        Assert.Equal(request.RequestId, authorization.RequestId);
        Assert.Equal(request.RegistrationId, authorization.RegistrationId);
        Assert.Equal(request.Consent.TenantHash, authorization.TenantHash);
        Assert.Equal(request.Consent.ConsentEnvelopeHash, authorization.ConsentEnvelopeHash);
        Assert.Equal(request.LocalEgressManifestHash, authorization.ManifestHash);
        Assert.Equal(request.TotalContextBytes, authorization.InputBytes);
        Assert.Matches("^sha256:[0-9a-f]{64}$", authorization.RequestHash);
    }

    [Theory]
    [InlineData("catalog_stale", "catalog_stale")]
    [InlineData("profile_not_entitled", "profile_not_entitled")]
    [InlineData("deployment_missing", "model_deployment_missing")]
    [InlineData("context_limit_exceeded", "model_context_limit_exceeded")]
    [InlineData("capability_mismatch", "model_capability_mismatch")]
    public void Route_resolution_failures_map_onto_reviewed_problem_codes(
        string routeError,
        string expectedCode)
    {
        (string code, int status) = ModelStreamEndpoint.ForRouteError(routeError);

        Assert.Equal(expectedCode, code);
        Assert.True(SupportProblemCodes.IsKnown(code));
        Assert.InRange(status, 400, 599);
    }

    private static ModelAccessRequestV2 CreateRequest()
    {
        const string content = "review this bounded context";
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(Encoding.UTF8.GetBytes(content))),
            Encoding.UTF8.GetByteCount(content),
            "internal",
            content);
        ModelContextConsentV2 consent = new(
            ModelContextConsentV2.SchemaVersionValue,
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.OtherHash,
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            FoundryResponsesTestFixtures.Hash,
            "lease_01J00000000000000000000000",
            FoundryResponsesTestFixtures.Hash,
            "policy_01J00000000000000000000000",
            7,
            FoundryResponsesTestFixtures.Hash,
            "bmad_help",
            "planner",
            ModelRequestModes.StreamingChat,
            "coding-balanced",
            "balanced",
            "v1.0123456789abcdef",
            "snap_AAAAAAAAAAAAAAAA",
            null,
            null,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.OtherHash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            DateTimeOffset.Parse("2026-07-28T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-28T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-28T10:05:00.000Z"),
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.Hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                FoundryResponsesTestFixtures.Hash,
                FoundryResponsesTestFixtures.Hash,
                "ZXhhbXBsZS1zaWduYXR1cmU"));
        return new ModelAccessRequestV2(
            ModelAccessRequestV2.SchemaVersionValue,
            consent.RequestId,
            "windows_local",
            consent.RegistrationId,
            "bmad_help",
            "planner",
            ModelRequestModes.StreamingChat,
            "coding-balanced",
            "balanced",
            "v1.0123456789abcdef",
            FoundryResponsesTestFixtures.Hash,
            FoundryResponsesTestFixtures.OtherHash,
            "snap_AAAAAAAAAAAAAAAA",
            null,
            null,
            FoundryResponsesTestFixtures.Hash,
            consent,
            [item],
            [],
            "transient_no_store",
            "interactive-standard");
    }
}
