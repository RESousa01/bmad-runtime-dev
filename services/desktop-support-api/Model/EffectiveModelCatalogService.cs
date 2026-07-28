using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// What the subject and device are entitled to, reduced to the only two things
/// the catalog is allowed to know: an opaque class name used in the catalog
/// digest, and the set of profile identifiers the subject may invoke.
///
/// It carries no group name, directory object identifier, role assignment, or
/// licence identifier, so entitlement filtering cannot leak the reason a
/// profile was withheld even by accident.
/// </summary>
public sealed record ModelEntitlementSnapshot(
    string EntitlementClass,
    IReadOnlySet<string> EntitledProfileIds,
    string? OrganizationDefaultProfileId);

/// <summary>Resolves the entitlement of one authenticated subject partition.</summary>
public interface IModelEntitlementSource
{
    ValueTask<ModelEntitlementSnapshot> GetAsync(
        string subjectPartition,
        CancellationToken cancellationToken);
}

/// <summary>Supplies the verified tenant policy snapshot to the catalog.</summary>
public interface IModelCatalogPolicySource
{
    ValueTask<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken);
}

/// <summary>
/// A deployment inventory read. <see cref="HasPreviousSnapshot"/> is what tells
/// "the catalog has never been reachable" apart from "the catalog was
/// reachable and its snapshot has now aged past the staleness budget"; the two
/// are different problems for the operator and different messages for the user.
/// </summary>
public sealed record ModelDeploymentRead(
    FoundryCatalogState State,
    FoundryDeploymentSnapshot? Snapshot,
    bool HasPreviousSnapshot);

/// <summary>Supplies the normalized Foundry deployment inventory.</summary>
public interface IModelCatalogDeploymentSource
{
    ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken);
}

/// <summary>
/// A fixed policy snapshot. Development composes one from the validated support
/// plane options; tests compose one directly.
/// </summary>
public sealed class StaticModelCatalogPolicySource(PolicySnapshot snapshot)
    : IModelCatalogPolicySource
{
    /// <summary>
    /// The Development policy: the configured region, no deployment allowlist,
    /// and the one supported retention mode. It is not signed and is never
    /// composed outside Development.
    /// </summary>
    public static PolicySnapshot DevelopmentSnapshot(
        string region,
        DateTimeOffset refreshedAt) => new(
            "policy_00000000000000000000000000",
            PolicyVersion: 1,
            SystemBrowserFallbackAllowed: false,
            MaximumContextBytes: 512 * 1024,
            MaximumContextItems: 64,
            [region],
            [],
            ModelProfile.SupportedRetentionMode,
            refreshedAt);

    public ValueTask<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken) =>
        ValueTask.FromResult(snapshot);
}

/// <summary>Adapts the verified policy provider to the catalog seam.</summary>
public sealed class AppConfigurationCatalogPolicySource(
    AppConfigurationPolicyProvider provider) : IModelCatalogPolicySource
{
    public async ValueTask<PolicySnapshot> GetSnapshotAsync(
        CancellationToken cancellationToken) =>
        await provider.GetSnapshotAsync(cancellationToken).ConfigureAwait(false);
}

/// <summary>Adapts the Foundry deployment catalog cache to the catalog seam.</summary>
public sealed class FoundryCatalogDeploymentSource(
    FoundryDeploymentCatalogCache cache) : IModelCatalogDeploymentSource
{
    public async ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken)
    {
        FoundryCatalogRead read = await cache.GetAsync(cancellationToken).ConfigureAwait(false);
        FoundryCatalogStatus status = cache.Peek();
        return new ModelDeploymentRead(
            read.State,
            read.Snapshot,
            status.GeneratedAt is not null);
    }
}

