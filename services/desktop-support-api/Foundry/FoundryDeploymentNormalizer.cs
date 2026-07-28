using System.Text.Json;
using System.Text.RegularExpressions;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Foundry;

internal sealed record FoundryNormalizationResult(
    bool Succeeded,
    string ErrorCode,
    FoundryDeploymentSnapshot? Snapshot);

/// <summary>
/// Projects raw Foundry deployment pages into the canonical
/// <c>sapphirus.foundry-deployment.v1</c> shape.
///
/// Every field is bounded and re-validated here, so a provider string that
/// cannot be represented canonically fails the whole snapshot rather than
/// leaking a raw value or half an inventory. Capability keys pass an
/// allowlist: an unadvertised capability normalizes to false, and a provider
/// key nobody has approved is dropped. Normalization narrows; it never
/// broadens.
/// </summary>
internal static partial class FoundryDeploymentNormalizer
{
    internal const string DeploymentHashPurpose = "foundry-deployment";
    internal const string SnapshotHashPurpose = "foundry-deployment-snapshot";
    internal const int MaximumCapabilityKeys = 64;

    /// <summary>The eight canonical capability names the contract defines.</summary>
    internal const int CanonicalCapabilityCount = 8;

    /// <summary>
    /// The only provider capability keys that may influence the canonical
    /// capability set. Anything else is ignored.
    /// </summary>
    private static readonly Dictionary<string, FoundryCapabilityFlag> CapabilityAllowlist =
        new(StringComparer.Ordinal)
        {
            ["text_input"] = FoundryCapabilityFlag.TextInput,
            ["image_input"] = FoundryCapabilityFlag.ImageInput,
            ["pdf_input"] = FoundryCapabilityFlag.PdfInput,
            ["attachments"] = FoundryCapabilityFlag.Attachments,
            ["tool_calling"] = FoundryCapabilityFlag.ToolCalling,
            ["json_schema"] = FoundryCapabilityFlag.StructuredOutput,
            ["structured_output"] = FoundryCapabilityFlag.StructuredOutput,
            ["reasoning"] = FoundryCapabilityFlag.Reasoning,
            ["streaming"] = FoundryCapabilityFlag.Streaming,
        };

    internal enum FoundryCapabilityFlag
    {
        TextInput,
        ImageInput,
        PdfInput,
        Attachments,
        ToolCalling,
        StructuredOutput,
        Reasoning,
        Streaming,
    }

    /// <summary>
    /// Normalizes every accumulated provider page into one canonical snapshot.
    /// The parameter type is the internal DTO on purpose: a raw provider shape
    /// has no public entry point into this service.
    /// </summary>
    internal static FoundryNormalizationResult Normalize(
        IReadOnlyList<FoundryDeploymentDto> providerDeployments,
        DateTimeOffset retrievedAt)
    {
        ArgumentNullException.ThrowIfNull(providerDeployments);
        if (providerDeployments.Count > FoundryDeploymentSnapshot.MaximumDeployments)
        {
            return Failure("foundry_deployment_count_exceeded");
        }

        Dictionary<string, FoundryDeployment> byName = new(StringComparer.Ordinal);
        foreach (FoundryDeploymentDto dto in providerDeployments)
        {
            if (!TryProject(dto, out FoundryDeployment? projected, out string errorCode))
            {
                return Failure(errorCode);
            }
            if (byName.TryGetValue(projected!.DeploymentName, out FoundryDeployment? existing))
            {
                // Identical duplicates collapse only when the canonical
                // identity hash matches exactly; a conflicting redefinition of
                // the same name is never silently resolved.
                if (!string.Equals(
                    existing.DeploymentHash,
                    projected.DeploymentHash,
                    StringComparison.Ordinal))
                {
                    return Failure("foundry_deployment_duplicate_conflict");
                }
                continue;
            }
            byName.Add(projected.DeploymentName, projected);
        }

        FoundryDeployment[] canonical = [.. byName.Values
            .OrderBy(static deployment => deployment.DeploymentName, StringComparer.Ordinal)];
        if (canonical.Length > FoundryDeploymentSnapshot.MaximumDeployments)
        {
            return Failure("foundry_deployment_count_exceeded");
        }

        return new FoundryNormalizationResult(
            Succeeded: true,
            ErrorCode: "",
            new FoundryDeploymentSnapshot(
                FoundryDeploymentSnapshot.SchemaVersionValue,
                ComputeSnapshotHash(canonical),
                retrievedAt,
                canonical));
    }

