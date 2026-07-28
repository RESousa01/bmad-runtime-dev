using Sapphirus.DesktopSupportApi.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

/// <summary>
/// FND-024. The signed organization model profile registry.
/// </summary>
public sealed class ModelProfileRegistryTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    // ------------------------------------------------- cross-language parity

    [Fact]
    public void The_profile_value_hash_matches_the_publishers_canonicalizer()
    {
        Assert.Equal(
            ModelCatalogTestFixtures.ParityAlphaProfileHash,
            ModelProfileValidator.ComputeProfileValueHash(ModelCatalogTestFixtures.AlphaValue));
        Assert.Equal(
            ModelCatalogTestFixtures.ParityBetaProfileHash,
            ModelProfileValidator.ComputeProfileValueHash(ModelCatalogTestFixtures.BetaValue));
    }

    [Fact]
    public void The_registry_hash_matches_the_publishers_canonicalizer()
    {
        string hash = ModelProfileValidator.ComputeRegistryHash(
            ModelCatalogTestFixtures.ParityRegistryVersion,
            [
                (ModelCatalogTestFixtures.AlphaProfileId,
                    ModelCatalogTestFixtures.ParityAlphaProfileHash),
                (ModelCatalogTestFixtures.BetaProfileId,
                    ModelCatalogTestFixtures.ParityBetaProfileHash),
            ]);

        Assert.Equal(ModelCatalogTestFixtures.ParityRegistryHash, hash);
    }

    [Fact]
    public void The_capability_hash_matches_the_publishers_canonicalizer()
    {
        string hash = ModelProfileValidator.ComputeCapabilityHash(
            [ModelCatalogTestFixtures.ParseAlpha(), ModelCatalogTestFixtures.ParseBeta()]);

        Assert.Equal(ModelCatalogTestFixtures.ParityCapabilityHash, hash);
    }

    [Fact]
    public void The_registry_hash_is_canonicalized_in_profile_identifier_order()
    {
        string forward = ModelProfileValidator.ComputeRegistryHash(
            ModelCatalogTestFixtures.ParityRegistryVersion,
            [
                (ModelCatalogTestFixtures.AlphaProfileId,
                    ModelCatalogTestFixtures.ParityAlphaProfileHash),
                (ModelCatalogTestFixtures.BetaProfileId,
                    ModelCatalogTestFixtures.ParityBetaProfileHash),
            ]);
        string reversed = ModelProfileValidator.ComputeRegistryHash(
            ModelCatalogTestFixtures.ParityRegistryVersion,
            [
                (ModelCatalogTestFixtures.BetaProfileId,
                    ModelCatalogTestFixtures.ParityBetaProfileHash),
                (ModelCatalogTestFixtures.AlphaProfileId,
                    ModelCatalogTestFixtures.ParityAlphaProfileHash),
            ]);

        Assert.Equal(forward, reversed);
    }

    // ------------------------------------------------------------ key layout

    [Fact]
    public void The_registry_version_key_is_outside_the_publisher_profile_prefix()
    {
        Assert.False(
            AppConfigurationModelProfileSettingsSource.RegistryVersionKey.StartsWith(
                AppConfigurationModelProfileSettingsSource.PublisherKeyPrefix,
                StringComparison.Ordinal));
    }

    // ------------------------------------------------------ happy-path read

    [Fact]
    public async Task A_published_profile_set_validates_into_a_signed_snapshot()
    {
        AppConfigurationModelProfileRegistry registry = Build(
            new StubSource(ModelCatalogTestFixtures.ParitySettings));

        ModelProfileRegistrySnapshot snapshot = await registry.GetSnapshotAsync(Ct);

        Assert.Equal(ModelProfileTrust.Signed, snapshot.Trust);
        Assert.Equal(ModelCatalogTestFixtures.ParityRegistryVersion, snapshot.RegistryVersion);
        Assert.Equal(ModelCatalogTestFixtures.ParityRegistryHash, snapshot.RegistryHash);
        Assert.Equal(ModelCatalogTestFixtures.ParityCapabilityHash, snapshot.CapabilityHash);
        Assert.Equal(2, snapshot.Profiles.Count);
    }

    [Fact]
    public async Task A_cached_snapshot_is_served_without_a_second_store_read()
    {
        StubSource source = new(ModelCatalogTestFixtures.ParitySettings);
        AppConfigurationModelProfileRegistry registry = Build(source);

        _ = await registry.GetSnapshotAsync(Ct);
        _ = await registry.GetSnapshotAsync(Ct);

        Assert.Equal(1, source.LoadCount);
    }

    [Fact]
    public async Task A_transient_load_failure_keeps_serving_the_last_known_valid_snapshot()
    {
        StubSource source = new(ModelCatalogTestFixtures.ParitySettings);
        ModelCatalogTestFixtures.ControlledTimeProvider time = new(DateTimeOffset.UnixEpoch);
        AppConfigurationModelProfileRegistry registry = new(
            source,
            time,
            TimeSpan.FromMinutes(5),
            TimeSpan.FromHours(1));
        _ = await registry.GetSnapshotAsync(Ct);
        source.FailNext = true;
        time.Advance(TimeSpan.FromMinutes(6));

        ModelProfileRegistrySnapshot snapshot = await registry.GetSnapshotAsync(Ct);

        Assert.Equal(ModelCatalogTestFixtures.ParityRegistryHash, snapshot.RegistryHash);
    }

    [Fact]
    public async Task A_transient_load_failure_beyond_the_last_known_valid_window_fails_closed()
    {
        StubSource source = new(ModelCatalogTestFixtures.ParitySettings);
        ModelCatalogTestFixtures.ControlledTimeProvider time = new(DateTimeOffset.UnixEpoch);
        AppConfigurationModelProfileRegistry registry = new(
            source,
            time,
            TimeSpan.FromMinutes(5),
            TimeSpan.FromMinutes(30));
        _ = await registry.GetSnapshotAsync(Ct);
        source.FailNext = true;
        time.Advance(TimeSpan.FromMinutes(31));

        // The load failure propagates rather than being converted into a stale
        // answer: past the window there is nothing valid left to serve.
        await Assert.ThrowsAsync<TimeoutException>(async () =>
            await registry.GetSnapshotAsync(Ct));
    }

    [Fact]
    public async Task A_registry_version_rollback_fails_closed()
    {
        StubSource source = new(ModelCatalogTestFixtures.ParitySettings);
        ModelCatalogTestFixtures.ControlledTimeProvider time = new(DateTimeOffset.UnixEpoch);
        AppConfigurationModelProfileRegistry registry = new(
            source,
            time,
            TimeSpan.FromMinutes(5),
            TimeSpan.FromHours(1));
        _ = await registry.GetSnapshotAsync(Ct);

        // A lower registry version, republished with an internally consistent
        // record, is still a rollback.
        source.Settings = Rebuild(registryVersion: 6);
        time.Advance(TimeSpan.FromMinutes(6));

        InvalidOperationException failure = await Assert.ThrowsAsync<InvalidOperationException>(
            async () => await registry.GetSnapshotAsync(Ct));
        Assert.Equal("model_profile_registry_version_rollback", failure.Message);
    }

    // ------------------------------------------------------- record integrity

    [Fact]
    public void A_registry_hash_that_does_not_match_the_stored_values_fails_closed()
    {
        ModelProfileSettings settings = new(
            ModelCatalogTestFixtures.ParitySettings.ProfileValues,
            ModelCatalogTestFixtures.RegistryRecordValue(
                ModelCatalogTestFixtures.ParityRegistryVersion,
                "sha256:0000000000000000000000000000000000000000000000000000000000000001",
                ModelCatalogTestFixtures.ParityCapabilityHash,
                ModelCatalogTestFixtures.AlphaProfileId,
                ModelCatalogTestFixtures.BetaProfileId));

        Assert.Equal(
            "model_profile_registry_hash_mismatch",
            Refuse(settings));
    }

    [Fact]
    public void A_capability_hash_that_does_not_match_the_capability_surface_fails_closed()
    {
        string registryHash = ModelProfileValidator.ComputeRegistryHash(
            ModelCatalogTestFixtures.ParityRegistryVersion,
            [
                (ModelCatalogTestFixtures.AlphaProfileId,
                    ModelCatalogTestFixtures.ParityAlphaProfileHash),
                (ModelCatalogTestFixtures.BetaProfileId,
                    ModelCatalogTestFixtures.ParityBetaProfileHash),
            ]);
        ModelProfileSettings settings = new(
            ModelCatalogTestFixtures.ParitySettings.ProfileValues,
            ModelCatalogTestFixtures.RegistryRecordValue(
                ModelCatalogTestFixtures.ParityRegistryVersion,
                registryHash,
                "sha256:0000000000000000000000000000000000000000000000000000000000000002",
                ModelCatalogTestFixtures.AlphaProfileId,
                ModelCatalogTestFixtures.BetaProfileId));

        Assert.Equal("model_profile_capability_hash_mismatch", Refuse(settings));
    }

    [Fact]
    public void A_stored_profile_value_edited_after_publication_fails_closed()
    {
        Dictionary<string, string> values = new(StringComparer.Ordinal)
        {
            [ModelCatalogTestFixtures.AlphaProfileId] =
                ModelCatalogTestFixtures.AlphaValue.Replace(
                    "\"maxOutputTokens\":32000",
                    "\"maxOutputTokens\":64000",
                    StringComparison.Ordinal),
            [ModelCatalogTestFixtures.BetaProfileId] = ModelCatalogTestFixtures.BetaValue,
        };

        Assert.Equal(
            "model_profile_registry_hash_mismatch",
            Refuse(new ModelProfileSettings(
                values,
                ModelCatalogTestFixtures.ParityRegistryRecord)));
    }

    [Fact]
    public void A_profile_named_by_the_record_but_absent_from_the_store_fails_closed()
    {
        Dictionary<string, string> values = new(StringComparer.Ordinal)
        {
            [ModelCatalogTestFixtures.AlphaProfileId] = ModelCatalogTestFixtures.AlphaValue,
        };

        Assert.Equal(
            "model_profile_registry_profile_missing",
            Refuse(new ModelProfileSettings(
                values,
                ModelCatalogTestFixtures.ParityRegistryRecord)));
    }

    [Fact]
    public void A_profile_present_in_the_store_but_absent_from_the_record_is_inert()
    {
        Dictionary<string, string> values = new(StringComparer.Ordinal)
        {
            [ModelCatalogTestFixtures.AlphaProfileId] = ModelCatalogTestFixtures.AlphaValue,
            [ModelCatalogTestFixtures.BetaProfileId] = ModelCatalogTestFixtures.BetaValue,
        };
        string registryHash = ModelProfileValidator.ComputeRegistryHash(
            3,
            [(ModelCatalogTestFixtures.AlphaProfileId,
                ModelCatalogTestFixtures.ParityAlphaProfileHash)]);
        string capabilityHash = ModelProfileValidator.ComputeCapabilityHash(
            [ModelCatalogTestFixtures.ParseAlpha()]);

        ModelProfileRegistrySnapshot snapshot = AppConfigurationModelProfileRegistry.Validate(
            new ModelProfileSettings(
                values,
                ModelCatalogTestFixtures.RegistryRecordValue(
                    3,
                    registryHash,
                    capabilityHash,
                    ModelCatalogTestFixtures.AlphaProfileId)),
            DateTimeOffset.UnixEpoch);

        Assert.Equal(ModelCatalogTestFixtures.AlphaProfileId, Assert.Single(snapshot.Profiles).ProfileId);
    }

    [Theory]
    [InlineData("{}")]
    [InlineData("not json at all")]
    [InlineData("""{"schemaVersion":"forge.model-profile-registry.v2","registryVersion":1,"registryHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","capabilityHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","profileIds":[]}""")]
    [InlineData("""{"schemaVersion":"forge.model-profile-registry.v1","registryVersion":0,"registryHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","capabilityHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","profileIds":[]}""")]
    [InlineData("""{"schemaVersion":"forge.model-profile-registry.v1","registryVersion":1,"registryHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","capabilityHash":"sha256:1111111111111111111111111111111111111111111111111111111111111111","profileIds":[],"extra":1}""")]
    public void An_unusable_registry_record_is_refused(string record)
    {
        Assert.False(ModelProfileValidator.TryParseRegistryRecord(record, out _, out _));
    }

    [Fact]
    public void A_registry_record_naming_the_same_profile_twice_is_refused()
    {
        string record = ModelCatalogTestFixtures.RegistryRecordValue(
            1,
            ModelCatalogTestFixtures.ParityRegistryHash,
            ModelCatalogTestFixtures.ParityCapabilityHash,
            ModelCatalogTestFixtures.AlphaProfileId,
            ModelCatalogTestFixtures.AlphaProfileId);

        Assert.False(
            ModelProfileValidator.TryParseRegistryRecord(record, out _, out string errorCode));
        Assert.Equal("model_profile_registry_record_duplicate_id", errorCode);
    }

    // -------------------------------------------------------- profile parsing

    [Fact]
    public void A_profile_member_the_contract_does_not_declare_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"visible\":true}",
            "\"visible\":true,\"deploymentEndpoint\":\"https://example.invalid\"}",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_unknown_member", errorCode);
    }

    [Fact]
    public void A_repeated_profile_member_is_refused_rather_than_overwritten()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"visible\":true}",
            "\"visible\":true,\"visible\":false}",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_duplicate_member", errorCode);
    }

    [Fact]
    public void A_profile_member_spelled_with_different_casing_is_not_bound()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"profileId\":",
            "\"ProfileId\":",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_unknown_member", errorCode);
    }

    [Fact]
    public void A_profile_of_an_unsupported_schema_version_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "sapphirus.model-profile.v1",
            "sapphirus.model-profile.v2",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_schema_version_unsupported", errorCode);
    }

    [Fact]
    public void A_profile_value_beyond_the_stored_value_bound_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "Synthetic parity fixture for synthetic-alpha.",
            new string('x', ModelProfileValidator.MaximumProfileValueBytes + 1),
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_value_out_of_bounds", errorCode);
    }

    [Theory]
    [InlineData("\"maxInputTokens\":220000", "\"maxInputTokens\":0", "model_profile_limit_not_positive")]
    [InlineData("\"contextTokens\":256000", "\"contextTokens\":-1", "model_profile_limit_not_positive")]
    [InlineData("\"transient_no_store\"", "\"retain_thirty_days\"", "model_profile_retention_unsupported")]
    public void An_out_of_policy_profile_field_is_refused(
        string original,
        string replacement,
        string expectedCode)
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            original,
            replacement,
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal(expectedCode, errorCode);
    }

    [Fact]
    public void A_variant_that_outruns_the_profile_output_ceiling_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"maxOutputTokens\":16000",
            "\"maxOutputTokens\":48000",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_variant_exceeds_output_ceiling", errorCode);
    }

    [Fact]
    public void A_profile_declaring_the_same_variant_twice_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"variantId\":\"deep\"",
            "\"variantId\":\"balanced\"",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_variant_duplicate", errorCode);
    }

    [Theory]
    [InlineData("../../etc/passwd")]
    [InlineData("deployment name with spaces")]
    [InlineData("-leading-hyphen")]
    [InlineData("trailing-hyphen-")]
    [InlineData("a")]
    public void An_unsafe_deployment_name_is_refused(string deploymentName)
    {
        Assert.False(ModelProfileValidator.IsSafeDeploymentName(deploymentName));
    }

    [Fact]
    public void A_deployment_name_outside_printable_ascii_is_refused()
    {
        // Refused rather than normalized or escaped: either would mean the
        // stored bytes and the dispatched name could differ.
        Assert.False(ModelProfileValidator.IsSafeDeploymentName("deployment-\u00e9"));
        Assert.False(ModelProfileValidator.IsSafeDeploymentName("deployment-e\u0301"));
        Assert.False(ModelProfileValidator.IsSafeDeploymentName("deployment\u0000name"));
    }

    [Fact]
    public void A_profile_whose_qualification_evidence_is_a_placeholder_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            ModelCatalogTestFixtures.SyntheticEvidenceHash,
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_qualification_placeholder", errorCode);
    }

    [Fact]
    public void A_qualified_profile_without_a_qualification_instant_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"qualifiedAt\":\"2026-07-28T00:00:00.000Z\"",
            "\"qualifiedAt\":null",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_qualification_missing", errorCode);
    }

    [Fact]
    public void A_modality_without_its_input_capability_is_refused()
    {
        string value = ModelCatalogTestFixtures.AlphaValue.Replace(
            "\"supportedModalities\":[\"text\"]",
            "\"supportedModalities\":[\"text\",\"image\"]",
            StringComparison.Ordinal);

        Assert.False(ModelProfileValidator.TryParseProfile(value, out _, out string errorCode));
        Assert.Equal("model_profile_modality_contradicts_capability", errorCode);
    }

    // ------------------------------------------------------------ set checks

    [Fact]
    public void Two_profiles_sharing_an_identifier_are_refused()
    {
        ModelProfile profile = ModelCatalogTestFixtures.ParseAlpha();

        Assert.False(ModelProfileValidator.ValidateSet([profile, profile], out string errorCode));
        Assert.Equal("model_profile_id_duplicate", errorCode);
    }

    [Fact]
    public void Two_visible_profiles_sharing_a_display_name_are_refused()
    {
        ModelProfile alpha = ModelCatalogTestFixtures.ParseAlpha();
        ModelProfile clash = ModelCatalogTestFixtures.ParseBeta() with
        {
            DisplayName = "  Synthetic   SYNTHETIC-alpha ",
        };

        Assert.False(ModelProfileValidator.ValidateSet([alpha, clash], out string errorCode));
        Assert.Equal("model_profile_display_name_duplicate", errorCode);
    }

    [Fact]
    public void A_hidden_profile_may_reuse_a_visible_display_name()
    {
        ModelProfile alpha = ModelCatalogTestFixtures.ParseAlpha();
        ModelProfile hidden = ModelCatalogTestFixtures.ParseBeta() with
        {
            DisplayName = alpha.DisplayName,
            Visible = false,
        };

        Assert.True(ModelProfileValidator.ValidateSet([alpha, hidden], out _));
    }

    // ---------------------------------------------------- development fixture

    [Fact]
    public async Task The_development_fixture_registry_is_labelled_untrusted()
    {
        DevelopmentFixtureModelProfileRegistry registry = new(
            "Development",
            DevelopmentModelProfileFixtures.Build(ModelCatalogTestFixtures.Region),
            TimeProvider.System);

        ModelProfileRegistrySnapshot snapshot = await registry.GetSnapshotAsync(Ct);

        Assert.Equal(ModelProfileTrust.DevelopmentFixtureUntrusted, snapshot.Trust);
    }

    [Theory]
    [InlineData("Production")]
    [InlineData("Staging")]
    [InlineData("development")]
    [InlineData("")]
    public void The_development_fixture_registry_refuses_any_other_environment(string environment)
    {
        InvalidOperationException failure = Assert.Throws<InvalidOperationException>(() =>
            new DevelopmentFixtureModelProfileRegistry(
                environment,
                DevelopmentModelProfileFixtures.Build(ModelCatalogTestFixtures.Region),
                TimeProvider.System));

        Assert.Equal(
            DevelopmentFixtureModelProfileRegistry.EnvironmentForbiddenCode,
            failure.Message);
    }

    [Fact]
    public void The_development_fixture_deployment_name_is_obviously_synthetic()
    {
        Assert.Equal(
            "synthetic-development-deployment",
            DevelopmentModelProfileFixtures.SyntheticDeploymentName);
    }

    // ----------------------------------------------------------------- helpers

    private static AppConfigurationModelProfileRegistry Build(StubSource source) =>
        new(source, new ModelCatalogTestFixtures.ControlledTimeProvider(DateTimeOffset.UnixEpoch));

    private static string Refuse(ModelProfileSettings settings)
    {
        InvalidOperationException failure = Assert.Throws<InvalidOperationException>(() =>
            AppConfigurationModelProfileRegistry.Validate(settings, DateTimeOffset.UnixEpoch));
        return failure.Message;
    }

    private static ModelProfileSettings Rebuild(long registryVersion)
    {
        string registryHash = ModelProfileValidator.ComputeRegistryHash(
            registryVersion,
            [
                (ModelCatalogTestFixtures.AlphaProfileId,
                    ModelCatalogTestFixtures.ParityAlphaProfileHash),
                (ModelCatalogTestFixtures.BetaProfileId,
                    ModelCatalogTestFixtures.ParityBetaProfileHash),
            ]);
        return new ModelProfileSettings(
            ModelCatalogTestFixtures.ParitySettings.ProfileValues,
            ModelCatalogTestFixtures.RegistryRecordValue(
                registryVersion,
                registryHash,
                ModelCatalogTestFixtures.ParityCapabilityHash,
                ModelCatalogTestFixtures.AlphaProfileId,
                ModelCatalogTestFixtures.BetaProfileId));
    }

    private sealed class StubSource(ModelProfileSettings settings) : IModelProfileSettingsSource
    {
        public ModelProfileSettings Settings { get; set; } = settings;

        public bool FailNext { get; set; }

        public int LoadCount { get; private set; }

        public Task<ModelProfileSettings> LoadAsync(CancellationToken cancellationToken)
        {
            LoadCount++;
            if (FailNext)
            {
                throw new TimeoutException("The synthetic store did not answer.");
            }
            return Task.FromResult(Settings);
        }
    }
}
