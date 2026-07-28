using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-027. Route resolution.
/// </summary>
public sealed class ModelRouteResolverTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    // ------------------------------------------------------------- happy path

    [Fact]
    public async Task A_bound_request_resolves_to_the_exact_profile_deployment_and_variant()
    {
        Harness harness = await Harness.CreateAsync();

        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.Equal(ModelCatalogTestFixtures.AlphaProfileId, route.ProfileId);
        Assert.Equal(ModelCatalogTestFixtures.AlphaDeploymentName, route.DeploymentName);
        Assert.Equal(ModelCatalogTestFixtures.AlphaDeploymentHash, route.DeploymentHash);
        Assert.Equal("balanced", route.VariantId);
        Assert.Equal(16_000, route.MaxOutputTokens);
        Assert.Equal(256_000, route.ContextTokens);
        Assert.Equal("transient_no_store", route.RetentionMode);
        Assert.Equal(ModelCatalogTestFixtures.Region, route.Region);
        Assert.Equal(harness.Catalog.RegistryHash, route.RegistryHash);
        Assert.True(RequestGuards.IsSha256(route.RouteHash));
    }

    [Fact]
    public async Task A_null_variant_resolves_to_the_profile_default()
    {
        Harness harness = await Harness.CreateAsync();

        ResolvedModelRoute route = harness.Resolve(
            harness.Request(requestedVariantId: null, consentVariantId: null));

        Assert.Equal("balanced", route.VariantId);
        Assert.Equal("medium", route.ReasoningEffort);
    }

    [Fact]
    public async Task A_named_variant_binds_its_own_output_budget()
    {
        Harness harness = await Harness.CreateAsync();

        ResolvedModelRoute route = harness.Resolve(harness.Request(
            requestedVariantId: "deep",
            consentVariantId: "deep",
            requestedOutputTokens: 32_000));

        Assert.Equal("deep", route.VariantId);
        Assert.Equal(32_000, route.MaxOutputTokens);
        Assert.Equal("high", route.ReasoningEffort);
    }

    [Fact]
    public async Task Resolution_is_deterministic_so_a_retry_reuses_the_identical_route()
    {
        Harness harness = await Harness.CreateAsync();
        ModelRouteRequest request = harness.Request();

        ResolvedModelRoute first = harness.Resolve(request);
        ResolvedModelRoute second = harness.Resolve(request);

        Assert.Equal(first, second);
        Assert.Equal(first.RouteHash, second.RouteHash);
    }

    [Fact]
    public async Task The_route_hash_changes_when_the_variant_changes()
    {
        Harness harness = await Harness.CreateAsync();

        ResolvedModelRoute balanced = harness.Resolve(harness.Request());
        ResolvedModelRoute deep = harness.Resolve(harness.Request(
            requestedVariantId: "deep",
            consentVariantId: "deep",
            requestedOutputTokens: 32_000));

        Assert.NotEqual(balanced.RouteHash, deep.RouteHash);
    }

    [Fact]
    public async Task The_deployment_binding_hash_is_built_from_server_owned_data_only()
    {
        Harness harness = await Harness.CreateAsync();
        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.Equal(
            route.ComputeDeploymentBindingHash(),
            harness.Resolve(harness.Request()).ComputeDeploymentBindingHash());
        Assert.True(RequestGuards.IsSha256(route.ComputeDeploymentBindingHash()));
    }

    // ------------------------------------------------------------- refusals

    [Fact]
    public async Task A_request_naming_a_superseded_catalog_epoch_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.CatalogStale,
            harness.Refuse(harness.Request(catalogVersion: "v1.0123456789abcdef")));
    }

    [Fact]
    public async Task A_request_naming_a_profile_the_catalog_does_not_carry_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.ProfileUnavailable,
            harness.Refuse(harness.Request(profileId: "synthetic-absent")));
    }

    [Fact]
    public async Task A_request_for_an_unentitled_profile_is_refused()
    {
        Harness harness = await Harness.CreateAsync(
            entitled: [ModelCatalogTestFixtures.BetaProfileId]);

        Assert.Equal(
            ModelRouteErrorCodes.ProfileNotEntitled,
            harness.Refuse(harness.Request()));
    }

    [Fact]
    public async Task An_edited_profile_binding_hash_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.ProfileBindingMismatch,
            harness.Refuse(harness.Request(
                profileHash:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000001")));
    }

    [Fact]
    public async Task An_edited_capability_binding_hash_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.CapabilityMismatch,
            harness.Refuse(harness.Request(
                capabilityHash:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000002")));
    }

    [Fact]
    public async Task A_variant_from_another_profile_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.VariantInvalid,
            harness.Refuse(harness.Request(
                requestedVariantId: "elsewhere",
                consentVariantId: "elsewhere")));
    }

    [Theory]
    [InlineData("not_an_allowed_purpose", null, null)]
    [InlineData(null, "not_an_allowed_role", null)]
    [InlineData(null, null, "sapphirus.some-other-schema.v1")]
    public async Task A_purpose_role_or_output_schema_outside_the_allowlist_is_refused(
        string? purpose,
        string? modelRole,
        string? outputSchemaId)
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.CapabilityMismatch,
            harness.Refuse(harness.Request(
                purpose: purpose,
                modelRole: modelRole,
                canonicalOutputSchemaId: outputSchemaId)));
    }

    [Fact]
    public async Task A_streaming_request_that_also_binds_a_structured_schema_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.CapabilityMismatch,
            harness.Refuse(harness.Request(requestMode: "streaming_chat")));
    }

    [Fact]
    public async Task A_modality_the_profile_never_qualified_is_refused()
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.CapabilityMismatch,
            harness.Refuse(harness.Request(usesImageInput: true)));
    }

    [Fact]
    public async Task A_tool_definition_without_the_tool_capability_is_refused()
    {
        Harness harness = await Harness.CreateAsync(
            capabilities: new FoundryDeploymentCapabilities(
                TextInput: true,
                ImageInput: false,
                PdfInput: false,
                Attachments: false,
                ToolCalling: false,
                StructuredOutput: true,
                Reasoning: true,
                Streaming: true));

        Assert.Equal(
            ModelRouteErrorCodes.CapabilityMismatch,
            harness.Refuse(harness.Request(toolCount: 1)));
    }

    [Theory]
    [InlineData(300_000, 100)]
    [InlineData(100, 20_000)]
    [InlineData(250_000, 16_000)]
    public async Task A_request_beyond_the_context_or_output_budget_is_refused(
        int estimatedInputTokens,
        int requestedOutputTokens)
    {
        Harness harness = await Harness.CreateAsync();

        Assert.Equal(
            ModelRouteErrorCodes.ContextLimitExceeded,
            harness.Refuse(harness.Request(
                estimatedInputTokens: estimatedInputTokens,
                requestedOutputTokens: requestedOutputTokens)));
    }

    [Fact]
    public async Task A_consent_bound_to_a_different_profile_is_refused()
    {
        Harness harness = await Harness.CreateAsync();
        ModelRouteRequest request = harness.Request();
        ModelRouteRequest tampered = request with
        {
            Consent = request.Consent with
            {
                ModelProfileId = ModelCatalogTestFixtures.BetaProfileId,
            },
        };

        Assert.Equal(ModelRouteErrorCodes.ProfileBindingMismatch, harness.Refuse(tampered));
    }

    [Fact]
    public async Task A_consent_bound_to_a_different_region_is_refused()
    {
        Harness harness = await Harness.CreateAsync();
        ModelRouteRequest request = harness.Request();
        ModelRouteRequest tampered = request with
        {
            Consent = request.Consent with { Region = "otherregion" },
        };

        Assert.Equal(ModelRouteErrorCodes.ProfileBindingMismatch, harness.Refuse(tampered));
    }

    [Fact]
    public async Task An_unsupported_retention_mode_is_refused()
    {
        Harness harness = await Harness.CreateAsync();
        ModelRouteRequest request = harness.Request();
        ModelRouteRequest tampered = request with
        {
            RetentionMode = "retain_thirty_days",
            Consent = request.Consent with { RetentionMode = "retain_thirty_days" },
        };

        Assert.Equal(ModelRouteErrorCodes.ProfileBindingMismatch, harness.Refuse(tampered));
    }

    // --------------------------------------------------- pre-dispatch recheck

    [Fact]
    public async Task A_deployment_that_vanished_after_selection_fails_before_dispatch()
    {
        Harness harness = await Harness.CreateAsync();
        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.False(ModelRouteResolver.VerifyBeforeDispatch(
            route,
            ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.BetaDeploymentName,
                    ModelCatalogTestFixtures.BetaDeploymentHash)),
            out string errorCode));
        Assert.Equal(ModelRouteErrorCodes.DeploymentMissing, errorCode);
    }

    [Fact]
    public async Task A_deployment_that_now_serves_a_different_model_fails_before_dispatch()
    {
        Harness harness = await Harness.CreateAsync();
        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.False(ModelRouteResolver.VerifyBeforeDispatch(
            route,
            ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.AlphaDeploymentHash,
                    modelVersion: "9999-99-99")),
            out string errorCode));
        Assert.Equal(ModelRouteErrorCodes.DeploymentMissing, errorCode);
    }

    [Fact]
    public async Task A_deployment_whose_identity_hash_changed_fails_before_dispatch()
    {
        Harness harness = await Harness.CreateAsync();
        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.False(ModelRouteResolver.VerifyBeforeDispatch(
            route,
            ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.BetaDeploymentHash)),
            out string errorCode));
        Assert.Equal(ModelRouteErrorCodes.DeploymentMissing, errorCode);
    }

    [Fact]
    public async Task An_unchanged_deployment_passes_the_pre_dispatch_recheck()
    {
        Harness harness = await Harness.CreateAsync();
        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.True(ModelRouteResolver.VerifyBeforeDispatch(
            route,
            ModelCatalogTestFixtures.Snapshot(
                ModelCatalogTestFixtures.Deployment(
                    ModelCatalogTestFixtures.AlphaDeploymentName,
                    ModelCatalogTestFixtures.AlphaDeploymentHash)),
            out string errorCode));
        Assert.Equal("", errorCode);
    }

    // ----------------------------------------------------------- hidden lane

    [Fact]
    public async Task A_hidden_profile_is_resolvable_but_still_fully_checked()
    {
        Harness harness = await Harness.CreateAsync(
            profiles:
            [
                ModelCatalogTestFixtures.ParseAlpha() with { Visible = false },
                ModelCatalogTestFixtures.ParseBeta(),
            ]);

        ResolvedModelRoute route = harness.Resolve(harness.Request());

        Assert.Equal(ModelCatalogTestFixtures.AlphaProfileId, route.ProfileId);
        Assert.DoesNotContain(
            harness.Catalog.Projection.Profiles,
            profile => profile.ProfileId == ModelCatalogTestFixtures.AlphaProfileId);
    }

    // ----------------------------------------------------------------- helpers

    private sealed class Harness
    {
        private Harness(
            EffectiveModelCatalog catalog,
            ModelEntitlementSnapshot entitlement,
            PolicySnapshot policy)
        {
            Catalog = catalog;
            Entitlement = entitlement;
            Policy = policy;
        }

        public EffectiveModelCatalog Catalog { get; }

        public ModelEntitlementSnapshot Entitlement { get; }

        public PolicySnapshot Policy { get; }

        public static async Task<Harness> CreateAsync(
            IReadOnlyList<ModelProfile>? profiles = null,
            IReadOnlyList<string>? entitled = null,
            FoundryDeploymentCapabilities? capabilities = null)
        {
            IReadOnlyList<ModelProfile> resolved = profiles
                ?? [ModelCatalogTestFixtures.ParseAlpha(), ModelCatalogTestFixtures.ParseBeta()];
            ModelEntitlementSnapshot entitlement = ModelCatalogTestFixtures.Entitlement(
                entitled ?? [.. resolved.Select(static profile => profile.ProfileId)]);
            PolicySnapshot policy = ModelCatalogTestFixtures.Policy();
            EffectiveModelCatalogService service = new(
                new FixedRegistry(resolved),
                new FixedDeployments(capabilities),
                new FixedPolicy(policy),
                new FixedEntitlement(entitlement),
                new ModelCatalogTestFixtures.ControlledTimeProvider(DateTimeOffset.UnixEpoch),
                ModelCatalogTestFixtures.Region,
                untrustedRegistryPermitted: false);
            ModelCatalogResult result = await service.BuildAsync("synthetic-subject", null, Ct);
            Assert.True(result.IsSuccess, result.ErrorCode);
            return new Harness(result.Catalog!, entitlement, policy);
        }

        public ResolvedModelRoute Resolve(ModelRouteRequest request)
        {
            ModelRouteResolution resolution = ModelRouteResolver.Resolve(
                request,
                Catalog,
                Entitlement,
                Policy,
                ModelCatalogTestFixtures.Region);
            Assert.True(resolution.IsSuccess, resolution.ErrorCode);
            return resolution.Route!;
        }

        public string Refuse(ModelRouteRequest request)
        {
            ModelRouteResolution resolution = ModelRouteResolver.Resolve(
                request,
                Catalog,
                Entitlement,
                Policy,
                ModelCatalogTestFixtures.Region);
            Assert.False(resolution.IsSuccess);
            return resolution.ErrorCode;
        }

        public ModelRouteRequest Request(
            string profileId = ModelCatalogTestFixtures.AlphaProfileId,
            string? requestedVariantId = "balanced",
            string? consentVariantId = "balanced",
            string? catalogVersion = null,
            string? profileHash = null,
            string? capabilityHash = null,
            string? purpose = null,
            string? modelRole = null,
            string requestMode = "structured_response",
            string? canonicalOutputSchemaId = null,
            bool usesImageInput = false,
            int toolCount = 0,
            int estimatedInputTokens = 1_000,
            int requestedOutputTokens = 1_000)
        {
            EffectiveModelEntry? entry = Catalog.Find(profileId);
            string version = catalogVersion ?? Catalog.Projection.CatalogVersion;
            string boundProfileHash = profileHash
                ?? (entry is null
                    ? "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    : ModelRouteResolver.ComputeProfileBindingHash(
                        Catalog.Projection.CatalogVersion,
                        entry));
            string boundCapabilityHash = capabilityHash
                ?? (entry is null
                    ? "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    : ModelRouteResolver.ComputeCapabilityBindingHash(
                        Catalog.Projection.CatalogVersion,
                        profileId,
                        entry.Capabilities));
            // The schema is bound in both modes on purpose: in streaming chat
            // that is itself the refusal under test.
            string outputSchemaId = canonicalOutputSchemaId
                ?? "sapphirus.bmad-method-help-proposal.v1";
            const string outputSchemaHash =
                "sha256:6666666666666666666666666666666666666666666666666666666666666666";
            string boundPurpose = purpose ?? "bmad_help";
            string boundRole = modelRole ?? "planner";

            return new ModelRouteRequest(
                "request_01J00000000000000000000000",
                "dreg_01J00000000000000000000000",
                boundPurpose,
                boundRole,
                requestMode,
                profileId,
                requestedVariantId,
                version,
                boundProfileHash,
                boundCapabilityHash,
                outputSchemaId,
                outputSchemaHash,
                "transient_no_store",
                "interactive-standard",
                usesImageInput,
                UsesPdfInput: false,
                AttachmentBytes: 0,
                toolCount,
                estimatedInputTokens,
                requestedOutputTokens,
                new ModelRouteConsent(
                    profileId,
                    consentVariantId,
                    version,
                    requestMode,
                    boundPurpose,
                    boundRole,
                    outputSchemaId,
                    outputSchemaHash,
                    boundProfileHash,
                    boundCapabilityHash,
                    ModelCatalogTestFixtures.Region,
                    "transient_no_store"));
        }

        private sealed class FixedRegistry(IReadOnlyList<ModelProfile> profiles)
            : IModelProfileRegistry
        {
            public ValueTask<ModelProfileRegistrySnapshot> GetSnapshotAsync(
                CancellationToken cancellationToken) =>
                ValueTask.FromResult(new ModelProfileRegistrySnapshot(
                    ModelCatalogTestFixtures.ParityRegistryVersion,
                    ModelCatalogTestFixtures.ParityRegistryHash,
                    ModelCatalogTestFixtures.ParityCapabilityHash,
                    profiles,
                    ModelProfileTrust.Signed,
                    DateTimeOffset.UnixEpoch));
        }

        private sealed class FixedDeployments(FoundryDeploymentCapabilities? capabilities)
            : IModelCatalogDeploymentSource
        {
            public ValueTask<ModelDeploymentRead> GetAsync(CancellationToken cancellationToken) =>
                ValueTask.FromResult(new ModelDeploymentRead(
                    FoundryCatalogState.Fresh,
                    ModelCatalogTestFixtures.Snapshot(
                        ModelCatalogTestFixtures.Deployment(
                            ModelCatalogTestFixtures.AlphaDeploymentName,
                            ModelCatalogTestFixtures.AlphaDeploymentHash,
                            capabilities),
                        ModelCatalogTestFixtures.Deployment(
                            ModelCatalogTestFixtures.BetaDeploymentName,
                            ModelCatalogTestFixtures.BetaDeploymentHash,
                            capabilities)),
                    HasPreviousSnapshot: false));
        }

        private sealed class FixedPolicy(PolicySnapshot snapshot) : IModelCatalogPolicySource
        {
            public ValueTask<PolicySnapshot> GetSnapshotAsync(CancellationToken cancellationToken) =>
                ValueTask.FromResult(snapshot);
        }

        private sealed class FixedEntitlement(ModelEntitlementSnapshot snapshot)
            : IModelEntitlementSource
        {
            public ValueTask<ModelEntitlementSnapshot> GetAsync(
                string subjectPartition,
                CancellationToken cancellationToken) =>
                ValueTask.FromResult(snapshot);
        }
    }
}