/// <summary>
/// The deployment source for a host composed without Foundry discovery. It
/// reports an unavailable inventory rather than an empty one, because an empty
/// inventory would read as "the organization deployed nothing" and would
/// quietly blank the picker.
/// </summary>
public sealed class UnavailableModelCatalogDeploymentSource : IModelCatalogDeploymentSource
{
    public ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken) =>
        ValueTask.FromResult(new ModelDeploymentRead(
            FoundryCatalogState.Unavailable,
            Snapshot: null,
            HasPreviousSnapshot: false));
}

/// <summary>
/// One resolvable profile, joined to the deployment that actually backs it.
///
/// This is server-only. It is the only place outside <see cref="ModelProfile"/>
/// where a deployment name exists at catalog time, and it never leaves the
/// support plane: <see cref="EffectiveModelCatalog.Projection"/> is what the
/// desktop receives.
/// </summary>
public sealed record EffectiveModelEntry(
    ModelProfile Profile,
    FoundryDeployment Deployment,
    ProjectedCapabilities Capabilities,
    ProjectedLimits Limits,
    SelectableAvailability Availability,
    string? StatusDetail,
    bool Selectable,
    BlockedAvailability? Blocked);

/// <summary>
/// The server-side effective catalog: the sanitized projection the desktop
/// receives, plus the joined entries route resolution needs.
/// </summary>
public sealed record EffectiveModelCatalog(
    ModelCatalogProjection Projection,
    IReadOnlyList<EffectiveModelEntry> Entries,
    long RegistryVersion,
    string RegistryHash,
    string CapabilityHash,
    string EntitlementClass,
    string Region)
{
    public EffectiveModelEntry? Find(string profileId) =>
        Entries.FirstOrDefault(entry =>
            string.Equals(entry.Profile.ProfileId, profileId, StringComparison.Ordinal));
}

/// <summary>Stable catalog problem codes.</summary>
public static class ModelCatalogErrorCodes
{
    public const string Unavailable = "model_catalog_unavailable";
    public const string Stale = "model_catalog_stale";
    public const string ContractUpgradeRequired = "model_contract_upgrade_required";

    /// <summary>
    /// Nothing is selectable and there is nothing visible to explain why. The
    /// plan's vocabulary names three codes for four conditions; this is the
    /// fourth. It is deliberately distinct from
    /// <see cref="Unavailable"/> because an entitlement gap is fixable by a
    /// person and an unreachable inventory is not.
    /// </summary>
    public const string NoEntitledProfiles = "model_catalog_no_entitled_profiles";
}

public sealed record ModelCatalogResult(EffectiveModelCatalog? Catalog, string ErrorCode)
{
    public bool IsSuccess => Catalog is not null;
}

/// <summary>
/// The effective model catalog: the intersection of the Foundry deployment
/// inventory, the enabled signed organization profiles, tenant policy, the
/// current region, qualification evidence, and the subject's entitlement.
///
/// <para>
/// Profiles join to deployments by exact deployment name and only after all
/// three parts of the expected model identity are confirmed. A deployment may
/// confirm or narrow a capability; it can never broaden one, because the
/// projection is the boolean AND of what the organization qualified with what
/// the provider currently advertises.
/// </para>
/// <para>
/// A blocked profile never becomes the default and never appears in the
/// selectable list. When it is visible it appears in
/// <c>unavailableProfiles</c> carrying only its identifier, its display name,
/// the blocked state, and a recovery action — which is what lets the desktop
/// explain a disappearance instead of silently switching models.
/// </para>
/// </summary>
public sealed class EffectiveModelCatalogService
{
    /// <summary>Hash purpose for the catalog digest.</summary>
    public const string CatalogHashPurpose = "model-catalog";

    /// <summary>Hash purpose for the qualification-evidence digest.</summary>
    public const string QualificationHashPurpose = "model-catalog-qualification";

    /// <summary>Largest catalog epoch the canonical version pattern admits.</summary>
    public const long MaximumCatalogEpoch = 9_999_999_999L;

    public static readonly TimeSpan DefaultLifetime = TimeSpan.FromMinutes(5);
    public static readonly TimeSpan StaleLifetime = TimeSpan.FromMinutes(1);

