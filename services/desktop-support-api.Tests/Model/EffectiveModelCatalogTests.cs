using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-025. The effective model catalog.
/// </summary>
public sealed class EffectiveModelCatalogTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private const string CatalogVersionPattern = @"^v[1-9][0-9]{0,9}\.[0-9a-f]{16}$";

    // -------------------------------------------------------------- the join

    [Fact]
    public async Task Enabled_profiles_join_to_discovered_deployments_by_exact_name()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        Assert.Equal(2, catalog.Projection.Profiles.Count);
        Assert.Equal(
            ModelCatalogTestFixtures.AlphaDeploymentName,
            catalog.Find(ModelCatalogTestFixtures.AlphaProfileId)!.Deployment.DeploymentName);
    }

    [Fact]
    public async Task A_profile_whose_deployment_no_longer_exists_is_blocked_not_listed()
    {
        EffectiveModelCatalog catalog = await BuildAsync(snapshot: ModelCatalogTestFixtures.Snapshot(
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.AlphaDeploymentName,
                ModelCatalogTestFixtures.AlphaDeploymentHash)));

        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            Assert.Single(catalog.Projection.Profiles).ProfileId);
        UnavailableModelProfile blocked = Assert.Single(catalog.Projection.UnavailableProfiles);
        Assert.Equal(ModelCatalogTestFixtures.BetaProfileId, blocked.ProfileId);
        Assert.Equal("deployment_missing", blocked.Availability);
        Assert.Equal("select_another_profile", blocked.RecoveryAction);
    }

    [Fact]
    public async Task A_deployment_that_now_serves_a_different_model_fails_closed()
    {
        EffectiveModelCatalog catalog = await BuildAsync(snapshot: ModelCatalogTestFixtures.Snapshot(
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.AlphaDeploymentName,
                ModelCatalogTestFixtures.AlphaDeploymentHash,
                modelVersion: "9999-99-99"),
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.BetaDeploymentName,
                ModelCatalogTestFixtures.BetaDeploymentHash)));

        Assert.Equal(
            ModelCatalogTestFixtures.BetaProfileId,
            Assert.Single(catalog.Projection.Profiles).ProfileId);
        Assert.Equal(
            "deployment_missing",
            Assert.Single(catalog.Projection.UnavailableProfiles).Availability);
    }

    // ------------------------------------------------------------ narrowing

    [Fact]
    public async Task A_deployment_may_narrow_a_capability_but_never_broaden_one()
    {
        EffectiveModelCatalog catalog = await BuildAsync(snapshot: ModelCatalogTestFixtures.Snapshot(
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.AlphaDeploymentName,
                ModelCatalogTestFixtures.AlphaDeploymentHash,
                // The provider advertises an image capability the organization
                // never qualified and withdraws the reasoning capability it did.
                new FoundryDeploymentCapabilities(
                    TextInput: true,
                    ImageInput: true,
                    PdfInput: false,
                    Attachments: true,
                    ToolCalling: true,
                    StructuredOutput: true,
                    Reasoning: false,
                    Streaming: true)),
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.BetaDeploymentName,
                ModelCatalogTestFixtures.BetaDeploymentHash)));

        ModelProfileProjection alpha = catalog.Projection.Profiles.Single(profile =>
            profile.ProfileId == ModelCatalogTestFixtures.AlphaProfileId);
        Assert.False(alpha.Capabilities.ImageInput);
        Assert.False(alpha.Capabilities.Attachments);
        Assert.False(alpha.Capabilities.Reasoning);
        Assert.Equal("degraded", alpha.Availability);
        Assert.NotNull(alpha.StatusDetail);
    }

    [Fact]
    public async Task The_projected_output_budget_is_the_default_variant_not_the_profile_ceiling()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        ModelProfileProjection alpha = catalog.Projection.Profiles.Single(profile =>
            profile.ProfileId == ModelCatalogTestFixtures.AlphaProfileId);
        Assert.Equal(16_000, alpha.Limits.MaxOutputTokens);
        Assert.Equal(256_000, alpha.Limits.ContextTokens);
        Assert.Equal("balanced", alpha.DefaultVariantId);
    }

    [Fact]
    public async Task An_unpublished_attachment_budget_is_projected_as_unknown()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        Assert.All(
            catalog.Projection.Profiles,
            profile => Assert.Null(profile.Limits.MaxAttachmentBytes));
    }

    // ------------------------------------------------------ version and hash

    [Fact]
    public async Task The_catalog_version_matches_the_canonical_pattern()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        Assert.Matches(CatalogVersionPattern, catalog.Projection.CatalogVersion);
        Assert.StartsWith("v7.", catalog.Projection.CatalogVersion, StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_changed_deployment_snapshot_produces_a_new_catalog_version()
    {
        EffectiveModelCatalog first = await BuildAsync();
        EffectiveModelCatalog second = await BuildAsync(snapshot: new FoundryDeploymentSnapshot(
            FoundryDeploymentSnapshot.SchemaVersionValue,
            "sha256:dd11ee22ff3300445566778899001122334455667788990011223344556677bb",
            DateTimeOffset.UnixEpoch,
            [
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.AlphaDeploymentHash),
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.BetaDeploymentName,
                    ModelCatalogTestFixtures.BetaDeploymentHash),
            ]));

        Assert.NotEqual(first.Projection.CatalogHash, second.Projection.CatalogHash);
        Assert.NotEqual(first.Projection.CatalogVersion, second.Projection.CatalogVersion);
    }

    [Fact]
    public async Task A_changed_entitlement_class_produces_a_new_catalog_version()
    {
        EffectiveModelCatalog first = await BuildAsync();
        EffectiveModelCatalog second = await BuildAsync(entitlementClass: "another-class");

        Assert.NotEqual(first.Projection.CatalogHash, second.Projection.CatalogHash);
    }

    [Fact]
    public async Task An_unchanged_input_set_produces_an_identical_catalog_hash()
    {
        EffectiveModelCatalog first = await BuildAsync();
        EffectiveModelCatalog second = await BuildAsync();

        Assert.Equal(first.Projection.CatalogHash, second.Projection.CatalogHash);
    }

    [Fact]
    public async Task The_catalog_carries_a_bounded_expiry()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        Assert.True(catalog.Projection.ExpiresAt > catalog.Projection.GeneratedAt);
        Assert.Equal(
            EffectiveModelCatalogService.DefaultLifetime,
            catalog.Projection.ExpiresAt - catalog.Projection.GeneratedAt);
    }

    // ------------------------------------------------------ default selection

    [Fact]
    public async Task The_explicit_organization_default_wins()
    {
        EffectiveModelCatalog catalog = await BuildAsync(
            organizationDefault: ModelCatalogTestFixtures.BetaProfileId);

        Assert.Equal(
            ModelCatalogTestFixtures.BetaProfileId,
            catalog.Projection.DefaultProfileId);
    }

    [Fact]
    public async Task The_last_valid_user_selection_wins_over_deterministic_order()
    {
        EffectiveModelCatalog catalog = await BuildAsync(
            hint: ModelCatalogTestFixtures.BetaProfileId);

        Assert.Equal(
            ModelCatalogTestFixtures.BetaProfileId,
            catalog.Projection.DefaultProfileId);
    }

    [Fact]
    public async Task Deterministic_priority_then_identifier_order_is_the_final_default()
    {
        EffectiveModelCatalog catalog = await BuildAsync();

        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            catalog.Projection.DefaultProfileId);
    }

    [Fact]
    public async Task A_blocked_profile_is_never_selected_as_the_default()
    {
        // Both the organization default and the user's last selection name a
        // profile whose deployment has gone. Neither may resurrect it.
        EffectiveModelCatalog catalog = await BuildAsync(
            snapshot: ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.AlphaDeploymentHash)),
            organizationDefault: ModelCatalogTestFixtures.BetaProfileId,
            hint: ModelCatalogTestFixtures.BetaProfileId);

        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            catalog.Projection.DefaultProfileId);
    }

    [Fact]
    public void No_selectable_profile_means_no_default_rather_than_a_substitute()
    {
        Assert.Null(ModelDefaultSelector.Select([], "anything", "anything-else"));
    }

    // ----------------------------------------------------- blocked and hidden

    [Fact]
    public async Task An_unentitled_visible_profile_states_only_its_prerequisite()
    {
        EffectiveModelCatalog catalog = await BuildAsync(
            entitled: [ModelCatalogTestFixtures.AlphaProfileId]);

        UnavailableModelProfile blocked = Assert.Single(catalog.Projection.UnavailableProfiles);
        Assert.Equal(ModelCatalogTestFixtures.BetaProfileId, blocked.ProfileId);
        Assert.Equal("not_entitled", blocked.Availability);
        Assert.Equal("request_entitlement", blocked.RecoveryAction);
    }

    [Fact]
    public async Task A_hidden_profile_is_omitted_from_both_lists_but_stays_resolvable()
    {
        ModelProfile hidden = ModelCatalogTestFixtures.ParseBeta() with { Visible = false };
        EffectiveModelCatalog catalog = await BuildAsync(
            profiles: [ModelCatalogTestFixtures.ParseAlpha(), hidden]);

        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            Assert.Single(catalog.Projection.Profiles).ProfileId);
        Assert.Empty(catalog.Projection.UnavailableProfiles);
        Assert.NotNull(catalog.Find(ModelCatalogTestFixtures.BetaProfileId));
    }

    [Fact]
    public async Task A_disabled_visible_profile_is_reported_as_policy_blocked()
    {
        ModelProfile disabled = ModelCatalogTestFixtures.ParseBeta() with { Enabled = false };
        EffectiveModelCatalog catalog = await BuildAsync(
            profiles: [ModelCatalogTestFixtures.ParseAlpha(), disabled]);

        UnavailableModelProfile blocked = Assert.Single(catalog.Projection.UnavailableProfiles);
        Assert.Equal("policy_blocked", blocked.Availability);
        Assert.Equal("contact_administrator", blocked.RecoveryAction);
    }

    [Fact]
    public async Task An_unqualified_profile_is_blocked_awaiting_qualification()
    {
        ModelProfile expired = ModelCatalogTestFixtures.ParseBeta() with
        {
            Qualification = ModelCatalogTestFixtures.ParseBeta().Qualification with
            {
                Status = ModelQualificationStatus.Expired,
            },
        };
        EffectiveModelCatalog catalog = await BuildAsync(
            profiles: [ModelCatalogTestFixtures.ParseAlpha(), expired]);

        UnavailableModelProfile blocked = Assert.Single(catalog.Projection.UnavailableProfiles);
        Assert.Equal("unqualified", blocked.Availability);
        Assert.Equal("awaiting_qualification", blocked.RecoveryAction);
    }

    [Fact]
    public async Task A_profile_outside_the_current_region_is_policy_blocked()
    {
        ModelProfile elsewhere = ModelCatalogTestFixtures.ParseBeta() with
        {
            AllowedRegions = ["otherregion"],
        };
        EffectiveModelCatalog catalog = await BuildAsync(
            profiles: [ModelCatalogTestFixtures.ParseAlpha(), elsewhere]);

        Assert.Equal(
            "policy_blocked",
            Assert.Single(catalog.Projection.UnavailableProfiles).Availability);
    }

    [Fact]
    public async Task A_deployment_outside_a_published_policy_allowlist_is_policy_blocked()
    {
        EffectiveModelCatalog catalog = await BuildAsync(
            policy: ModelCatalogTestFixtures.Policy(
                approvedDeployments: [ModelCatalogTestFixtures.AlphaDeploymentName]));

        Assert.Equal(
            ModelCatalogTestFixtures.AlphaProfileId,
            Assert.Single(catalog.Projection.Profiles).ProfileId);
        Assert.Equal(
            "policy_blocked",
            Assert.Single(catalog.Projection.UnavailableProfiles).Availability);
    }

    // ------------------------------------------------------------ staleness

    [Fact]
    public async Task A_stale_but_usable_snapshot_degrades_rather_than_disappearing()
    {
        EffectiveModelCatalog catalog = await BuildAsync(state: FoundryCatalogState.StaleUsable);

        Assert.Equal("stale_usable", catalog.Projection.CatalogState);
        Assert.All(
            catalog.Projection.Profiles,
            profile => Assert.Equal("degraded", profile.Availability));
        Assert.Equal(
            EffectiveModelCatalogService.StaleLifetime,
            catalog.Projection.ExpiresAt - catalog.Projection.GeneratedAt);
    }

    [Fact]
    public async Task A_stale_snapshot_is_not_authority_for_a_missing_deployment()
    {
        ModelCatalogResult result = await RunAsync(
            snapshot: ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.AlphaDeploymentHash)),
            state: FoundryCatalogState.StaleUsable);

        UnavailableModelProfile blocked =
            Assert.Single(result.Catalog!.Projection.UnavailableProfiles);
        Assert.Equal("catalog_stale", blocked.Availability);
        Assert.Equal("refresh_catalog", blocked.RecoveryAction);
    }

    [Fact]
    public async Task An_unreachable_inventory_produces_no_catalog_at_all()
    {
        ModelCatalogResult result = await RunAsync(state: FoundryCatalogState.Unavailable);

        Assert.False(result.IsSuccess);
        Assert.Equal(ModelCatalogErrorCodes.Unavailable, result.ErrorCode);
    }

    [Fact]
    public async Task An_inventory_aged_past_its_budget_is_reported_as_stale()
    {
        ModelCatalogResult result = await RunAsync(
            state: FoundryCatalogState.Unavailable,
            hasPreviousSnapshot: true);

        Assert.Equal(ModelCatalogErrorCodes.Stale, result.ErrorCode);
    }

    // ------------------------------------------------------------ fail closed

    [Fact]
    public async Task An_untrusted_registry_is_refused_when_untrusted_profiles_are_not_permitted()
    {
        ModelCatalogResult result = await RunAsync(trust: ModelProfileTrust.DevelopmentFixtureUntrusted);

        Assert.False(result.IsSuccess);
        Assert.Equal(ModelCatalogErrorCodes.Unavailable, result.ErrorCode);
    }

    [Fact]
    public async Task An_untrusted_registry_is_permitted_only_where_it_is_explicitly_allowed()
    {
        ModelCatalogResult result = await RunAsync(
            trust: ModelProfileTrust.DevelopmentFixtureUntrusted,
            untrustedPermitted: true);

        Assert.True(result.IsSuccess);
    }

    [Fact]
    public async Task A_policy_version_rollback_fails_closed()
    {
        StubPolicySource policy = new(ModelCatalogTestFixtures.Policy(policyVersion: 9));
        EffectiveModelCatalogService service = Service(policy: policy);
        _ = await service.BuildAsync("subject", null, Ct);

        policy.Snapshot = ModelCatalogTestFixtures.Policy(policyVersion: 8);
        ModelCatalogResult result = await service.BuildAsync("subject", null, Ct);

        Assert.False(result.IsSuccess);
        Assert.Equal(ModelCatalogErrorCodes.Unavailable, result.ErrorCode);
    }

    [Fact]
    public async Task Nothing_selectable_and_nothing_visible_is_its_own_problem()
    {
        ModelProfile hidden = ModelCatalogTestFixtures.ParseAlpha() with { Visible = false };
        ModelCatalogResult result = await RunAsync(
            profiles: [hidden],
            snapshot: ModelCatalogTestFixtures.Snapshot());

        Assert.False(result.IsSuccess);
        Assert.Equal(ModelCatalogErrorCodes.NoEntitledProfiles, result.ErrorCode);
    }

    [Fact]
    public async Task Nothing_selectable_but_something_visible_is_still_a_catalog()
    {
        ModelCatalogResult result = await RunAsync(entitled: []);

        Assert.True(result.IsSuccess);
        Assert.Empty(result.Catalog!.Projection.Profiles);
        Assert.Null(result.Catalog.Projection.DefaultProfileId);
        Assert.Equal(2, result.Catalog.Projection.UnavailableProfiles.Count);
    }

    // ----------------------------------------------------------------- helpers

    private static async Task<EffectiveModelCatalog> BuildAsync(
        IReadOnlyList<ModelProfile>? profiles = null,
        FoundryDeploymentSnapshot? snapshot = null,
        PolicySnapshot? policy = null,
        FoundryCatalogState state = FoundryCatalogState.Fresh,
        string? organizationDefault = null,
        string? hint = null,
        string entitlementClass = "synthetic-class",
        IReadOnlyList<string>? entitled = null)
    {
        ModelCatalogResult result = await RunAsync(
            profiles,
            snapshot,
            policy,
            state,
            organizationDefault,
            hint,
            entitlementClass,
            entitled);
        Assert.True(result.IsSuccess, result.ErrorCode);
        return result.Catalog!;
    }

    private static async Task<ModelCatalogResult> RunAsync(
        IReadOnlyList<ModelProfile>? profiles = null,
        FoundryDeploymentSnapshot? snapshot = null,
        PolicySnapshot? policy = null,
        FoundryCatalogState state = FoundryCatalogState.Fresh,
        string? organizationDefault = null,
        string? hint = null,
        string entitlementClass = "synthetic-class",
        IReadOnlyList<string>? entitled = null,
        ModelProfileTrust trust = ModelProfileTrust.Signed,
        bool untrustedPermitted = false,
        bool hasPreviousSnapshot = false)
    {
        IReadOnlyList<ModelProfile> resolved = profiles
            ?? [ModelCatalogTestFixtures.ParseAlpha(), ModelCatalogTestFixtures.ParseBeta()];
        EffectiveModelCatalogService service = Service(
            resolved,
            snapshot,
            policy is null ? null : new StubPolicySource(policy),
            state,
            organizationDefault,
            entitlementClass,
            entitled ?? [.. resolved.Select(static profile => profile.ProfileId)],
            trust,
            untrustedPermitted,
            hasPreviousSnapshot);
        return await service.BuildAsync("synthetic-subject", hint, Ct);
    }

    private static EffectiveModelCatalogService Service(
        IReadOnlyList<ModelProfile>? profiles = null,
        FoundryDeploymentSnapshot? snapshot = null,
        StubPolicySource? policy = null,
        FoundryCatalogState state = FoundryCatalogState.Fresh,
        string? organizationDefault = null,
        string entitlementClass = "synthetic-class",
        IReadOnlyList<string>? entitled = null,
        ModelProfileTrust trust = ModelProfileTrust.Signed,
        bool untrustedPermitted = false,
        bool hasPreviousSnapshot = false)
    {
        IReadOnlyList<ModelProfile> resolved = profiles
            ?? [ModelCatalogTestFixtures.ParseAlpha(), ModelCatalogTestFixtures.ParseBeta()];
        FoundryDeploymentSnapshot inventory = snapshot ?? ModelCatalogTestFixtures.Snapshot(
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.AlphaDeploymentName,
                ModelCatalogTestFixtures.AlphaDeploymentHash),
            ModelCatalogTestFixtures.Deployment(
                ModelCatalogTestFixtures.BetaDeploymentName,
                ModelCatalogTestFixtures.BetaDeploymentHash));
        return new EffectiveModelCatalogService(
            new StubRegistry(resolved, trust),
            new StubDeploymentSource(
                state is FoundryCatalogState.Fresh or FoundryCatalogState.StaleUsable
                    ? inventory
                    : null,
                state,
                hasPreviousSnapshot),
            policy ?? new StubPolicySource(ModelCatalogTestFixtures.Policy()),
            new StubEntitlementSource(ModelCatalogTestFixtures.Entitlement(
                entitled ?? [.. resolved.Select(static profile => profile.ProfileId)],
                entitlementClass,
                organizationDefault)),
            new ModelCatalogTestFixtures.ControlledTimeProvider(DateTimeOffset.UnixEpoch),
            ModelCatalogTestFixtures.Region,
            untrustedPermitted);
    }

    private sealed class StubRegistry(IReadOnlyList<ModelProfile> profiles, ModelProfileTrust trust)
        : IModelProfileRegistry
    {
        public ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(new ModelProfileRegistrySnapshot(
                ModelCatalogTestFixtures.ParityRegistryVersion,
                ModelCatalogTestFixtures.ParityRegistryHash,
                ModelCatalogTestFixtures.ParityCapabilityHash,
                profiles,
                trust,
                DateTimeOffset.UnixEpoch));
    }

    private sealed class StubDeploymentSource(
        FoundryDeploymentSnapshot? snapshot,
        FoundryCatalogState state,
        bool hasPreviousSnapshot) : IModelCatalogDeploymentSource
    {
        public ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken) =>
            ValueTask.FromResult(new ModelDeploymentRead(state, snapshot, hasPreviousSnapshot));
    }

    private sealed class StubPolicySource(PolicySnapshot snapshot) : IModelCatalogPolicySource
    {
        public PolicySnapshot Snapshot { get; set; } = snapshot;

        public ValueTask<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken) =>
            ValueTask.FromResult(Snapshot);
    }

    private sealed class StubEntitlementSource(ModelEntitlementSnapshot snapshot)
        : IModelEntitlementSource
    {
        public ValueTask<ModelEntitlementSnapshot> GetAsync(
            string subjectPartition,
            CancellationToken cancellationToken) =>
            ValueTask.FromResult(snapshot);
    }
}
