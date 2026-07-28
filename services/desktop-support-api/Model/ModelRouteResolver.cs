using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// The consent bindings route resolution must reconcile, lifted out of the
/// signed version two decision.
/// </summary>
public sealed record ModelRouteConsent(
    string ModelProfileId,
    string? RequestedVariantId,
    string CatalogVersion,
    string RequestMode,
    string Purpose,
    string ModelRole,
    string? CanonicalOutputSchemaId,
    string? CanonicalOutputSchemaHash,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string Region,
    string RetentionMode);

/// <summary>
/// The routing-relevant projection of a version two model access request,
/// mirroring <c>packages/contracts/schemas/model-access-request-v2.schema.json</c>.
///
/// The wire record itself belongs in <c>Contracts.cs</c> alongside the other
/// hand-written wire types; this is the server-side view resolution needs, and
/// it deliberately carries no property that could name a deployment, an
/// endpoint, an API version, or a credential.
/// </summary>
public sealed record ModelRouteRequest(
    string RequestId,
    string RegistrationId,
    string Purpose,
    string ModelRole,
    string RequestMode,
    string ModelProfileId,
    string? RequestedVariantId,
    string CatalogVersion,
    string ModelProfileHash,
    string ModelCapabilityHash,
    string? CanonicalOutputSchemaId,
    string? CanonicalOutputSchemaHash,
    string RetentionMode,
    string BudgetClass,
    bool UsesImageInput,
    bool UsesPdfInput,
    int AttachmentBytes,
    int ToolCount,
    int EstimatedInputTokens,
    int RequestedOutputTokens,
    ModelRouteConsent Consent);

/// <summary>
/// Turns a named profile and a catalog epoch into an immutable resolved route.
///
/// <para>
/// Resolution is the single place where a desktop-supplied identifier becomes a
/// deployment. Everything it consumes is server-owned: the current effective
/// catalog, the verified tenant policy, and the subject's entitlement. The
/// request contributes a profile identifier, a variant identifier, a catalog
/// epoch, and two binding hashes — and every one of them is checked against
/// what the server computed rather than trusted.
/// </para>
/// <para>
/// Catalog selection alone is not authority. The catalog may be up to its
/// staleness budget old, so <see cref="VerifyBeforeDispatch"/> rechecks
/// deployment existence and model identity against the inventory as it is at
/// the moment of dispatch. A deployment that was replaced between selection and
/// dispatch fails closed rather than serving a different model under the name
/// the user chose.
/// </para>
/// </summary>
public static class ModelRouteResolver
{
    public static ModelRouteResolution Resolve(
        ModelRouteRequest request,
        EffectiveModelCatalog catalog,
        ModelEntitlementSnapshot entitlement,
        PolicySnapshot policy,
        string region)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(catalog);
        ArgumentNullException.ThrowIfNull(entitlement);
        ArgumentNullException.ThrowIfNull(policy);
        ArgumentException.ThrowIfNullOrWhiteSpace(region);

        // 1. The epoch. A request built against a catalog the server has since
        //    replaced is refused outright: every hash below is only meaningful
        //    relative to one epoch.
        if (!string.Equals(
            request.CatalogVersion,
            catalog.Projection.CatalogVersion,
            StringComparison.Ordinal))
        {
            return Failure(ModelRouteErrorCodes.CatalogStale);
        }

        // 2. The profile. Entries carry hidden profiles too, so an adapted
        //    legacy request can resolve one the picker never offered; a hidden
        //    profile is still subject to every check below.
        EffectiveModelEntry? entry = catalog.Find(request.ModelProfileId);
        if (entry is null)
        {
            return Failure(ModelRouteErrorCodes.ProfileUnavailable);
        }
        // An entitlement gap is named precisely rather than folded into the
        // weaker "unavailable": one is fixable by a person, the other is not.
        if (entry.Blocked == BlockedAvailability.NotEntitled
            || !entitlement.EntitledProfileIds.Contains(request.ModelProfileId))
        {
            return Failure(ModelRouteErrorCodes.ProfileNotEntitled);
        }
        if (!entry.Selectable)
        {
            return Failure(ModelRouteErrorCodes.ProfileUnavailable);
        }

        ModelProfile profile = entry.Profile;

