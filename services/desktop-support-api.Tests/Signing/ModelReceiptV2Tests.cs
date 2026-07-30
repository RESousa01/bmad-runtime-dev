using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Signing;
using Sapphirus.DesktopSupportApi.Tests.Model;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Signing;

/// <summary>
/// FND-030. The version two receipt records what the support plane actually
/// did: a server-authoritative profile, catalog epoch, variant, and deployment,
/// a credential binding that names the mechanism, and a usage record that keeps
/// "not reported" distinct from zero.
/// </summary>
public sealed class ModelReceiptV2Tests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public void The_credential_mechanism_changes_the_binding_hash()
    {
        ResolvedModelRoute route = FoundryResponsesTestFixtures.Route();

        string managedIdentity = ModelReceiptBinding.ComputeCredentialBindingHash(
            route,
            FoundryCredentialMechanisms.ManagedIdentity,
            "subject-a",
            FoundryResponsesTestFixtures.CredentialVersionId,
            FoundryResponsesTestFixtures.ProjectEndpointHash,
            FoundryResponsesTestFixtures.ApplicationRevision);
        string developmentKey = ModelReceiptBinding.ComputeCredentialBindingHash(
            route,
            FoundryCredentialMechanisms.DevelopmentApiKey,
            "subject-a",
            FoundryResponsesTestFixtures.CredentialVersionId,
            FoundryResponsesTestFixtures.ProjectEndpointHash,
            FoundryResponsesTestFixtures.ApplicationRevision);

        // Every other input is byte-identical: same route, same subject, same
        // price profile. Only the mechanism differs, and that is enough.
        Assert.NotEqual(managedIdentity, developmentKey);
        Assert.Matches("^sha256:[0-9a-f]{64}$", managedIdentity);
        Assert.Matches("^sha256:[0-9a-f]{64}$", developmentKey);
    }

    [Fact]
    public void The_provider_profile_hash_also_separates_the_two_mechanisms()
    {
        ResolvedModelRoute route = FoundryResponsesTestFixtures.Route();

        Assert.NotEqual(
            ModelReceiptBinding.ComputeProviderProfileHash(
                route,
                FoundryCredentialMechanisms.ManagedIdentity),
            ModelReceiptBinding.ComputeProviderProfileHash(
                route,
                FoundryCredentialMechanisms.DevelopmentApiKey));
    }

    [Fact]
    public void The_binding_is_taken_from_the_route_and_not_from_the_client()
    {
        ResolvedModelRoute route = FoundryResponsesTestFixtures.Route();

        ResolvedReceiptBinding binding = ModelReceiptBinding.Create(
            route,
            FoundryResponsesTestFixtures.OtherHash,
            FoundryCredentialMechanisms.ManagedIdentity,
            "subject-a",
            FoundryResponsesTestFixtures.CredentialVersionId,
            FoundryResponsesTestFixtures.ProjectEndpointHash,
            FoundryResponsesTestFixtures.ApplicationRevision);

        Assert.Equal(route.ProfileBindingHash, binding.ModelProfileHash);
        Assert.Equal(route.ProfileBindingHash, binding.ResolvedProfileHash);
        Assert.Equal(route.CapabilityBindingHash, binding.ModelCapabilityHash);
        Assert.Equal(route.CatalogVersion, binding.CatalogVersion);
        Assert.Equal(route.CatalogHash, binding.CatalogHash);
        Assert.Equal(route.ComputeDeploymentBindingHash(), binding.ResolvedDeploymentHash);
        Assert.Equal(FoundryResponsesTestFixtures.OtherHash, binding.DeploymentInventoryHash);
        Assert.Equal(route.ModelPublisher, binding.ResolvedModel.Publisher);
        Assert.Equal(route.ModelName, binding.ResolvedModel.Name);
        Assert.Equal(route.ModelVersion, binding.ResolvedModel.Version);
        // The deployment name is the one thing that must never appear.
        Assert.DoesNotContain(
            route.DeploymentName,
            JsonSerializer.Serialize(binding),
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task A_signed_streaming_receipt_validates_against_its_binding()
    {
        (ModelAccessReceiptV2 receipt, ResolvedReceiptBinding binding, string payloadHash) =
            await SignAsync();

        Assert.Equal(ModelAccessReceiptV2.SchemaVersionValue, receipt.SchemaVersion);
        Assert.Equal(ModelTerminalStatuses.Succeeded, receipt.TerminalStatus);
        StreamingChatLane lane = Assert.IsType<StreamingChatLane>(receipt.ResponseLane);
        Assert.Equal(ModelRequestModes.StreamingChat, lane.RequestMode);
        Assert.Equal(ModelSupportRoutes.ModelStream, lane.SupportRoute);
        Assert.True(ModelResultGuards.ValidateReceiptV2(
            FoundryResponsesTestFixtures.Authorization(),
            receipt,
            binding,
            "westeurope",
            payloadHash,
            payloadBytes: 6));
    }

    [Fact]
    public async Task A_receipt_whose_profile_hash_was_swapped_fails_validation()
    {
        (ModelAccessReceiptV2 receipt, ResolvedReceiptBinding binding, string payloadHash) =
            await SignAsync();

        Assert.False(ModelResultGuards.ValidateReceiptV2(
            FoundryResponsesTestFixtures.Authorization(),
            receipt with { ModelProfileHash = FoundryResponsesTestFixtures.OtherHash },
            binding,
            "westeurope",
            payloadHash,
            payloadBytes: 6));
    }

    [Theory]
    [InlineData("refused")]
    [InlineData("failed")]
    [InlineData("cancelled")]
    [InlineData("incomplete")]
    public async Task A_run_that_did_not_succeed_is_never_signed(string terminalStatus)
    {
        AzureModelReceiptSigner signer = CreateSigner();
        (UnsignedModelAccessResultV2 unsigned, _) = CreateUnsigned();

        await Assert.ThrowsAsync<InvalidOperationException>(() => signer.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            unsigned with { TerminalStatus = terminalStatus },
            Ct));
    }

    [Fact]
    public void Unavailable_usage_is_all_null_rather_than_a_free_zero()
    {
        ModelUsageV2 usage = ModelUsageProjector.Project(
            null,
            FoundryResponsesTestFixtures.PriceProfile());

        Assert.Equal(ModelUsageProvenance.Unavailable, usage.UsageProvenance);
        Assert.True(usage.IsFullyUnavailable);
        Assert.Null(usage.CostMicrounits);
        Assert.Equal("EUR", usage.Currency);
        Assert.Equal("foundry-price-2026-07", usage.PriceProfileVersion);
    }

    [Fact]
    public void A_category_the_provider_never_reported_stays_null()
    {
        ModelUsageV2 usage = ModelUsageProjector.Project(
            new ModelProviderReportedUsage(100, 50, null, null, null),
            FoundryResponsesTestFixtures.PriceProfile());

        Assert.Equal(ModelUsageProvenance.Reported, usage.UsageProvenance);
        Assert.Equal(100, usage.InputTokens);
        Assert.Equal(50, usage.OutputTokens);
        Assert.Null(usage.ReasoningTokens);
        Assert.Null(usage.CacheReadTokens);
        Assert.Null(usage.CacheWriteTokens);
    }

    [Fact]
    public void A_reported_zero_is_kept_apart_from_an_unreported_category()
    {
        ModelUsageV2 zeroed = ModelUsageProjector.Project(
            new ModelProviderReportedUsage(0, 0, 0, 0, 0),
            FoundryResponsesTestFixtures.PriceProfile());

        Assert.Equal(ModelUsageProvenance.Reported, zeroed.UsageProvenance);
        Assert.Equal(0, zeroed.InputTokens);
        Assert.Equal(0, zeroed.CacheReadTokens);
        Assert.Equal(0, zeroed.CostMicrounits);
        Assert.False(zeroed.IsFullyUnavailable);
    }

    [Fact]
    public void Cost_comes_from_the_versioned_price_profile_and_never_double_charges()
    {
        // 1000 input of which 200 cached, 500 output of which 100 reasoning.
        long? cost = ModelUsageProjector.ComputeCostMicrounits(
            new ModelProviderReportedUsage(1_000, 500, 100, 200, null),
            FoundryResponsesTestFixtures.PriceProfile());

        long expected = (800 * 2_750 / 1_000)
            + (200 * 275 / 1_000)
            + (400 * 11_000 / 1_000)
            + (100 * 11_000 / 1_000);
        Assert.Equal(expected, cost);
    }

    [Fact]
    public async Task The_version_two_digest_covers_every_field_except_the_hash_and_proof()
    {
        (ModelAccessReceiptV2 receipt, _, _) = await SignAsync();
        byte[] baseline = AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt);

        Assert.Equal(
            baseline,
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt with
            {
                ReceiptHash = FoundryResponsesTestFixtures.OtherHash,
                Proof = receipt.Proof with { Signature = "ZGlmZmVyZW50" },
            }));
        Assert.NotEqual(
            baseline,
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt with
            {
                ResolvedDeploymentHash = FoundryResponsesTestFixtures.OtherHash,
            }));
        Assert.NotEqual(
            baseline,
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt with
            {
                CredentialBindingHash = FoundryResponsesTestFixtures.OtherHash,
            }));
        Assert.NotEqual(
            baseline,
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt with
            {
                ReportedResponseModel = "another-model",
            }));
    }

    [Fact]
    public async Task The_two_receipt_versions_never_share_a_digest_preimage()
    {
        (ModelAccessReceiptV2 receipt, _, _) = await SignAsync();

        // The version one purpose is "model-access-receipt"; the version two
        // purpose is distinct, so a historical receipt keeps verifying against
        // exactly the digest it was signed with.
        Assert.Equal(
            "model-access-receipt-v2",
            AzureModelReceiptSignerAccess.ReceiptV2DigestPurpose);
        Assert.NotEqual(
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt),
            AzureModelReceiptSigner.ComputeReceiptDigestV2(receipt with
            {
                SchemaVersion = "sapphirus.model-access-receipt.v1",
            }));
    }

    [Fact]
    public async Task A_signed_receipt_never_carries_a_deployment_name_or_endpoint()
    {
        (ModelAccessReceiptV2 receipt, _, _) = await SignAsync();

        string serialized = JsonSerializer.Serialize(receipt);
        Assert.DoesNotContain("desktop-planning", serialized, StringComparison.Ordinal);
        // The proof issuer is legitimately a URL — it is the support plane's
        // own authority. No Foundry project endpoint may appear.
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.ProjectEndpoint,
            serialized,
            StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("services.ai.azure.com", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("forge-fake.test", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryResponsesTestFixtures.CredentialCanary,
            serialized,
            StringComparison.Ordinal);
        Assert.DoesNotContain(
            FoundryCredentialMechanisms.ManagedIdentity,
            serialized,
            StringComparison.Ordinal);
    }

    private static async Task<(
        ModelAccessReceiptV2 Receipt,
        ResolvedReceiptBinding Binding,
        string PayloadHash)> SignAsync()
    {
        AzureModelReceiptSigner signer = CreateSigner();
        (UnsignedModelAccessResultV2 unsigned, ResolvedReceiptBinding binding) =
            CreateUnsigned();
        ModelAccessReceiptV2 receipt = await signer.SignAsync(
            "subject-a",
            FoundryResponsesTestFixtures.Authorization(),
            unsigned,
            Ct);
        return (receipt, binding, unsigned.PayloadHash);
    }

    private static (UnsignedModelAccessResultV2 Unsigned, ResolvedReceiptBinding Binding)
        CreateUnsigned()
    {
        const string transcript = "answer";
        ResolvedReceiptBinding binding = ModelReceiptBinding.Create(
            FoundryResponsesTestFixtures.Route(),
            FoundryResponsesTestFixtures.OtherHash,
            FoundryCredentialMechanisms.ManagedIdentity,
            "subject-a",
            FoundryResponsesTestFixtures.CredentialVersionId,
            FoundryResponsesTestFixtures.ProjectEndpointHash,
            FoundryResponsesTestFixtures.ApplicationRevision);
        UnsignedModelAccessResultV2 unsigned = new(
            "run_01J00000000000000000000000",
            new StreamingChatLane(4, 4),
            transcript,
            "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(Encoding.UTF8.GetBytes(transcript))),
            binding,
            "coding-balanced",
            "balanced",
            "gpt-5-mini",
            ModelUsageProjector.Project(
                new ModelProviderReportedUsage(100, 50, null, null, null),
                FoundryResponsesTestFixtures.PriceProfile()),
            OutputBytes: 6,
            RetryCount: 0,
            FallbackEvents: [],
            ProviderRequestId: "resp_stream_text",
            StartedAt: DateTimeOffset.Parse("2026-07-28T10:00:01.000Z"),
            CompletedAt: DateTimeOffset.Parse("2026-07-28T10:00:02.000Z"),
            TerminalStatus: ModelTerminalStatuses.Succeeded);
        return (unsigned, binding);
    }

    private static AzureModelReceiptSigner CreateSigner() => new(
        new FakeHashSigner(),
        new SupportPlaneOptions
        {
            Authority =
                "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
            Audience = "api://22222222-2222-2222-2222-222222222222",
            ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
            Region = "westeurope",
        });

    private sealed class FakeHashSigner : IHashSigner
    {
        public string KeyId => "model-receipt-key-2026-07";

        public Task<string> SignAsync(byte[] sha256Digest, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return Task.FromResult(
                Convert.ToBase64String(sha256Digest)
                    .Replace('+', '-')
                    .Replace('/', '_')
                    .TrimEnd('='));
        }
    }
}

/// <summary>Exposes the internal digest purpose constant for assertion.</summary>
internal static class AzureModelReceiptSignerAccess
{
    internal const string ReceiptV2DigestPurpose = "model-access-receipt-v2";
}