    /// <summary>
    /// Hashes the canonical deployment set. The retrieval timestamp is
    /// excluded on purpose: an unchanged inventory must hash identically
    /// across refreshes so the cache can recognize a no-op.
    /// </summary>
    internal static string ComputeSnapshotHash(
        IReadOnlyList<FoundryDeployment> deployments)
    {
        List<object> identities = [];
        foreach (FoundryDeployment deployment in deployments)
        {
            identities.Add(deployment.DeploymentHash);
        }
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["deployments"] = identities,
            ["schemaVersion"] = FoundryDeploymentSnapshot.SchemaVersionValue,
        };
        return CanonicalProof.Render(
            CanonicalProof.ComputeDigest(SnapshotHashPurpose, draft));
    }

    private static bool TryProject(
        FoundryDeploymentDto dto,
        out FoundryDeployment? deployment,
        out string errorCode)
    {
        deployment = null;
        errorCode = "";
        if (dto.Name is null || !DeploymentName().IsMatch(dto.Name))
        {
            errorCode = "foundry_deployment_name_invalid";
            return false;
        }
        if (!string.Equals(
            dto.Type,
            FoundryDeployment.ModelDeploymentType,
            StringComparison.Ordinal))
        {
            errorCode = "foundry_deployment_type_invalid";
            return false;
        }
        if (!IsModelIdentityPart(dto.ModelPublisher)
            || !IsModelIdentityPart(dto.ModelName)
            || !IsModelIdentityPart(dto.ModelVersion))
        {
            errorCode = "foundry_deployment_model_identity_invalid";
            return false;
        }
        if (!TryProjectSku(dto.Sku, out FoundryDeploymentSku? sku))
        {
            errorCode = "foundry_deployment_sku_invalid";
            return false;
        }
        if (!TryProjectCapabilities(
            dto.Capabilities,
            out FoundryDeploymentCapabilities? capabilities))
        {
            errorCode = "foundry_deployment_capabilities_invalid";
            return false;
        }

        deployment = new FoundryDeployment(
            dto.Name,
            FoundryDeployment.ModelDeploymentType,
            dto.ModelPublisher!,
            dto.ModelName!,
            dto.ModelVersion!,
            sku!,
            capabilities!,
            ComputeDeploymentHash(
                dto.Name,
                dto.ModelPublisher!,
                dto.ModelName!,
                dto.ModelVersion!,
                sku!,
                capabilities!));
        return true;
    }

    private static bool TryProjectSku(
        FoundryDeploymentSkuDto? dto,
        out FoundryDeploymentSku? sku)
    {
        sku = null;
        if (dto is null
            || !IsModelIdentityPart(dto.Name)
            || !IsOptionalModelIdentityPart(dto.Tier)
            || !IsOptionalModelIdentityPart(dto.Family)
            || !IsOptionalModelIdentityPart(dto.Size)
            || dto.Capacity is < 0 or > 1_000_000)
        {
            return false;
        }
        sku = new FoundryDeploymentSku(
            dto.Name!,
            dto.Tier,
            dto.Family,
            dto.Size,
            dto.Capacity);
        return true;
    }

    private static bool TryProjectCapabilities(
        IReadOnlyDictionary<string, JsonElement>? advertised,
        out FoundryDeploymentCapabilities? capabilities)
    {
        capabilities = null;
        if (advertised is null)
        {
            capabilities = FoundryDeploymentCapabilities.None;
            return true;
        }
        if (advertised.Count > MaximumCapabilityKeys)
        {
            return false;
        }

        bool[] flags = new bool[CanonicalCapabilityCount];
        foreach ((string key, JsonElement value) in advertised)
        {
            if (!CapabilityAllowlist.TryGetValue(key, out FoundryCapabilityFlag flag))
            {
                // An unapproved or future provider key is ignored, never
                // projected and never a failure.
                continue;
            }
            if (value.ValueKind == JsonValueKind.True)
            {
                flags[(int)flag] = true;
            }
        }

        capabilities = new FoundryDeploymentCapabilities(
            flags[(int)FoundryCapabilityFlag.TextInput],
            flags[(int)FoundryCapabilityFlag.ImageInput],
            flags[(int)FoundryCapabilityFlag.PdfInput],
            flags[(int)FoundryCapabilityFlag.Attachments],
            flags[(int)FoundryCapabilityFlag.ToolCalling],
            flags[(int)FoundryCapabilityFlag.StructuredOutput],
            flags[(int)FoundryCapabilityFlag.Reasoning],
            flags[(int)FoundryCapabilityFlag.Streaming]);
        return true;
    }

    private static string ComputeDeploymentHash(
        string deploymentName,
        string modelPublisher,
        string modelName,
        string modelVersion,
        FoundryDeploymentSku sku,
        FoundryDeploymentCapabilities capabilities)
    {
        SortedDictionary<string, object?> draft = new(StringComparer.Ordinal)
        {
            ["capabilities"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["attachments"] = capabilities.Attachments,
                ["imageInput"] = capabilities.ImageInput,
                ["pdfInput"] = capabilities.PdfInput,
                ["reasoning"] = capabilities.Reasoning,
                ["streaming"] = capabilities.Streaming,
                ["structuredOutput"] = capabilities.StructuredOutput,
                ["textInput"] = capabilities.TextInput,
                ["toolCalling"] = capabilities.ToolCalling,
            },
            ["deploymentName"] = deploymentName,
            ["deploymentType"] = FoundryDeployment.ModelDeploymentType,
            ["modelName"] = modelName,
            ["modelPublisher"] = modelPublisher,
            ["modelVersion"] = modelVersion,
            ["sku"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["capacity"] = sku.Capacity,
                ["family"] = sku.Family,
                ["name"] = sku.Name,
                ["size"] = sku.Size,
                ["tier"] = sku.Tier,
            },
        };
        return CanonicalProof.Render(
            CanonicalProof.ComputeDigest(DeploymentHashPurpose, draft));
    }

    private static bool IsModelIdentityPart(string? value) =>
        value is not null && ModelIdentityPart().IsMatch(value);

    private static bool IsOptionalModelIdentityPart(string? value) =>
        value is null || ModelIdentityPart().IsMatch(value);

    private static FoundryNormalizationResult Failure(string errorCode) =>
        new(Succeeded: false, errorCode, Snapshot: null);

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9_-]{0,126}[A-Za-z0-9]$")]
    private static partial Regex DeploymentName();

    [GeneratedRegex("^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")]
    private static partial Regex ModelIdentityPart();
}