    private readonly IModelProfileRegistry _registry;
    private readonly IModelCatalogDeploymentSource _deployments;
    private readonly IModelCatalogPolicySource _policy;
    private readonly IModelEntitlementSource _entitlement;
    private readonly TimeProvider _timeProvider;
    private readonly string _region;
    private readonly bool _untrustedRegistryPermitted;
    private readonly TimeSpan _lifetime;
    private long _highestPolicyVersion;

    public EffectiveModelCatalogService(
        IModelProfileRegistry registry,
        IModelCatalogDeploymentSource deployments,
        IModelCatalogPolicySource policy,
        IModelEntitlementSource entitlement,
        TimeProvider timeProvider,
        string region,
        bool untrustedRegistryPermitted)
        : this(
            registry,
            deployments,
            policy,
            entitlement,
            timeProvider,
            region,
            untrustedRegistryPermitted,
            DefaultLifetime)
    {
    }

    public EffectiveModelCatalogService(
        IModelProfileRegistry registry,
        IModelCatalogDeploymentSource deployments,
        IModelCatalogPolicySource policy,
        IModelEntitlementSource entitlement,
        TimeProvider timeProvider,
        string region,
        bool untrustedRegistryPermitted,
        TimeSpan lifetime)
    {
        ArgumentNullException.ThrowIfNull(registry);
        ArgumentNullException.ThrowIfNull(deployments);
        ArgumentNullException.ThrowIfNull(policy);
        ArgumentNullException.ThrowIfNull(entitlement);
        ArgumentNullException.ThrowIfNull(timeProvider);
        ArgumentException.ThrowIfNullOrWhiteSpace(region);
        if (lifetime <= TimeSpan.Zero || lifetime > TimeSpan.FromHours(1))
        {
            throw new ArgumentOutOfRangeException(nameof(lifetime));
        }
        _registry = registry;
        _deployments = deployments;
        _policy = policy;
        _entitlement = entitlement;
        _timeProvider = timeProvider;
        _region = region;
        _untrustedRegistryPermitted = untrustedRegistryPermitted;
        _lifetime = lifetime;
    }

    /// <summary>
    /// Builds the catalog for one subject. The last-valid-selection hint is
    /// advisory: it is honoured only when the profile it names is still
    /// selectable, and it can never resurrect a blocked one.
    /// </summary>
    public async ValueTask<ModelCatalogResult> BuildAsync(
        string subjectPartition,
        string? lastValidSelectionHint,
        CancellationToken cancellationToken)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(subjectPartition);

        ModelProfileRegistrySnapshot registry;
        PolicySnapshot policy;
        ModelDeploymentRead deployments;
        ModelEntitlementSnapshot entitlement;
        try
        {
            registry = await _registry.GetSnapshotAsync(cancellationToken).ConfigureAwait(false);
            policy = await _policy.GetSnapshotAsync(cancellationToken).ConfigureAwait(false);
            deployments = await _deployments.GetAsync(cancellationToken).ConfigureAwait(false);
            entitlement = await _entitlement
                .GetAsync(subjectPartition, cancellationToken)
                .ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception)
        {
            // A dependency that cannot answer produces no catalog at all. A
            // partial catalog would be indistinguishable from a smaller
            // organization.
            return new ModelCatalogResult(null, ModelCatalogErrorCodes.Unavailable);
        }

        // An untrusted fixture registry is structurally unreachable outside
        // Development: the constructor of the fixture registry refuses the
        // environment, and this is the second, independent refusal.
        if (registry.Trust != ModelProfileTrust.Signed && !_untrustedRegistryPermitted)
        {
            return new ModelCatalogResult(null, ModelCatalogErrorCodes.Unavailable);
        }
        if (!registry.IsUsable)
        {
            return new ModelCatalogResult(null, ModelCatalogErrorCodes.Unavailable);
        }

