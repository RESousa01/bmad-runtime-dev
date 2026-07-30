using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Tests.Foundry;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// Shared inputs for the Responses adapter, the stream normalizer, and the
/// version two receipt.
///
/// The provider bodies come from the same
/// <c>tools/foundry-fake/fixtures/foundry-fixtures.v1.json</c> the fake server
/// serves, so a fixture change breaks these tests instead of quietly letting
/// the adapter diverge from the wire it claims to speak.
/// </summary>
internal static class FoundryResponsesTestFixtures
{
    internal const string SyntheticBearerMarker = "forge-foundry-fake-marker-v1";

    internal const string ProjectEndpoint =
        "https://forge-fake.test/api/projects/forge-test/";

    internal const string ResponsesEndpoint = ProjectEndpoint + "openai/v1/";

    internal const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    internal const string OtherHash =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    /// <summary>Non-secret credential version label used across binding-hash tests.</summary>
    internal const string CredentialVersionId = "v-test-1";

    /// <summary>Non-secret project endpoint hash used across binding-hash tests.</summary>
    internal const string ProjectEndpointHash =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    /// <summary>Non-secret application revision used across binding-hash tests.</summary>
    internal const string ApplicationRevision = "revision-test-1";

    /// <summary>
    /// Synthetic values that must never appear in a log, a trace, a problem
    /// document, a receipt, or an exception. They use the allowlisted canary
    /// spelling so the repository secret scanner accepts them deliberately.
    /// </summary>
    internal const string CredentialCanary = "must-not-cross-credential-boundary";

    internal const string BodyCanary = "must-not-cross-provider-body";

    internal const string ReasoningCanary = "must-not-cross-private-reasoning";

    private static readonly JsonDocumentOptions FixtureOptions = new()
    {
        AllowTrailingCommas = false,
        CommentHandling = JsonCommentHandling.Disallow,
        MaxDepth = 32,
    };

    /// <summary>Reads one <c>responseScenarios</c> entry's first reply body.</summary>
    internal static string ResponseScenarioBody(string scenarioName)
    {
        string path = Path.Combine(
            AppContext.BaseDirectory,
            "ContractFixtures",
            "foundry-fixtures.v1.json");
        using JsonDocument document = JsonDocument.Parse(
            File.ReadAllText(path),
            FixtureOptions);
        foreach (JsonElement scenario in document.RootElement
            .GetProperty("responseScenarios")
            .EnumerateArray())
        {
            if (!string.Equals(
                scenario.GetProperty("name").GetString(),
                scenarioName,
                StringComparison.Ordinal))
            {
                continue;
            }
            JsonElement reply = scenario.GetProperty("responses")[0];
            return reply.TryGetProperty("body", out JsonElement raw)
                ? raw.GetString() ?? ""
                : reply.GetProperty("jsonBody").GetRawText();
        }
        throw new InvalidOperationException(
            "The named response scenario is absent from the canonical fixtures.");
    }

    /// <summary>Whether the named scenario serves a server-sent event body.</summary>
    internal static bool IsEventStreamScenario(string scenarioName)
    {
        string path = Path.Combine(
            AppContext.BaseDirectory,
            "ContractFixtures",
            "foundry-fixtures.v1.json");
        using JsonDocument document = JsonDocument.Parse(
            File.ReadAllText(path),
            FixtureOptions);
        foreach (JsonElement scenario in document.RootElement
            .GetProperty("responseScenarios")
            .EnumerateArray())
        {
            if (string.Equals(
                scenario.GetProperty("name").GetString(),
                scenarioName,
                StringComparison.Ordinal))
            {
                return scenario.GetProperty("responses")[0]
                    .GetProperty("contentType")
                    .GetString() == "text/event-stream";
            }
        }
        throw new InvalidOperationException(
            "The named response scenario is absent from the canonical fixtures.");
    }

    internal static ModelProviderRequest StructuredRequest(
        string deployment = "desktop-planning",
        int maximumOutputTokens = 4096) => new(
        deployment,
        ApiVersion: "",
        SystemInstruction: "Use only the provided context items.",
        UserPayloadJson: """{"items":[],"purpose":"bmad_help"}""",
        OutputSchemaJson: CanonicalModelOutputValidator.OutputSchemaJson,
        MaximumOutputBytes: 262_144,
        ProfileId: "coding-balanced",
        VariantId: "balanced",
        RequestMode: ModelRequestModes.StructuredResponse,
        MaximumOutputTokens: maximumOutputTokens);

    internal static ModelProviderRequest StreamingRequest(
        string deployment = "desktop-planning",
        IReadOnlyList<ModelProviderTool>? tools = null,
        string reasoningEffort = "medium",
        string verbosity = "low") => new(
        deployment,
        ApiVersion: "",
        SystemInstruction: "Answer in plain text.",
        UserPayloadJson: """{"items":[],"purpose":"bmad_help"}""",
        OutputSchemaJson: "",
        MaximumOutputBytes: 262_144,
        ProfileId: "coding-balanced",
        VariantId: "balanced",
        RequestMode: ModelRequestModes.StreamingChat,
        ReasoningEffort: reasoningEffort,
        Verbosity: verbosity,
        MaximumOutputTokens: 4096,
        Tools: tools);

