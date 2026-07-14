using System.Text;
using System.Text.Json;

namespace Sapphirus.Contracts.Conformance.Tests;

public static class BmadSemantics
{
    private const string BuilderLimitProfile = "sapphirus.bmad-builder-limits.v1";
    private const string BmmModuleHash =
        "sha256:5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100";
    private const string ArchitectManagedPersonaHash =
        "sha256:6d3512c6f9014a2344418ce0b53b1c9ed8521e6bf8b337f2a802ade6307146e4";
    private const string ArchitectCustomizationHash =
        "sha256:d9763009d7c20246119c24bcea5eacebd21ad60c22ab191b74c9a5fb6e5f57ad";

    private static readonly IReadOnlyDictionary<string, string> ExpectedAgentRecordHashes =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["bmad-agent-analyst"] = "sha256:6b37055d48b0b5a8186d4bac5986aefc68f30ca168124f0d101b6539c21adce9",
            ["bmad-agent-architect"] = "sha256:4dc48526aac64c60d15a389f707189ac313cfdf3c69290860790b0272c5f1d20",
            ["bmad-agent-dev"] = "sha256:00b6cd96945f5563f446e09f8cb5e5dc1c3cb11a2059e42555044d47f308f54f",
            ["bmad-agent-pm"] = "sha256:ee14a413e53a6f4f52d9ca83e24babe32ba7f5cd8d2324ef921cddeb89c24869",
            ["bmad-agent-tech-writer"] = "sha256:dbd78337564afb6d7b142c2ea3188f3b1eec3250d9ba8b64281bc016325f74bf",
            ["bmad-agent-ux-designer"] = "sha256:bc39797efddbbf455b30c3de5e4b67f5df1bd9d0d4417567ab3cb109f98fcfd5",
        };

    public static IReadOnlyList<string> Validate(JsonElement value, JsonElement? descriptor = null)
    {
        var errors = new List<string>();
        string? version = String(value, "schemaVersion");
        switch (version)
        {
            case "sapphirus.bmad-package-descriptor.v1":
                ValidateDescriptor(value, errors);
                break;
            case "sapphirus.bmad-capability-catalog.v1":
                ValidateCatalog(value, descriptor, errors);
                break;
            case "sapphirus.bmad-method-session.v1":
                ValidateMethod(value, descriptor, errors);
                break;
            case "sapphirus.bmad-builder-authoring.v1":
            case "sapphirus.bmad-builder-revision.v1":
            case "sapphirus.bmad-builder-analysis.v1":
                ValidateBuilder(value, errors);
                break;
        }

        VerifyHash(value, errors);
        return errors;
    }

    private static string? String(JsonElement value, string field) =>
        value.ValueKind == JsonValueKind.Object
        && value.TryGetProperty(field, out JsonElement property)
        && property.ValueKind == JsonValueKind.String
            ? property.GetString()
            : null;

    private static JsonElement.ArrayEnumerator Array(JsonElement value, string field)
    {
        if (!value.TryGetProperty(field, out JsonElement property)
            || property.ValueKind != JsonValueKind.Array)
        {
            throw new InvalidOperationException($"Structurally validated field {field} is not an array.");
        }

        return property.EnumerateArray();
    }

    private static string NullableString(JsonElement value, string field) =>
        String(value, field) ?? string.Empty;

    private static string CapabilityKey(JsonElement value) => string.Join(
        '\0',
        NullableString(value, "packageVersionId"),
        NullableString(value, "moduleCode"),
        NullableString(value, "skillName"),
        NullableString(value, "normalizedAction"));

    private static string ScopeKey(JsonElement value)
    {
        JsonElement scope = value.GetProperty("scope"u8);
        return string.Join(
            '\0',
            NullableString(value, "graphKind"),
            NullableString(scope, "packageVersionId"),
            NullableString(scope, "moduleCode"),
            NullableString(scope, "skillName"));
    }

    private static bool StrictlySortedUnique(IEnumerable<string> keys)
    {
        string? previous = null;
        foreach (string key in keys)
        {
            if (previous is not null && StringComparer.Ordinal.Compare(previous, key) >= 0)
            {
                return false;
            }

            previous = key;
        }

        return true;
    }

    private static bool SameCapability(JsonElement? left, JsonElement? right) =>
        left is JsonElement leftValue
        && right is JsonElement rightValue
        && StringComparer.Ordinal.Equals(CapabilityKey(leftValue), CapabilityKey(rightValue));

    private static bool SameModelBinding(JsonElement? left, JsonElement? right) =>
        left is JsonElement leftValue
        && right is JsonElement rightValue
        && JsonElement.DeepEquals(leftValue, rightValue);

    private static bool SameStrings(IEnumerable<string?> actual, params string?[] expected) =>
        actual.SequenceEqual(expected, StringComparer.Ordinal);

    private static string HashJsonObject(
        string purpose,
        IEnumerable<KeyValuePair<string, object?>> properties)
    {
        byte[] source = JsonSerializer.SerializeToUtf8Bytes(
            new Dictionary<string, object?>(properties, StringComparer.Ordinal));
        using JsonDocument document = JsonDocument.Parse(source);
        return BmadCanonicalJson.Hash(purpose, "v1", document.RootElement);
    }

    private static bool AgentRecordHashIsExact(JsonElement agent)
    {
        string agentCode = NullableString(agent, "agentCode");
        if (!ExpectedAgentRecordHashes.TryGetValue(agentCode, out string? expected))
        {
            return false;
        }

        string actual = HashJsonObject(
            "bmad-agent-record",
            new Dictionary<string, object?>
            {
                ["moduleCode"] = String(agent, "moduleCode"),
                ["agentCode"] = String(agent, "agentCode"),
                ["name"] = String(agent, "name"),
                ["title"] = String(agent, "title"),
                ["icon"] = String(agent, "icon"),
                ["team"] = String(agent, "team"),
                ["description"] = String(agent, "description"),
                ["personaSourceHash"] = String(agent, "personaSourceHash"),
                ["customizationSourceHash"] = String(agent, "customizationSourceHash"),
                ["menuItems"] = agent.GetProperty("menuItems"u8),
            });
        string menuGraphHash = BmadCanonicalJson.Hash(
            "bmad-agent-menu-graph",
            "v1",
            agent.GetProperty("menuItems"u8));
        return StringComparer.Ordinal.Equals(actual, expected)
            && StringComparer.Ordinal.Equals(String(agent, "agentRecordHash"), expected)
            && StringComparer.Ordinal.Equals(String(agent, "menuGraphHash"), menuGraphHash)
            && StringComparer.Ordinal.Equals(
                String(agent, "personaCustomizationGraphHash"),
                String(agent, "customizationSourceHash"));
    }

    private static void Add(List<string> errors, string code)
    {
        if (!errors.Contains(code, StringComparer.Ordinal))
        {
            errors.Add(code);
        }
    }

    private static void VerifyHash(JsonElement value, List<string> errors)
    {
        (string Purpose, string Field)? rule = String(value, "schemaVersion") switch
        {
            "sapphirus.bmad-package-descriptor.v1" => ("bmad-package-descriptor", "descriptorHash"),
            "sapphirus.bmad-capability-catalog.v1" => ("bmad-capability-catalog", "catalogHash"),
            "sapphirus.bmad-method-checkpoint.v1" => ("bmad-method-checkpoint", "checkpointHash"),
            "sapphirus.bmad-method-session.v1" => ("contract-object", "contentHash"),
            "sapphirus.bmad-builder-revision.v1" => ("bmad-builder-revision", "revisionHash"),
            "sapphirus.bmad-builder-analysis.v1" => ("bmad-builder-analysis", "analysisHash"),
            "sapphirus.bmad-validation-report.v1" => ("bmad-validation-report", "reportHash"),
            _ => null,
        };
        if (rule is null)
        {
            return;
        }

        string expected = BmadCanonicalJson.HashWithoutField(
            rule.Value.Purpose,
            "v1",
            value,
            rule.Value.Field);
        if (!StringComparer.Ordinal.Equals(String(value, rule.Value.Field), expected))
        {
            Add(errors, "HASH_MISMATCH");
        }
    }

    private static void ValidateDescriptor(JsonElement value, List<string> errors)
    {
        JsonElement sourceIdentity = value.GetProperty("sourceIdentity"u8);
        if (!StringComparer.Ordinal.Equals(String(sourceIdentity, "packageName"), String(value, "packageName"))
            || !StringComparer.Ordinal.Equals(String(sourceIdentity, "packageVersion"), String(value, "packageVersion")))
        {
            Add(errors, "BMAD_SOURCE_IDENTITY_MISMATCH");
        }

        if (String(value, "packageName") == "bmad-method")
        {
            JsonElement[] runtimes = Array(sourceIdentity, "runtimeCompatibility").ToArray();
            if (String(value, "packageVersion") != "6.10.0"
                || sourceIdentity.GetProperty("moduleVersion"u8).ValueKind != JsonValueKind.Null
                || sourceIdentity.GetProperty("sourceFormatVersion"u8).ValueKind != JsonValueKind.Null
                || String(sourceIdentity, "archiveArtifactLabel") != "BMAD-METHOD-main.zip"
                || String(sourceIdentity, "archiveSha256") != "sha256:a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32"
                || runtimes.Length != 1
                || String(runtimes[0], "runtime") != "node"
                || String(runtimes[0], "versionRange") != ">=20.12.0")
            {
                Add(errors, "BMAD_METHOD_SOURCE_IDENTITY_MISMATCH");
            }
        }

        JsonElement[] graphs = Array(value, "configGraphs").ToArray();
        JsonElement[] resolutions = Array(value, "configResolutions").ToArray();
        string[] kinds = graphs
            .Select(graph => NullableString(graph, "graphKind"))
            .Distinct(StringComparer.Ordinal)
            .Order(StringComparer.Ordinal)
            .ToArray();
        if (!kinds.SequenceEqual(
            ["compatibility_yaml", "method_central_toml", "skill_customization_toml"],
            StringComparer.Ordinal))
        {
            Add(errors, "BMAD_CONFIG_GRAPHS_INCOMPLETE");
        }

        if (!StrictlySortedUnique(graphs.Select(ScopeKey)))
        {
            Add(errors, "BMAD_CONFIG_GRAPH_NOT_CANONICAL");
        }
        if (!StrictlySortedUnique(resolutions.Select(ScopeKey)))
        {
            Add(errors, "BMAD_CONFIG_RESOLUTION_NOT_CANONICAL");
        }

        Dictionary<string, JsonElement> graphByKey = graphs.ToDictionary(
            ScopeKey,
            static graph => graph,
            StringComparer.Ordinal);
        foreach (JsonElement resolution in resolutions)
        {
            if (!graphByKey.TryGetValue(ScopeKey(resolution), out JsonElement graph))
            {
                Add(errors, "BMAD_CONFIG_RESOLUTION_ORPHAN");
            }
            else if (!StringComparer.Ordinal.Equals(String(graph, "graphHash"), String(resolution, "graphHash"))
                || !Array(resolution, "orderedLayerHashes")
                    .Select(static item => item.GetString())
                    .SequenceEqual(
                        Array(graph, "layers").Select(layer => String(layer, "sourceHash")),
                        StringComparer.Ordinal))
            {
                Add(errors, "BMAD_CONFIG_RESOLUTION_BINDING_MISMATCH");
            }
        }

        foreach (JsonElement graph in graphs)
        {
            string kind = NullableString(graph, "graphKind");
            JsonElement scope = graph.GetProperty("scope"u8);
            string? module = String(scope, "moduleCode");
            string? skill = String(scope, "skillName");
            if ((kind == "method_central_toml" && (module is not null || skill is not null))
                || (kind == "skill_customization_toml" && (module is null || skill is null))
                || (kind == "compatibility_yaml" && module is null)
                || !StringComparer.Ordinal.Equals(
                    String(scope, "packageVersionId"),
                    String(value, "packageVersionId")))
            {
                Add(errors, "BMAD_CONFIG_SCOPE_INVALID");
            }

            JsonElement[] layers = Array(graph, "layers").ToArray();
            if (!StrictlySortedUnique(layers.Select(layer => string.Join(
                    '\0',
                    layer.GetProperty("ordinal"u8).GetInt32().ToString("D8"),
                    NullableString(layer, "sourcePath"))))
                || layers.Any(layer => !StringComparer.Ordinal.Equals(
                    String(layer, "graphKind"),
                    kind)))
            {
                Add(errors, "BMAD_CONFIG_LAYER_INVALID");
            }
        }

        if (!StrictlySortedUnique(Array(value, "modules").Select(item => NullableString(item, "moduleCode"))))
        {
            Add(errors, "BMAD_MODULE_SET_NOT_CANONICAL");
        }

        if (!StrictlySortedUnique(Array(value, "skills").Select(item => string.Join(
            '\0',
            NullableString(item, "moduleCode"),
            NullableString(item, "skillName")))))
        {
            Add(errors, "BMAD_SKILL_SET_NOT_CANONICAL");
        }

        if (!StrictlySortedUnique(Array(value, "resourceInventory").Select(item => NullableString(item, "path"))))
        {
            Add(errors, "BMAD_RESOURCE_SET_NOT_CANONICAL");
        }

        JsonElement[] resources = Array(value, "resourceInventory").ToArray();
        JsonElement[] projections = Array(value, "instructionProjections").ToArray();
        if (!StrictlySortedUnique(projections.Select(projection => NullableString(projection, "projectionId"))))
        {
            Add(errors, "BMAD_INSTRUCTION_PROJECTION_SET_NOT_CANONICAL");
        }

        var projectionHashes = new HashSet<string>(StringComparer.Ordinal);
        foreach (JsonElement projection in projections)
        {
            if (!StringComparer.Ordinal.Equals(
                    String(projection, "sourceIdentityHash"),
                    String(value, "sourceSnapshotHash"))
                || !projectionHashes.Add(NullableString(projection, "projectionHash")))
            {
                Add(errors, "BMAD_INSTRUCTION_PROJECTION_IDENTITY_MISMATCH");
            }

            JsonElement[] sourceResources = Array(projection, "sourceResources").ToArray();
            if (!StrictlySortedUnique(sourceResources.Select(source => NullableString(source, "path"))))
            {
                Add(errors, "BMAD_INSTRUCTION_PROJECTION_SOURCE_NOT_CANONICAL");
            }

            foreach (JsonElement source in new[] { projection.GetProperty("sourceEntrypoint"u8) }
                .Concat(sourceResources))
            {
                bool found = resources.Any(resource =>
                    String(resource, "path") == String(source, "path")
                    && String(resource, "contentHash") == String(source, "contentHash")
                    && String(resource, "treatment") == String(source, "treatment")
                    && String(resource, "locationKind") == "source_tree");
                if (!found)
                {
                    Add(errors, "BMAD_INSTRUCTION_PROJECTION_SOURCE_TRANSPLANT");
                }
            }

            JsonElement managed = projection.GetProperty("managedInstruction"u8);
            bool managedFound = resources.Any(resource =>
                String(resource, "path") == String(managed, "path")
                && String(resource, "contentHash") == String(managed, "contentHash")
                && String(resource, "locationKind") == "managed_projection"
                && String(resource, "contentRole") == "managed_instruction"
                && String(resource, "runtimeUse") == "instruction_data");
            if (!managedFound)
            {
                Add(errors, "BMAD_MANAGED_INSTRUCTION_TRANSPLANT");
            }
        }

        var moduleCodes = Array(value, "modules")
            .Select(module => NullableString(module, "moduleCode"))
            .ToHashSet(StringComparer.Ordinal);
        foreach (JsonElement skill in Array(value, "skills"))
        {
            if (!moduleCodes.Contains(NullableString(skill, "moduleCode")))
            {
                Add(errors, "BMAD_SKILL_MODULE_ORPHAN");
            }
            if (!DescriptorHasResource(
                value,
                NullableString(skill, "sourceEntrypointPath"),
                NullableString(skill, "sourceEntrypointHash")))
            {
                Add(errors, "BMAD_SKILL_SOURCE_TRANSPLANT");
            }

            JsonElement projection = projections.FirstOrDefault(candidate =>
                String(candidate, "projectionHash") == String(skill, "instructionProjectionHash"));
            if (projection.ValueKind == JsonValueKind.Undefined
                || String(projection.GetProperty("sourceEntrypoint"u8), "path")
                    != String(skill, "sourceEntrypointPath")
                || String(projection.GetProperty("sourceEntrypoint"u8), "contentHash")
                    != String(skill, "sourceEntrypointHash"))
            {
                Add(errors, "BMAD_SKILL_PROJECTION_TRANSPLANT");
            }
        }
    }

    private static void ValidateCatalog(
        JsonElement value,
        JsonElement? descriptor,
        List<string> errors)
    {
        if (descriptor is JsonElement sourceDescriptor
            && (!StringComparer.Ordinal.Equals(
                    String(value, "packageVersionId"),
                    String(sourceDescriptor, "packageVersionId"))
                || !StringComparer.Ordinal.Equals(
                    String(value, "descriptorHash"),
                    String(sourceDescriptor, "descriptorHash"))))
        {
            Add(errors, "BMAD_CATALOG_DESCRIPTOR_BINDING_MISMATCH");
        }

        JsonElement[] skills = Array(value, "installedSkills").ToArray();
        if (!StrictlySortedUnique(skills.Select(skill => string.Join(
            '\0',
            NullableString(skill, "moduleCode"),
            NullableString(skill, "skillName")))))
        {
            Add(errors, "BMAD_INSTALLED_SKILL_SET_NOT_CANONICAL");
        }

        var installed = new HashSet<string>(StringComparer.Ordinal);
        foreach (JsonElement skill in skills)
        {
            JsonElement[] capabilityKeys = Array(skill, "capabilityKeys").ToArray();
            bool cardinalityValid = String(skill, "actionCardinality") switch
            {
                "single_action" => capabilityKeys.Length == 1,
                "multi_action" => capabilityKeys.Length >= 2
                    && capabilityKeys.All(key =>
                        key.GetProperty("normalizedAction"u8).ValueKind == JsonValueKind.String),
                _ => false,
            };
            if (!cardinalityValid)
            {
                Add(errors, "BMAD_CAPABILITY_CARDINALITY_INVALID");
            }
            if (!StrictlySortedUnique(capabilityKeys.Select(CapabilityKey)))
            {
                Add(errors, "BMAD_CAPABILITY_SET_NOT_CANONICAL");
            }

            foreach (JsonElement key in capabilityKeys)
            {
                string encoded = CapabilityKey(key);
                if (!StringComparer.Ordinal.Equals(
                        String(key, "packageVersionId"),
                        String(value, "packageVersionId"))
                    || !StringComparer.Ordinal.Equals(
                        String(key, "moduleCode"),
                        String(skill, "moduleCode"))
                    || !StringComparer.Ordinal.Equals(
                        String(key, "skillName"),
                        String(skill, "skillName"))
                    || !installed.Add(encoded))
                {
                    Add(errors, "BMAD_CAPABILITY_KEY_COLLISION");
                }
            }

            if (descriptor is JsonElement descriptorValue)
            {
                JsonElement descriptorSkill = Array(descriptorValue, "skills").FirstOrDefault(candidate =>
                    String(candidate, "moduleCode") == String(skill, "moduleCode")
                    && String(candidate, "skillName") == String(skill, "skillName"));
                if (descriptorSkill.ValueKind == JsonValueKind.Undefined
                    || String(descriptorSkill, "sourceEntrypointHash") != String(skill, "sourceEntrypointHash")
                    || String(descriptorSkill, "resourceSetHash") != String(skill, "resourceSetHash")
                    || String(descriptorSkill, "skillDescriptorHash") != String(skill, "skillDescriptorHash")
                    || String(descriptorSkill.GetProperty("executionProfile"u8), "profileHash")
                        != String(skill, "executionProfileHash")
                    || String(descriptorSkill, "instructionProjectionHash")
                        != String(skill, "instructionProjectionHash")
                    || String(descriptorSkill, "distributionProfile") != String(skill, "distributionProfile")
                    || String(descriptorSkill, "installProfile") != String(skill, "installProfile")
                    || String(descriptorSkill.GetProperty("executionProfile"u8), "entrypointKind")
                        != String(skill, "entrypointKind")
                    || String(descriptorSkill.GetProperty("executionProfile"u8), "validationProfile")
                        != String(skill, "validationProfile"))
                {
                    Add(errors, "BMAD_INSTALLED_SKILL_TRANSPLANT");
                }
            }
        }

        var dependencies = new HashSet<string>(StringComparer.Ordinal);
        JsonElement[] dependencyAvailability = Array(value, "dependencyAvailability").ToArray();
        if (!StrictlySortedUnique(dependencyAvailability.Select(dependency =>
            CapabilityKey(dependency.GetProperty("capabilityKey"u8)))))
        {
            Add(errors, "BMAD_DEPENDENCY_SET_NOT_CANONICAL");
        }
        foreach (JsonElement dependency in dependencyAvailability)
        {
            string encoded = CapabilityKey(dependency.GetProperty("capabilityKey"u8));
            if (installed.Contains(encoded) || !dependencies.Add(encoded))
            {
                Add(errors, "BMAD_CAPABILITY_KEY_COLLISION");
            }
        }

        JsonElement helpGraph = value.GetProperty("helpActionGraph"u8);
        JsonElement roster = value.GetProperty("agentRoster"u8);
        JsonElement[] actions = Array(helpGraph, "actions").ToArray();
        if (!StrictlySortedUnique(actions.Select(action =>
            CapabilityKey(action.GetProperty("capabilityKey"u8)))))
        {
            Add(errors, "BMAD_HELP_ACTION_SET_NOT_CANONICAL");
        }
        if (String(helpGraph, "packageVersionId") != String(value, "packageVersionId")
            || String(roster, "packageVersionId") != String(value, "packageVersionId"))
        {
            Add(errors, "BMAD_CATALOG_PACKAGE_BINDING_MISMATCH");
        }
        foreach (JsonElement action in actions)
        {
            string encoded = CapabilityKey(action.GetProperty("capabilityKey"u8));
            if (!installed.Contains(encoded) && !dependencies.Contains(encoded))
            {
                Add(errors, "BMAD_HELP_ORPHAN");
            }
        }

        JsonElement[] agents = Array(roster, "agents").ToArray();
        if (!StrictlySortedUnique(agents.Select(agent => string.Join(
            '\0',
            NullableString(agent, "moduleCode"),
            NullableString(agent, "agentCode")))))
        {
            Add(errors, "BMAD_AGENT_ROSTER_NOT_CANONICAL");
        }
        if (agents.Length != ExpectedAgentRecordHashes.Count
            || agents.Any(agent => !AgentRecordHashIsExact(agent)))
        {
            Add(errors, "BMAD_AGENT_ROSTER_BINDING_MISMATCH");
        }

        foreach (JsonElement agent in agents)
        {
            var menuCodes = new HashSet<string>(StringComparer.Ordinal);
            long previousOrdinal = -1;
            foreach (JsonElement item in Array(agent, "menuItems"))
            {
                string code = NullableString(item, "menuCode");
                long ordinal = item.GetProperty("sourceOrdinal"u8).GetInt64();
                if (!menuCodes.Add(code) || ordinal <= previousOrdinal)
                {
                    Add(errors, "BMAD_MENU_SCOPE_AMBIGUOUS");
                }

                previousOrdinal = ordinal;
                JsonElement target = item.GetProperty("target"u8);
                if (!StringComparer.Ordinal.Equals(
                    String(target, "sourceCustomizationGraphHash"),
                    String(agent, "personaCustomizationGraphHash")))
                {
                    Add(errors, "BMAD_MENU_TARGET_TRANSPLANT");
                }

                if (String(target, "targetKind") == "skill_target")
                {
                    string encoded = CapabilityKey(target.GetProperty("capabilityKey"u8));
                    if (!installed.Contains(encoded) && !dependencies.Contains(encoded))
                    {
                        Add(errors, "BMAD_AGENT_MENU_ORPHAN");
                    }
                }
                else if (String(target, "targetKind") == "prompt_reference"
                    && descriptor is JsonElement promptDescriptor
                    && !DescriptorHasResource(
                        promptDescriptor,
                        NullableString(target, "sourceLocalMemberLabel"),
                        NullableString(target, "sourceMemberHash")))
                {
                    Add(errors, "BMAD_PROMPT_REFERENCE_TRANSPLANT");
                }
            }

            if (descriptor is JsonElement descriptorValue)
            {
                JsonElement module = Array(descriptorValue, "modules").FirstOrDefault(candidate =>
                    String(candidate, "moduleCode") == String(agent, "moduleCode"));
                if (module.ValueKind == JsonValueKind.Undefined
                    || String(module, "metadataSourceHash") != String(agent, "moduleSourceHash"))
                {
                    Add(errors, "BMAD_AGENT_MODULE_HASH_MISMATCH");
                }
                string persona = NullableString(agent, "personaSourceHash");
                bool exists = Array(descriptorValue, "resourceInventory").Any(resource =>
                    StringComparer.Ordinal.Equals(String(resource, "contentHash"), persona));
                if (!exists)
                {
                    Add(errors, "BMAD_PERSONA_HASH_MISMATCH");
                }
                string customization = NullableString(agent, "customizationSourceHash");
                bool customizationExists = Array(descriptorValue, "resourceInventory").Any(resource =>
                    StringComparer.Ordinal.Equals(String(resource, "contentHash"), customization));
                if (!customizationExists)
                {
                    Add(errors, "BMAD_CUSTOMIZATION_HASH_MISMATCH");
                }
            }
        }
    }

    private static bool DescriptorHasResource(JsonElement descriptor, string path, string hash) =>
        Array(descriptor, "resourceInventory").Any(resource =>
            StringComparer.Ordinal.Equals(String(resource, "path"), path)
            && StringComparer.Ordinal.Equals(String(resource, "contentHash"), hash));

    private static void ValidateMethod(
        JsonElement value,
        JsonElement? catalogContext,
        List<string> errors)
    {
        JsonElement profile = value.GetProperty("executionProfile"u8);
        JsonElement capability = value.GetProperty("capabilityKey"u8);
        if (String(profile, "profileHash") != String(value, "executionProfileHash")
            || String(profile, "validationProfile") != String(value, "validationProfile"))
        {
            Add(errors, "BMAD_METHOD_PROFILE_BINDING_MISMATCH");
        }

        bool isHelp = String(value, "methodShape") == "no_agent_direct";
        JsonElement[] actions = Array(profile.GetProperty("invocationModes"u8), "actions").ToArray();
        if (isHelp)
        {
            if (String(capability, "moduleCode") != "core"
                || String(capability, "skillName") != "bmad-help"
                || capability.GetProperty("normalizedAction"u8).ValueKind != JsonValueKind.Null
                || String(value.GetProperty("agentBinding"u8), "bindingKind") != "no_agent"
                || value.GetProperty("agentRosterHash"u8).ValueKind != JsonValueKind.Null
                || String(profile, "entrypointKind") != "direct"
                || actions.Length != 0
                || String(value, "validationProfile") != "MethodOfficialSkillV6")
            {
                Add(errors, "BMAD_HELP_BINDING_MISMATCH");
            }
        }
        else
        {
            JsonElement binding = value.GetProperty("agentBinding"u8);
            if (String(capability, "moduleCode") != "bmm"
                || String(capability, "skillName") != "bmad-architecture"
                || String(capability, "normalizedAction") != "create"
                || String(profile, "entrypointKind") != "step_jit"
                || actions.Length != 1
                || actions[0].GetString() != "create"
                || String(profile.GetProperty("resourcePolicy"u8), "resourceTiming")
                    != "current_step_only"
                || String(value, "validationProfile") != "MethodStepWorkflowV6"
                || String(binding, "rosterHash") != String(value, "agentRosterHash")
                || String(binding, "moduleSourceHash") != BmmModuleHash
                || String(binding, "personaHash") != ArchitectManagedPersonaHash
                || String(binding, "customizationGraphHash") != ArchitectCustomizationHash
                || !SameCapability(binding.GetProperty("menuCapabilityKey"u8), capability))
            {
                Add(errors, "BMAD_ARCHITECT_BINDING_MISMATCH");
            }
        }

        if (catalogContext is JsonElement catalog
            && String(catalog, "schemaVersion") == "sapphirus.bmad-capability-catalog.v1")
        {
            JsonElement installed = Array(catalog, "installedSkills").FirstOrDefault(skill =>
                Array(skill, "capabilityKeys").Any(key => SameCapability(key, capability)));
            if (String(catalog, "packageVersionId") != String(value, "packageVersionId")
                || String(catalog, "catalogHash") != String(value, "capabilityCatalogHash")
                || installed.ValueKind == JsonValueKind.Undefined
                || String(installed, "instructionProjectionHash")
                    != String(value, "instructionProjectionHash")
                || String(installed, "resourceSetHash") != String(value, "resourceSetHash")
                || String(installed, "executionProfileHash") != String(value, "executionProfileHash")
                || String(installed, "validationProfile") != String(value, "validationProfile")
                || String(installed, "distributionProfile") != String(value, "distributionProfile")
                || String(installed, "installProfile") != String(value, "installProfile"))
            {
                Add(errors, "BMAD_METHOD_CATALOG_BINDING_MISMATCH");
            }

            if (!isHelp)
            {
                JsonElement binding = value.GetProperty("agentBinding"u8);
                JsonElement roster = catalog.GetProperty("agentRoster"u8);
                JsonElement agent = Array(roster, "agents").FirstOrDefault(candidate =>
                    String(candidate, "agentCode") == String(binding, "agentCode"));
                JsonElement menu = agent.ValueKind == JsonValueKind.Undefined
                    ? default
                    : Array(agent, "menuItems").FirstOrDefault(item =>
                        String(item, "menuCode") == String(binding, "menuCode"));
                if (String(roster, "rosterHash") != String(value, "agentRosterHash")
                    || agent.ValueKind == JsonValueKind.Undefined
                    || menu.ValueKind == JsonValueKind.Undefined
                    || String(agent, "agentRecordHash") != String(binding, "agentRecordHash")
                    || String(agent, "moduleSourceHash") != String(binding, "moduleSourceHash")
                    || String(agent, "name") != String(binding, "agentName")
                    || String(agent, "title") != String(binding, "agentTitle")
                    || String(agent, "personaCustomizationGraphHash")
                        != String(binding, "customizationGraphHash")
                    || String(menu, "sourceMenuItemHash") != String(binding, "menuItemHash")
                    || String(menu.GetProperty("target"u8), "targetKind") != "skill_target"
                    || !SameCapability(
                        menu.GetProperty("target"u8).GetProperty("capabilityKey"u8),
                        capability))
                {
                    Add(errors, "BMAD_METHOD_AGENT_CATALOG_TRANSPLANT");
                }
            }
        }

        JsonElement[] checkpoints = Array(value, "checkpoints").ToArray();
        var checkpointIds = new HashSet<string>(StringComparer.Ordinal);
        var checkpointDecisions = new HashSet<string>(StringComparer.Ordinal);
        for (int index = 0; index < checkpoints.Length; index++)
        {
            JsonElement checkpoint = checkpoints[index];
            if (String(checkpoint, "sessionId") != String(value, "sessionId")
                || checkpoint.GetProperty("turnOrdinal"u8).GetInt32() != index + 1
                || !checkpointIds.Add(NullableString(checkpoint, "checkpointId"))
                || !checkpointDecisions.Add(NullableString(checkpoint, "contextDecisionId"))
                || !SameCapability(checkpoint.GetProperty("capabilityKey"u8), capability)
                || String(checkpoint, "contextDigest") != String(value, "contextDigest")
                || String(checkpoint, "modelBindingHash")
                    != String(value.GetProperty("modelBinding"u8), "bindingHash"))
            {
                Add(errors, "BMAD_TURN_ORDINAL_INVALID");
            }
            VerifyHash(checkpoint, errors);
        }

        JsonElement ledger = value.GetProperty("contextLedger"u8);
        JsonElement[] ledgerEntries = Array(ledger, "entries").ToArray();
        JsonElement[] consumptions = Array(value, "decisionConsumptions").ToArray();
        if (String(ledger, "sessionId") != String(value, "sessionId")
            || ledgerEntries.Length != consumptions.Length
            || checkpoints.Length != consumptions.Length)
        {
            Add(errors, "BMAD_CONTEXT_LEDGER_BINDING_MISMATCH");
        }

        var ledgerByDecision = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        for (int index = 0; index < ledgerEntries.Length; index++)
        {
            JsonElement entry = ledgerEntries[index];
            string decision = NullableString(entry, "contextDecisionId");
            if (entry.GetProperty("reviewOrdinal"u8).GetInt32() != index + 1
                || !ledgerByDecision.TryAdd(decision, entry)
                || String(entry, "contextDigest") != String(value, "contextDigest")
                || String(entry, "resourceSetHash") != String(value, "resourceSetHash")
                || String(entry, "packageDescriptorHash") != String(value, "packageDescriptorHash")
                || String(entry, "instructionProjectionHash")
                    != String(value, "instructionProjectionHash")
                || String(entry, "configResolutionHash") != String(value, "configResolutionHash")
                || String(entry, "customizationHash") != String(value, "customizationHash")
                || String(entry, "modelBindingHash")
                    != String(value.GetProperty("modelBinding"u8), "bindingHash")
                || String(entry, "methodSchemaHash") != String(value, "methodSchemaHash")
                || String(entry, "executionProfileHash") != String(value, "executionProfileHash")
                || String(entry, "validationProfileHash") != String(value, "validationProfileHash"))
            {
                Add(errors, "BMAD_CONTEXT_LEDGER_BINDING_MISMATCH");
            }
        }

        var decisions = new HashSet<string>(StringComparer.Ordinal);
        var invocations = new HashSet<string>(StringComparer.Ordinal);
        foreach (JsonElement consumption in consumptions)
        {
            string decision = NullableString(consumption, "decisionId");
            bool hasEntry = ledgerByDecision.TryGetValue(decision, out JsonElement entry);
            bool exact = String(consumption, "sessionId") == String(value, "sessionId")
                && checkpointDecisions.Contains(decision)
                && hasEntry
                && String(entry, "manifestHash") == String(consumption, "manifestHash")
                && String(entry, "consentHash") == String(consumption, "consentHash")
                && String(consumption, "packageDescriptorHash")
                    == String(value, "packageDescriptorHash")
                && String(consumption, "packageSourceHash") == String(value, "packageSourceHash")
                && String(consumption, "instructionProjectionHash")
                    == String(value, "instructionProjectionHash")
                && String(consumption, "capabilityCatalogHash")
                    == String(value, "capabilityCatalogHash")
                && SameCapability(consumption.GetProperty("capabilityKey"u8), capability)
                && String(consumption, "contextDigest") == String(value, "contextDigest")
                && String(consumption, "distributionProfile") == String(value, "distributionProfile")
                && String(consumption, "installProfile") == String(value, "installProfile")
                && String(consumption, "executionProfileHash")
                    == String(value, "executionProfileHash")
                && String(consumption, "validationProfileHash")
                    == String(value, "validationProfileHash")
                && String(consumption, "configResolutionHash")
                    == String(value, "configResolutionHash")
                && String(consumption, "customizationHash") == String(value, "customizationHash")
                && String(consumption, "resourceSetHash") == String(value, "resourceSetHash")
                && SameModelBinding(
                    consumption.GetProperty("modelBinding"u8),
                    value.GetProperty("modelBinding"u8))
                && String(consumption, "methodSchemaHash") == String(value, "methodSchemaHash");
            if (!decisions.Add(decision)
                || !invocations.Add(NullableString(consumption, "invocationId"))
                || !exact)
            {
                Add(errors, "BMAD_CONTEXT_DECISION_REUSED");
            }
        }
    }

    private static bool IsWindowsReservedSegment(string segment)
    {
        string stem = segment.Split('.', 2)[0].ToLowerInvariant();
        return stem is "con" or "prn" or "aux" or "nul"
            || (stem.Length == 4
                && (stem.StartsWith("com", StringComparison.Ordinal)
                    || stem.StartsWith("lpt", StringComparison.Ordinal))
                && stem[3] is >= '1' and <= '9');
    }

    private static bool IsCapabilityReferencePath(string path)
    {
        const string prefix = "references/";
        if (!path.StartsWith(prefix, StringComparison.Ordinal)
            || !path.EndsWith(".md", StringComparison.Ordinal))
        {
            return false;
        }
        string stem = path[prefix.Length..^3];
        return stem.Length > 0
            && stem[0] is >= 'a' and <= 'z'
            && stem.All(character =>
                character is >= 'a' and <= 'z'
                || character is >= '0' and <= '9'
                || character == '-');
    }

    private static bool BuilderPathIsValid(string path)
    {
        string[] segments = path.Split('/');
        return StringComparer.Ordinal.Equals(path, path.Normalize(NormalizationForm.FormC))
            && Encoding.UTF8.GetByteCount(path) <= 240
            && segments.Length <= 16
            && segments.All(segment =>
                Encoding.UTF8.GetByteCount(segment) <= 120
                && !segment.EndsWith(".", StringComparison.Ordinal)
                && !segment.EndsWith(' ')
                && !IsWindowsReservedSegment(segment));
    }

    private static void ValidateBuilder(JsonElement value, List<string> errors)
    {
        string kind = NullableString(value, "builderKind");
        string expectedProfile = kind == "agent"
            ? "BuilderAgentV2Stateless"
            : "BuilderOutcomeSkillV2";
        if (!StringComparer.Ordinal.Equals(String(value, "validationProfile"), expectedProfile))
        {
            Add(errors, "BMAD_PROFILE_AMBIGUOUS");
        }

        if (value.TryGetProperty("authoringAction"u8, out JsonElement authoringAction)
            && String(authoringAction, "builderKind") != kind)
        {
            Add(errors, "BMAD_ACTION_UNSUPPORTED");
        }

        if (String(value, "objectKind") == "revision")
        {
            JsonElement fileSet = value.GetProperty("proposedFileSet"u8);
            if (String(fileSet, "limitProfile") != BuilderLimitProfile)
            {
                Add(errors, "BMAD_BUILDER_LIMIT_PROFILE_MISMATCH");
            }

            var folded = new HashSet<string>(StringComparer.Ordinal);
            var paths = new List<string>();
            int totalBytes = 0;
            foreach (JsonElement file in Array(fileSet, "files"))
            {
                string path = NullableString(file, "path");
                if (!BuilderPathIsValid(path))
                {
                    Add(errors, "BMAD_BUILDER_PATH_INVALID");
                }
                if (!folded.Add(path.ToLowerInvariant()))
                {
                    Add(errors, "BMAD_BUILDER_PATH_COLLISION");
                }
                int fileBytes = Encoding.UTF8.GetByteCount(NullableString(file, "content"));
                if (fileBytes > 262_144)
                {
                    Add(errors, "BMAD_BUILDER_FILE_TOO_LARGE");
                }
                totalBytes += fileBytes;
                paths.Add(path);
            }
            if (totalBytes > 1_048_576)
            {
                Add(errors, "BMAD_BUILDER_TOTAL_TOO_LARGE");
            }

            string[] sortedPaths = paths.Order(StringComparer.Ordinal).ToArray();
            bool inventoryValid = kind == "workflow"
                ? sortedPaths.SequenceEqual(["SKILL.md"], StringComparer.Ordinal)
                : new[]
                    {
                        "SKILL.md",
                        "customize.toml",
                        "references/prompt-quality-canon.md",
                    }.All(required => paths.Contains(required, StringComparer.Ordinal))
                    && paths.All(path => path is "SKILL.md" or "customize.toml"
                        or "references/prompt-quality-canon.md"
                        || IsCapabilityReferencePath(path))
                    && paths.Count(path => path.StartsWith("references/", StringComparison.Ordinal)
                        && path != "references/prompt-quality-canon.md") <= 13;
            if (!inventoryValid)
            {
                Add(errors, "BMAD_BUILDER_INVENTORY_INVALID");
            }
        }

        if (String(value, "objectKind") != "analysis")
        {
            return;
        }

        int findingCount = Array(value, "deterministicFindings").Count();
        if (value.TryGetProperty("modelLensResults"u8, out JsonElement modelLensResults))
        {
            findingCount += modelLensResults.EnumerateArray()
                .Sum(result => Array(result, "findings").Count());
        }
        if (findingCount > 512)
        {
            Add(errors, "BMAD_BUILDER_FINDING_LIMIT_EXCEEDED");
        }
        if (String(value, "analysisKind") != "model_lens")
        {
            return;
        }

        string[] expectedLenses = kind == "agent"
            ? ["leanness", "architecture", "determinism", "customization", "enhancement", "agent-cohesion"]
            : ["leanness", "architecture", "determinism", "customization", "enhancement"];
        JsonElement[] results = modelLensResults.EnumerateArray().ToArray();
        if (!results.Select(result => String(result, "lens"))
            .SequenceEqual(expectedLenses, StringComparer.Ordinal))
        {
            Add(errors, "BMAD_MODEL_LENS_SET_INVALID");
        }

        JsonElement binding = value.GetProperty("modelBinding"u8);
        foreach (JsonElement result in results)
        {
            if (String(result, "builderKind") != String(value, "builderKind")
                || String(result, "revisionId") != String(value, "revisionId")
                || String(result, "revisionHash") != String(value, "revisionHash")
                || String(result, "sourceMemberSetHash") != String(value, "sourceMemberSetHash")
                || String(result, "instructionProjectionSetHash")
                    != String(value, "instructionProjectionSetHash")
                || String(result, "deterministicFactsHash") != String(value, "deterministicFactsHash")
                || String(result, "modelHash") != String(binding, "modelHash")
                || String(result, "deploymentHash") != String(binding, "deploymentHash")
                || String(result, "modelProfileHash") != String(binding, "modelProfileHash")
                || String(result, "schemaHash") != String(binding, "schemaHash")
                || String(result, "consentHash") != String(binding, "consentHash")
                || String(result, "contextDecisionConsumptionHash")
                    != String(binding, "contextDecisionConsumptionHash"))
            {
                Add(errors, "BMAD_MODEL_LENS_BINDING_MISMATCH");
            }
        }
    }
}