        // A policy downgrade is refused here as well as in the policy provider,
        // because the catalog binds the policy version into its own epoch and a
        // rollback would reuse an epoch a client already trusted.
        long observed = Interlocked.Read(ref _highestPolicyVersion);
        if (policy.PolicyVersion < observed)
        {
            return new ModelCatalogResult(null, ModelCatalogErrorCodes.Unavailable);
        }
        Interlocked.Exchange(ref _highestPolicyVersion, Math.Max(observed, policy.PolicyVersion));

        if (deployments.Snapshot is null
            || deployments.State is not (FoundryCatalogState.Fresh or FoundryCatalogState.StaleUsable))
        {
            return new ModelCatalogResult(
                null,
                deployments.HasPreviousSnapshot
                    ? ModelCatalogErrorCodes.Stale
                    : ModelCatalogErrorCodes.Unavailable);
        }

        bool stale = deployments.State == FoundryCatalogState.StaleUsable;
        Dictionary<string, FoundryDeployment> byName = new(StringComparer.Ordinal);
        foreach (FoundryDeployment deployment in deployments.Snapshot.Deployments)
        {
            byName[deployment.DeploymentName] = deployment;
        }

        List<EffectiveModelEntry> entries = [];
        List<UnavailableModelProfile> unavailable = [];
        List<ModelProfile> selectable = [];

        foreach (ModelProfile profile in ModelDefaultSelector.Order(registry.Profiles))
        {
            BlockedAvailability? blocked = Classify(
                profile,
                policy,
                entitlement,
                byName,
                stale,
                out FoundryDeployment? deployment);
            if (blocked is not null && profile.Visible)
            {
                unavailable.Add(new UnavailableModelProfile(
                    profile.ProfileId,
                    profile.DisplayName,
                    ModelCatalogWireNames.ToWireName(blocked.Value),
                    ModelCatalogWireNames.ToWireName(
                        ModelCatalogWireNames.RecoveryFor(blocked.Value))));
            }
            if (deployment is null)
            {
                // Nothing to join to. Every other blocked reason is a property
                // of the profile and needs no entry; only an entitlement gap
                // leaves a fully resolved deployment behind, and that one is
                // retained so route resolution can say "not entitled" instead
                // of the weaker "unavailable".
                continue;
            }

            ProjectedCapabilities capabilities = Narrow(
                profile.RequiredCapabilities,
                deployment!.Capabilities);
            bool narrowed = IsNarrowed(profile.RequiredCapabilities, deployment.Capabilities);
            SelectableAvailability availability = narrowed || stale
                ? SelectableAvailability.Degraded
                : SelectableAvailability.Available;
            string? statusDetail = narrowed
                ? "Some qualified capabilities are not currently advertised for this profile."
                : stale
                    ? "The deployment inventory behind this catalog is not fresh."
                    : null;

            entries.Add(new EffectiveModelEntry(
                profile,
                deployment,
                capabilities,
                Project(profile, capabilities),
                availability,
                statusDetail,
                Selectable: blocked is null,
                blocked));
            if (profile.Visible && blocked is null)
            {
                selectable.Add(profile);
            }
        }

        // Nothing selectable and nothing visible to explain why. An empty
        // selectable list with populated unavailableProfiles is a valid
        // catalog, not a problem: it is exactly how the desktop explains a
        // disappearance.
        if (unavailable.Count == 0 && !entries.Any(static entry => entry.Selectable))
        {
            return new ModelCatalogResult(null, ModelCatalogErrorCodes.NoEntitledProfiles);
        }

        DateTimeOffset generatedAt = _timeProvider.GetUtcNow();
        TimeSpan lifetime = stale && StaleLifetime < _lifetime ? StaleLifetime : _lifetime;
        string catalogState = stale
            ? ModelCatalogWireNames.CatalogStateStaleUsable
            : ModelCatalogWireNames.CatalogStateFresh;
        (SignedDesktopPolicy unsignedPolicy, _) = CanonicalPolicyProjector.Project(policy);