    internal static ModelPriceProfile PriceProfile() => new(
        "foundry-price-2026-07",
        "EUR",
        InputMicrounitsPerThousand: 2_750,
        OutputMicrounitsPerThousand: 11_000,
        ReasoningMicrounitsPerThousand: 11_000,
        CacheReadMicrounitsPerThousand: 275);

    internal static ResolvedModelRoute Route(
        string profileId = "coding-balanced",
        string variantId = "balanced",
        string requestMode = ModelRequestModes.StreamingChat)
    {
        ResolvedModelRoute route = new(
            RouteHash: "",
            profileId,
            ProfileBindingHash: Hash,
            CapabilityBindingHash: OtherHash,
            RegistryHash: Hash,
            CatalogVersion: "v1.0123456789abcdef",
            CatalogHash: OtherHash,
            DeploymentName: "desktop-planning",
            DeploymentHash: Hash,
            ModelPublisher: "OpenAI",
            ModelName: "gpt-5-mini",
            ModelVersion: "2026-06-01",
            variantId,
            ReasoningEffort: "medium",
            Verbosity: "low",
            requestMode,
            CanonicalOutputSchemaId: null,
            Capabilities: new ProjectedCapabilities(
                TextInput: true,
                ImageInput: false,
                PdfInput: false,
                Attachments: false,
                ToolCalling: true,
                StructuredOutput: true,
                Reasoning: true),
            ContextTokens: 200_000,
            MaxInputTokens: 100_000,
            MaxOutputTokens: 4_096,
            MaxOutputBytes: 262_144,
            MaxAttachmentBytes: 0,
            RetentionMode: "transient_no_store",
            Region: "westeurope",
            RetryPolicy: new ModelProfileRetryPolicy(3, 100, 1_000),
            PriceProfile: PriceProfile());
        return route with { RouteHash = Hash };
    }

    internal static ModelReceiptAuthorization Authorization(
        string requestId = "request_01J00000000000000000000000",
        long inputBytes = 27) => new(
        requestId,
        RequestHash: Hash,
        RegistrationId: "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
        TenantHash: Hash,
        SubjectHash: OtherHash,
        ManifestHash: Hash,
        InvocationBindingHash: Hash,
        ConsumptionHash: Hash,
        ConsentEnvelopeHash: Hash,
        ConsentDisclosureHash: Hash,
        RetentionMode: "transient_no_store",
        inputBytes);

    internal static ModelStreamCall Call(
        ResolvedModelRoute? route = null,
        string credentialMechanismId = FoundryCredentialMechanisms.ManagedIdentity)
    {
        ResolvedModelRoute resolved = route ?? Route();
        return new ModelStreamCall(
            "subject-a",
            Authorization(),
            "run_01J00000000000000000000000",
            resolved,
            DeploymentInventoryHash: OtherHash,
            credentialMechanismId,
            StreamingRequest(resolved.DeploymentName),
            ModelRole: "planner",
            BudgetClass: "interactive-standard",
            RequestedVariantId: resolved.VariantId);
    }

    /// <summary>
    /// Applies the fake server's synthetic bearer marker. It stands in for a
    /// real mechanism so the transport can be proved without either credential
    /// implementation being present.
    /// </summary>
    internal sealed class FakeFoundryAuthenticator(
        string mechanismId = FoundryCredentialMechanisms.ManagedIdentity)
        : IFoundryRequestAuthenticator
    {
        public int OperationCount { get; private set; }

        public string CredentialMechanismId => mechanismId;

        public string CredentialVersionId => FoundryResponsesTestFixtures.CredentialVersionId;

        public ValueTask ApplyAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            ArgumentNullException.ThrowIfNull(request);
            cancellationToken.ThrowIfCancellationRequested();
            request.Headers.Authorization =
                new AuthenticationHeaderValue("Bearer", SyntheticBearerMarker);
            return ValueTask.CompletedTask;
        }

        public ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            OperationCount++;
            return ValueTask.FromResult<IFoundryRequestAuthenticator>(this);
        }
    }

    /// <summary>An authenticator that cannot acquire anything.</summary>
    internal sealed class FailingFoundryAuthenticator : IFoundryRequestAuthenticator
    {
        public string CredentialMechanismId => FoundryCredentialMechanisms.ManagedIdentity;

        public string CredentialVersionId => FoundryResponsesTestFixtures.CredentialVersionId;

        public ValueTask ApplyAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            throw new InvalidOperationException(CredentialCanary);

        public ValueTask<IFoundryRequestAuthenticator> BeginOperationAsync(
            CancellationToken cancellationToken) =>
            throw new InvalidOperationException(CredentialCanary);
    }

    internal static HttpClient CreateResponsesClient(FakeFoundryServer server)
    {
        ArgumentNullException.ThrowIfNull(server);
        return new HttpClient(server, disposeHandler: false)
        {
            BaseAddress = new Uri(ResponsesEndpoint),
        };
    }

    internal static FakeFoundryReply JsonReply(HttpStatusCode statusCode, string body) =>
        FakeFoundryReply.Json(statusCode, body);

    internal static FakeFoundryReply EventStreamReply(string body) =>
        FakeFoundryReply.EventStream(body);
}