        // 3. The bindings. Both are recomputed from the catalog the server
        //    published, so a request that edited a capability or a limit on its
        //    way through fails here rather than at the provider.
        string profileBindingHash = ComputeProfileBindingHash(
            catalog.Projection.CatalogVersion,
            entry);
        string capabilityBindingHash = ComputeCapabilityBindingHash(
            catalog.Projection.CatalogVersion,
            profile.ProfileId,
            entry.Capabilities);
        if (!string.Equals(request.ModelProfileHash, profileBindingHash, StringComparison.Ordinal))
        {
            return Failure(ModelRouteErrorCodes.ProfileBindingMismatch);
        }
        if (!string.Equals(
            request.ModelCapabilityHash,
            capabilityBindingHash,
            StringComparison.Ordinal))
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }

        // 4. The variant. Null asks for the profile default; anything else must
        //    exist in this profile, because a variant identifier from another
        //    profile is exactly how an output budget gets silently raised.
        ModelProfileVariant? variant = request.RequestedVariantId is null
            ? profile.DefaultVariant
            : profile.Variants.FirstOrDefault(candidate => string.Equals(
                candidate.VariantId,
                request.RequestedVariantId,
                StringComparison.Ordinal));
        if (variant is null)
        {
            return Failure(ModelRouteErrorCodes.VariantInvalid);
        }
        if (variant.MaxOutputTokens > profile.Limits.MaxOutputTokens)
        {
            return Failure(ModelRouteErrorCodes.VariantInvalid);
        }

        // 5. Purpose, role, mode, and output schema against the organization's
        //    allowlists.
        if (!profile.PurposeAllowlist.Contains(request.Purpose, StringComparer.Ordinal)
            || !profile.ModelRoleAllowlist.Contains(request.ModelRole, StringComparer.Ordinal)
            || !profile.RequestModeAllowlist.Contains(request.RequestMode, StringComparer.Ordinal))
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }
        if (request.RequestMode == "structured_response")
        {
            if (!entry.Capabilities.StructuredOutput
                || request.CanonicalOutputSchemaId is null
                || request.CanonicalOutputSchemaHash is null
                || !profile.CanonicalOutputSchemaAllowlist.Contains(
                    request.CanonicalOutputSchemaId,
                    StringComparer.Ordinal))
            {
                return Failure(ModelRouteErrorCodes.CapabilityMismatch);
            }
        }
        else if (request.CanonicalOutputSchemaId is not null
            || request.CanonicalOutputSchemaHash is not null)
        {
            // A streaming chat request that also carries a structured output
            // schema is asking for two different result contracts at once.
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }

        // 6. Modalities, attachments, and tools against the narrowed capability
        //    set — never against what the profile merely wished for.
        if (request.UsesImageInput
            && (!entry.Capabilities.ImageInput
                || !profile.SupportedModalities.Contains("image", StringComparer.Ordinal)))
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }
        if (request.UsesPdfInput
            && (!entry.Capabilities.PdfInput
                || !profile.SupportedModalities.Contains("pdf", StringComparer.Ordinal)))
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }
        if (request.AttachmentBytes > 0 && !entry.Capabilities.Attachments)
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }
        if (request.ToolCount > 0 && !entry.Capabilities.ToolCalling)
        {
            return Failure(ModelRouteErrorCodes.CapabilityMismatch);
        }

        // 7. Context and output budgets.
        if (request.AttachmentBytes < 0
            || request.AttachmentBytes > profile.Limits.MaxAttachmentBytes
            || request.EstimatedInputTokens < 0
            || request.EstimatedInputTokens > profile.Limits.MaxInputTokens
            || request.RequestedOutputTokens < 0
            || request.RequestedOutputTokens > variant.MaxOutputTokens
            || (long)request.EstimatedInputTokens + request.RequestedOutputTokens
                > profile.Limits.ContextTokens)
        {
            return Failure(ModelRouteErrorCodes.ContextLimitExceeded);
        }

        // 8. Region and retention, from the policy and the profile rather than
        //    from the request.
        if (!string.Equals(request.RetentionMode, profile.RetentionMode, StringComparison.Ordinal)
            || !string.Equals(policy.RetentionMode, profile.RetentionMode, StringComparison.Ordinal)
            || !profile.AllowedRegions.Contains(region, StringComparer.Ordinal)
            || !policy.AllowedRegions.Contains(region, StringComparer.Ordinal)
            || !string.Equals(catalog.Region, region, StringComparison.Ordinal))
        {
            return Failure(ModelRouteErrorCodes.ProfileBindingMismatch);
        }

        // 9. The consent. It was signed over the profile, epoch, variant, mode,
        //    and output schema, so every one of them must still say the same
        //    thing the request says.
        if (!ConsentAgrees(request, variant.VariantId, region))
        {
            return Failure(ModelRouteErrorCodes.ProfileBindingMismatch);
        }

        ResolvedModelRoute route = new(
            RouteHash: "",
            profile.ProfileId,
            profileBindingHash,
            capabilityBindingHash,
            catalog.RegistryHash,
            catalog.Projection.CatalogVersion,
            catalog.Projection.CatalogHash,
            entry.Deployment.DeploymentName,
            entry.Deployment.DeploymentHash,
            entry.Deployment.ModelPublisher,
            entry.Deployment.ModelName,
            entry.Deployment.ModelVersion,
            variant.VariantId,
            variant.ReasoningEffort,
            variant.Verbosity,
            request.RequestMode,
            request.CanonicalOutputSchemaId,
            entry.Capabilities,
            profile.Limits.ContextTokens,
            profile.Limits.MaxInputTokens,
            variant.MaxOutputTokens,
            profile.Limits.MaxOutputBytes,
            entry.Capabilities.Attachments ? profile.Limits.MaxAttachmentBytes : 0,
            profile.RetentionMode,
            region,
            profile.RetryPolicy,
            profile.PriceProfile);

        return new ModelRouteResolution(
            route with { RouteHash = ComputeRouteHash(route) },
            "");
    }

    /// <summary>
    /// The recheck that runs immediately before dispatch. Catalog selection
    /// proves the deployment existed when the catalog was built; only this
    /// proves it exists now and still serves the model the profile qualified.
    /// </summary>
    public static bool VerifyBeforeDispatch(
        ResolvedModelRoute route,
        FoundryDeploymentSnapshot currentSnapshot,
        out string errorCode)
    {
        ArgumentNullException.ThrowIfNull(route);
        ArgumentNullException.ThrowIfNull(currentSnapshot);
        errorCode = "";
        FoundryDeployment? deployment = currentSnapshot.Deployments.FirstOrDefault(candidate =>
            string.Equals(
                candidate.DeploymentName,
                route.DeploymentName,
                StringComparison.Ordinal));
        if (deployment is null)
        {
            errorCode = ModelRouteErrorCodes.DeploymentMissing;
            return false;
        }
        if (!string.Equals(
                deployment.ModelPublisher,
                route.ModelPublisher,
                StringComparison.Ordinal)
            || !string.Equals(deployment.ModelName, route.ModelName, StringComparison.Ordinal)
            || !string.Equals(
                deployment.ModelVersion,
                route.ModelVersion,
                StringComparison.Ordinal)
            || !string.Equals(
                deployment.DeploymentHash,
                route.DeploymentHash,
                StringComparison.Ordinal))
        {
            // The name survived but the thing behind it did not. Serving this
            // would mean answering with a model the organization never
            // qualified, under a name the user believes they chose.
            errorCode = ModelRouteErrorCodes.DeploymentMissing;
            return false;
        }
        return true;
    }

    /// <summary>
    /// The profile binding the desktop signs its consent over. Every input is
    /// a value the desktop received in the catalog projection and is drawn from
    /// a closed alphabet, so both sides compute the same digest without a
    /// shared canonicalizer for free text.
    /// </summary>
    public static string ComputeProfileBindingHash(
        string catalogVersion,
        EffectiveModelEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);
        List<object> variants = [];
        foreach (ModelProfileVariant variant in entry.Profile.Variants)
        {
            variants.Add(new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["maxOutputTokens"] = variant.MaxOutputTokens,
                ["reasoningEffort"] = variant.ReasoningEffort,
                ["variantId"] = variant.VariantId,
            });
        }
        return ModelProfileValidator.Digest(
            ResolvedModelRoute.ProfileBindingHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["availability"] = ModelCatalogWireNames.ToWireName(entry.Availability),
                ["capabilities"] = CapabilityDraft(entry.Capabilities),
                ["catalogVersion"] = catalogVersion,
                ["defaultVariantId"] = entry.Profile.DefaultVariant.VariantId,
                ["limits"] = new SortedDictionary<string, object?>(StringComparer.Ordinal)
                {
                    ["contextTokens"] = entry.Limits.ContextTokens,
                    ["maxAttachmentBytes"] = entry.Limits.MaxAttachmentBytes,
                    ["maxInputTokens"] = entry.Limits.MaxInputTokens,
                    ["maxOutputTokens"] = entry.Limits.MaxOutputTokens,
                },
                ["profileId"] = entry.Profile.ProfileId,
                ["variants"] = variants,
            });
    }

    /// <summary>The capability binding the desktop signs its consent over.</summary>
    public static string ComputeCapabilityBindingHash(
        string catalogVersion,
        string profileId,
        ProjectedCapabilities capabilities) =>
        ModelProfileValidator.Digest(
            ResolvedModelRoute.CapabilityBindingHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["capabilities"] = CapabilityDraft(capabilities),
                ["catalogVersion"] = catalogVersion,
                ["profileId"] = profileId,
            });

    private static string ComputeRouteHash(ResolvedModelRoute route) =>
        ModelProfileValidator.Digest(
            ResolvedModelRoute.RouteHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["canonicalOutputSchemaId"] = route.CanonicalOutputSchemaId,
                ["capabilityBindingHash"] = route.CapabilityBindingHash,
                ["catalogHash"] = route.CatalogHash,
                ["catalogVersion"] = route.CatalogVersion,
                ["deploymentBindingHash"] = route.ComputeDeploymentBindingHash(),
                ["maxOutputTokens"] = route.MaxOutputTokens,
                ["profileBindingHash"] = route.ProfileBindingHash,
                ["profileId"] = route.ProfileId,
                ["region"] = route.Region,
                ["registryHash"] = route.RegistryHash,
                ["requestMode"] = route.RequestMode,
                ["retentionMode"] = route.RetentionMode,
                ["variantId"] = route.VariantId,
            });

    private static SortedDictionary<string, object?> CapabilityDraft(
        ProjectedCapabilities capabilities) =>
        new(StringComparer.Ordinal)
        {
            ["attachments"] = capabilities.Attachments,
            ["imageInput"] = capabilities.ImageInput,
            ["pdfInput"] = capabilities.PdfInput,
            ["reasoning"] = capabilities.Reasoning,
            ["structuredOutput"] = capabilities.StructuredOutput,
            ["textInput"] = capabilities.TextInput,
            ["toolCalling"] = capabilities.ToolCalling,
        };

    private static bool ConsentAgrees(
        ModelRouteRequest request,
        string resolvedVariantId,
        string region)
    {
        ModelRouteConsent consent = request.Consent;
        return string.Equals(
                consent.ModelProfileId,
                request.ModelProfileId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.RequestedVariantId ?? resolvedVariantId,
                resolvedVariantId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.CatalogVersion,
                request.CatalogVersion,
                StringComparison.Ordinal)
            && string.Equals(consent.RequestMode, request.RequestMode, StringComparison.Ordinal)
            && string.Equals(consent.Purpose, request.Purpose, StringComparison.Ordinal)
            && string.Equals(consent.ModelRole, request.ModelRole, StringComparison.Ordinal)
            && string.Equals(
                consent.CanonicalOutputSchemaId,
                request.CanonicalOutputSchemaId,
                StringComparison.Ordinal)
            && string.Equals(
                consent.CanonicalOutputSchemaHash,
                request.CanonicalOutputSchemaHash,
                StringComparison.Ordinal)
            && string.Equals(
                consent.ModelProfileHash,
                request.ModelProfileHash,
                StringComparison.Ordinal)
            && string.Equals(
                consent.ModelCapabilityHash,
                request.ModelCapabilityHash,
                StringComparison.Ordinal)
            && string.Equals(consent.Region, region, StringComparison.Ordinal)
            && string.Equals(
                consent.RetentionMode,
                request.RetentionMode,
                StringComparison.Ordinal);
    }

    private static ModelRouteResolution Failure(string errorCode) => new(null, errorCode);
}