        string catalogHash = ComputeCatalogHash(
            catalogState,
            deployments.Snapshot.SnapshotHash,
            entitlement.EntitlementClass,
            unsignedPolicy.PolicyHash,
            policy.PolicyVersion,
            ComputeQualificationHash(entries),
            _region,
            registry.RegistryHash,
            registry.RegistryVersion);

        List<ModelProfileProjection> projections = [];
        foreach (EffectiveModelEntry entry in entries)
        {
            if (!entry.Profile.Visible || !entry.Selectable)
            {
                // Hidden and evaluation profiles stay resolvable server side
                // and are omitted from normal listing. A blocked entry is
                // already carried by unavailableProfiles and must never appear
                // in the selectable list.
                continue;
            }
            projections.Add(new ModelProfileProjection(
                entry.Profile.ProfileId,
                entry.Profile.DisplayName,
                entry.Profile.Description,
                entry.Profile.Family,
                ModelCatalogWireNames.Provider,
                ModelCatalogWireNames.ToWireName(entry.Availability),
                entry.StatusDetail,
                entry.Capabilities,
                entry.Limits,
                [
                    .. entry.Profile.Variants.Select(static variant =>
                        new ModelVariantProjection(
                            variant.VariantId,
                            variant.DisplayName,
                            variant.ReasoningEffort,
                            variant.MaxOutputTokens)),
                ],
                entry.Profile.DefaultVariant.VariantId,
                entry.Profile.Qualification.QualifiedAt));
        }

        ModelCatalogProjection projection = new(
            ModelCatalogWireNames.SchemaVersion,
            FormatCatalogVersion(registry.RegistryVersion, catalogHash),
            catalogHash,
            catalogState,
            policy.PolicyVersion,
            unsignedPolicy.PolicyHash,
            deployments.Snapshot.SnapshotHash,
            generatedAt,
            generatedAt + lifetime,
            ModelDefaultSelector.Select(
                selectable,
                entitlement.OrganizationDefaultProfileId,
                lastValidSelectionHint),
            projections,
            unavailable);

