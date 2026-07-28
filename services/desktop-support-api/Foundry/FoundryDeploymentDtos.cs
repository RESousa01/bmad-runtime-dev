using System.Text.Json;
using System.Text.Json.Serialization;

namespace Sapphirus.DesktopSupportApi.Foundry;

/// <summary>
/// Canonical normalized inventory of the Foundry deployments that physically
/// exist, matching <c>packages/contracts/schemas/foundry-deployment.schema.json</c>
/// (<c>sapphirus.foundry-deployment.v1</c>) field for field.
///
/// The generated Corvus types for that schema exist under
/// <c>packages/contracts/generated/dotnet</c>, but they are compiled only into
/// the contract-conformance test project. This service follows the same
/// convention as <c>Contracts.cs</c> and carries hand-written records.
///
/// This contract describes what exists, never how to reach it: the project
/// endpoint, subscription, resource group, and identity IDs are deliberately
/// unrepresentable here.
/// </summary>
public sealed record FoundryDeploymentSnapshot(
    string SchemaVersion,
    string SnapshotHash,
    DateTimeOffset RetrievedAt,
    IReadOnlyList<FoundryDeployment> Deployments)
{
    public const string SchemaVersionValue = "sapphirus.foundry-deployment.v1";
    public const int MaximumDeployments = 256;
}

public sealed record FoundryDeployment(
    string DeploymentName,
    string DeploymentType,
    string ModelPublisher,
    string ModelName,
    string ModelVersion,
    FoundryDeploymentSku Sku,
    FoundryDeploymentCapabilities Capabilities,
    string DeploymentHash)
{
    public const string ModelDeploymentType = "ModelDeployment";
}

public sealed record FoundryDeploymentSku(
    string Name,
    string? Tier,
    string? Family,
    string? Size,
    int? Capacity);

/// <summary>
/// Allowlisted provider capability keys normalized to canonical names. Every
/// key is present so the canonical ordering is total and hashable. A
/// capability the provider did not advertise normalizes to false: deployment
/// data may narrow a profile, never broaden it.
/// </summary>
public sealed record FoundryDeploymentCapabilities(
    bool TextInput,
    bool ImageInput,
    bool PdfInput,
    bool Attachments,
    bool ToolCalling,
    bool StructuredOutput,
    bool Reasoning,
    bool Streaming)
{
    public static readonly FoundryDeploymentCapabilities None = new(
        TextInput: false,
        ImageInput: false,
        PdfInput: false,
        Attachments: false,
        ToolCalling: false,
        StructuredOutput: false,
        Reasoning: false,
        Streaming: false);
}

/// <summary>
/// Server-only retrieval evidence. It never reaches a desktop client and never
/// carries a deployment name, endpoint, or token.
/// </summary>
public sealed record FoundryDiscoveryEvidence(
    DateTimeOffset RetrievedAt,
    int PageCount,
    long ResponseBytes,
    IReadOnlyList<string> ClientRequestIds,
    IReadOnlyList<string> ProviderRequestIds);

public enum FoundryDiscoveryOutcome
{
    Success,
    Authorization,
    ProjectConfiguration,
    Transient,
    InvalidBody,
    PaginationIntegrity,
}

public sealed record FoundryDiscoveryResult(
    FoundryDiscoveryOutcome Outcome,
    string ErrorCode,
    FoundryDeploymentSnapshot? Snapshot,
    FoundryDiscoveryEvidence Evidence)
{
    public bool IsSuccess => Outcome == FoundryDiscoveryOutcome.Success
        && Snapshot is not null;

    /// <summary>
    /// Only a transient outcome may keep a previously valid snapshot in
    /// service. Authorization, configuration, integrity, and body failures
    /// never extend staleness.
    /// </summary>
    public bool IsTransient => Outcome == FoundryDiscoveryOutcome.Transient;
}

/// <summary>
/// The raw provider page shape. Nothing in this layer is ever surfaced: the
/// normalizer projects it into the canonical contract and the DTOs are
/// discarded.
/// </summary>
internal sealed record FoundryDeploymentPageDto
{
    [JsonPropertyName("value")]
    public List<FoundryDeploymentDto>? Value { get; init; }

    [JsonPropertyName("nextLink")]
    public string? NextLink { get; init; }
}

internal sealed record FoundryDeploymentDto
{
    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("type")]
    public string? Type { get; init; }

    [JsonPropertyName("modelPublisher")]
    public string? ModelPublisher { get; init; }

    [JsonPropertyName("modelName")]
    public string? ModelName { get; init; }

    [JsonPropertyName("modelVersion")]
    public string? ModelVersion { get; init; }

    [JsonPropertyName("sku")]
    public FoundryDeploymentSkuDto? Sku { get; init; }

    [JsonPropertyName("capabilities")]
    public Dictionary<string, JsonElement>? Capabilities { get; init; }
}

internal sealed record FoundryDeploymentSkuDto
{
    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("tier")]
    public string? Tier { get; init; }

    [JsonPropertyName("family")]
    public string? Family { get; init; }

    [JsonPropertyName("size")]
    public string? Size { get; init; }

    [JsonPropertyName("capacity")]
    public int? Capacity { get; init; }
}