        return new ModelCatalogResult(
            new EffectiveModelCatalog(
                projection,
                entries,
                registry.RegistryVersion,
                registry.RegistryHash,
                registry.CapabilityHash,
                entitlement.EntitlementClass,
                _region),
            "");
    }

    /// <summary>
    /// Decides whether a profile is blocked, and joins it to its deployment
    /// when it is not. Order matters: the organization's own withdrawal is
    /// reported before an entitlement gap, so an administrator is not told to
    /// grant a licence for a profile nobody can use.
    /// </summary>
    private BlockedAvailability? Classify(
        ModelProfile profile,
        PolicySnapshot policy,
        ModelEntitlementSnapshot entitlement,
        IReadOnlyDictionary<string, FoundryDeployment> byName,
        bool stale,
        out FoundryDeployment? deployment)
    {
        deployment = null;
        if (!profile.Enabled)
        {
            return BlockedAvailability.PolicyBlocked;
        }
        if (!profile.AllowedRegions.Contains(_region, StringComparer.Ordinal)
            || !policy.AllowedRegions.Contains(_region, StringComparer.Ordinal))
        {
            return BlockedAvailability.PolicyBlocked;
        }
        // The tenant policy may publish a deployment allowlist. When it does it
        // is an additional narrowing on top of the signed registry; when it is
        // empty the signed registry is the only authority, which is the same
        // trust boundary the profile set already sits behind.
        if (policy.ApprovedModelDeployments.Length > 0
            && !policy.ApprovedModelDeployments.Contains(
                profile.DeploymentName,
                StringComparer.Ordinal))
        {
            return BlockedAvailability.PolicyBlocked;
        }
        if (profile.Qualification.Status != ModelQualificationStatus.Qualified)
        {
            return BlockedAvailability.Unqualified;
        }
        if (!byName.TryGetValue(profile.DeploymentName, out FoundryDeployment? candidate))
        {
            // A stale snapshot is not authority for absence: it may simply
            // predate the deployment. The user is told to refresh rather than
            // to pick something else.
            return stale
                ? BlockedAvailability.CatalogStale
                : BlockedAvailability.DeploymentMissing;
        }
        if (!string.Equals(
                candidate.ModelPublisher,
                profile.ExpectedModel.Publisher,
                StringComparison.Ordinal)
            || !string.Equals(
                candidate.ModelName,
                profile.ExpectedModel.Name,
                StringComparison.Ordinal)
            || !string.Equals(
                candidate.ModelVersion,
                profile.ExpectedModel.Version,
                StringComparison.Ordinal))
        {
            // The name still exists but now serves a different model. That is
            // not a narrowing, it is a different model, and it fails closed.
            return BlockedAvailability.DeploymentMissing;
        }
        // The deployment is published to the caller before the entitlement
        // check so an entitlement gap keeps a fully joined entry. That entry is
        // never selectable and never projected; it exists only so route
        // resolution can name the gap precisely.
        deployment = candidate;
        return entitlement.EntitledProfileIds.Contains(profile.ProfileId)
            ? null
            : BlockedAvailability.NotEntitled;
    }

    /// <summary>
    /// The boolean AND of what the organization qualified with what the
    /// provider currently advertises. Narrowing only.
    /// </summary>
    internal static ProjectedCapabilities Narrow(
        ModelProfileCapabilities required,
        FoundryDeploymentCapabilities advertised) => new(
            required.TextInput && advertised.TextInput,
            required.ImageInput && advertised.ImageInput,
            required.PdfInput && advertised.PdfInput,
            required.Attachments && advertised.Attachments,
            required.ToolCalling && advertised.ToolCalling,
            required.StructuredOutput && advertised.StructuredOutput,
            required.Reasoning && advertised.Reasoning);

    internal static bool IsNarrowed(
        ModelProfileCapabilities required,
        FoundryDeploymentCapabilities advertised) =>
        (required.TextInput && !advertised.TextInput)
        || (required.ImageInput && !advertised.ImageInput)
        || (required.PdfInput && !advertised.PdfInput)
        || (required.Attachments && !advertised.Attachments)
        || (required.ToolCalling && !advertised.ToolCalling)
        || (required.StructuredOutput && !advertised.StructuredOutput)
        || (required.Reasoning && !advertised.Reasoning)
        || (required.Streaming && !advertised.Streaming);

    /// <summary>
    /// Limits Forge will actually enforce. The output budget is the default
    /// variant's, not the profile ceiling, because the ceiling is the maximum
    /// any variant may reach rather than what an unqualified request gets. The
    /// attachment budget is null unless attachments survived narrowing, so the
    /// desktop renders it as unknown rather than as a zero-byte allowance.
    /// </summary>
    internal static ProjectedLimits Project(
        ModelProfile profile,
        ProjectedCapabilities capabilities) => new(
            profile.Limits.ContextTokens,
            profile.Limits.MaxInputTokens,
            profile.DefaultVariant.MaxOutputTokens,
            capabilities.Attachments ? profile.Limits.MaxAttachmentBytes : null);

    /// <summary>
    /// The canonical catalog version: a monotonic epoch taken from the signed
    /// registry version, then a short digest of every authority-relevant input.
    /// A rollback of the registry version fails closed before this is ever
    /// reached, so the epoch never repeats with different content.
    /// </summary>
    internal static string FormatCatalogVersion(long registryVersion, string catalogHash)
    {
        long epoch = Math.Clamp(registryVersion, 1, MaximumCatalogEpoch);
        return $"v{epoch}.{catalogHash.AsSpan("sha256:".Length, 16)}";
    }

    internal static string ComputeCatalogHash(
        string catalogState,
        string deploymentSnapshotHash,
        string entitlementClass,
        string policyHash,
        long policyVersion,
        string qualificationEvidenceHash,
        string region,
        string registryHash,
        long registryVersion) =>
        ModelProfileValidator.Digest(
            CatalogHashPurpose,
            new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["catalogState"] = catalogState,
                ["deploymentSnapshotHash"] = deploymentSnapshotHash,
                ["entitlementClass"] = entitlementClass,
                ["policyHash"] = policyHash,
                ["policyVersion"] = policyVersion,
                ["qualificationEvidenceHash"] = qualificationEvidenceHash,
                ["region"] = region,
                ["registryHash"] = registryHash,
                ["registryVersion"] = registryVersion,
                ["schemaVersion"] = ModelCatalogWireNames.SchemaVersion,
            });

    internal static string ComputeQualificationHash(IReadOnlyList<EffectiveModelEntry> entries)
    {
        List<object> ordered = [];
        foreach (EffectiveModelEntry entry in entries
            .OrderBy(static entry => entry.Profile.ProfileId, StringComparer.Ordinal))
        {
            ordered.Add(new SortedDictionary<string, object?>(StringComparer.Ordinal)
            {
                ["deploymentHash"] = entry.Deployment.DeploymentHash,
                ["evidenceHash"] = entry.Profile.Qualification.EvidenceHash,
                ["profileId"] = entry.Profile.ProfileId,
                ["status"] = ModelProfileValidator.ToWireName(entry.Profile.Qualification.Status),
            });
        }
        return ModelProfileValidator.Digest(QualificationHashPurpose, ordered);
    }
}

/// <summary>
/// A Development-only entitlement source. Every profile the registry carries is
/// entitled, under one fixed class name. It refuses any environment but
/// Development and Test.
/// </summary>
public sealed class DevelopmentModelEntitlementSource : IModelEntitlementSource
{
    public const string EnvironmentForbiddenCode =
        "development_model_entitlement_environment_forbidden";

    public const string EntitlementClassName = "development-all";

    private readonly IModelProfileRegistry _registry;

    public DevelopmentModelEntitlementSource(
        string environmentName,
        IModelProfileRegistry registry)
    {
        ArgumentNullException.ThrowIfNull(registry);
        if (!DevelopmentFixtureModelProfileRegistry.IsPermittedEnvironment(environmentName))
        {
            throw new InvalidOperationException(EnvironmentForbiddenCode);
        }
        _registry = registry;
    }

    public async ValueTask<ModelEntitlementSnapshot> GetAsync(
        string subjectPartition,
        CancellationToken cancellationToken)
    {
        ModelProfileRegistrySnapshot snapshot = await _registry
            .GetSnapshotAsync(cancellationToken)
            .ConfigureAwait(false);
        return new ModelEntitlementSnapshot(
            EntitlementClassName,
            new HashSet<string>(
                snapshot.Profiles.Select(static profile => profile.ProfileId),
                StringComparer.Ordinal),
            OrganizationDefaultProfileId: null);
    }
}

/// <summary>
/// The production entitlement source until a directory-backed one exists: every
/// profile the signed registry carries is entitled, under one class name that
/// is bound into the catalog digest.
///
/// It is deliberately not a permissive stub in disguise — the signed registry
/// is itself the organization's authority over which profiles exist — but it
/// carries no per-subject differentiation, which is recorded rather than
/// hidden.
/// </summary>
public sealed class RegistryModelEntitlementSource(IModelProfileRegistry registry)
    : IModelEntitlementSource
{
    public const string EntitlementClassName = "organization-registry";

    public async ValueTask<ModelEntitlementSnapshot> GetAsync(
        string subjectPartition,
        CancellationToken cancellationToken)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(subjectPartition);
        ModelProfileRegistrySnapshot snapshot = await registry
            .GetSnapshotAsync(cancellationToken)
            .ConfigureAwait(false);
        return new ModelEntitlementSnapshot(
            EntitlementClassName,
            new HashSet<string>(
                snapshot.Profiles.Select(static profile => profile.ProfileId),
                StringComparer.Ordinal),
            OrganizationDefaultProfileId: null);
    }
}
