using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Policy;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Model;

public sealed class ModelBrokerTests
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string PayloadCanary = "CANARY_MODEL_BODY_1f9c2ab4";
    private const string ValidOutput =
        """{"summary":"Reviewed the context.","steps":["Understand","Plan"],"proposedChanges":[{"path":"src/example.cs","rationale":"Clarify naming."}]}""";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_valid_request_produces_a_signed_result_with_server_side_cost()
    {
        FakeExecutor executor = new([Ok(ValidOutput)]);
        CapturingReceiptSigner signer = new();
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, signer);

        ModelAccessResult result = await broker.CompleteAsync(
            "subject-a",
            CreateRequest(),
            Ct);

        Assert.Single(executor.Calls);
        UnsignedModelAccessResult unsigned = signer.LastUnsigned!;
        Assert.Equal("succeeded", unsigned.TerminalStatus);
        Assert.Equal(0, unsigned.RetryCount);
        // 1000 input at 2750/1k + 500 output at 11000/1k.
        Assert.Equal(2_750 + 5_500, unsigned.Usage.CostMicrounits);
        Assert.Equal("EUR", unsigned.Usage.Currency);
        Assert.Equal(
            "sha256:" + Convert.ToHexStringLower(SHA256.HashData(
                Encoding.UTF8.GetBytes(unsigned.PayloadJson))),
            unsigned.PayloadHash);
        Assert.NotNull(result);
    }

    [Theory]
    [InlineData("purpose")]
    [InlineData("model_role")]
    [InlineData("schema_id")]
    [InlineData("retention")]
    [InlineData("region")]
    public async Task Request_data_cannot_alter_the_fixed_profile(string mutation)
    {
        FakeExecutor executor = new([Ok(ValidOutput)]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());
        ModelAccessRequest request = CreateRequest();
        request = mutation switch
        {
            "purpose" => request with { Purpose = "exfiltrate" },
            "model_role" => request with { ModelRole = "operator" },
            "schema_id" => request with { CanonicalOutputSchemaId = "custom.schema.v9" },
            "retention" => request with { RetentionMode = "persistent" },
            "region" => request with
            {
                Consent = request.Consent with { Region = "eastus" },
            },
            _ => request,
        };

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", request, Ct));

        Assert.Equal("profile_mismatch", failure.Outcome);
        Assert.Empty(executor.Calls);
    }

    [Theory]
    [InlineData("/etc/passwd")]
    [InlineData("\\\\server\\share\\file.cs")]
    [InlineData("C:/repo/file.cs")]
    [InlineData("src/../secrets.txt")]
    [InlineData("Users/rodrigo/project/file.cs")]
    [InlineData("home/rodrigo/file.cs")]
    [InlineData("src/**")]
    [InlineData("src/*.cs")]
    public async Task Unsafe_labels_fail_before_provider_egress(string label)
    {
        FakeExecutor executor = new([Ok(ValidOutput)]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());
        ModelAccessRequest request = CreateRequest(relativeLabel: label);

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", request, Ct));

        Assert.Equal("context_rejected", failure.Outcome);
        Assert.Empty(executor.Calls);
    }

    [Theory]
    [InlineData("confidential")]
    [InlineData("secret")]
    public async Task Unsupported_classifications_fail_before_provider_egress(
        string classification)
    {
        FakeExecutor executor = new([Ok(ValidOutput)]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());
        ModelAccessRequest request = CreateRequest(classification: classification);

        await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
            broker.CompleteAsync("subject-a", request, Ct));
        Assert.Empty(executor.Calls);
    }

    [Theory]
    [InlineData("provider_refusal", false)]
    [InlineData("quota_exhausted", false)]
    [InlineData("content_filtered", false)]
    public async Task Terminal_provider_failures_map_to_safe_outcomes_without_retry(
        string outcome,
        bool retryable)
    {
        FakeExecutor executor = new([
            Fail(new ModelProviderException(outcome, retryable)),
            Ok(ValidOutput),
        ]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", CreateRequest(), Ct));

        Assert.Equal(outcome, failure.Outcome);
        Assert.Single(executor.Calls);
    }

    [Fact]
    public async Task Transient_failures_retry_bounded_with_identical_request_bytes()
    {
        FakeExecutor executor = new([
            Fail(new ModelProviderException("timeout", retryable: true)),
            Fail(new ModelProviderException("rate_limited", retryable: true)),
            Ok(ValidOutput),
        ]);
        CapturingReceiptSigner signer = new();
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, signer);

        _ = await broker.CompleteAsync("subject-a", CreateRequest(), Ct);

        Assert.Equal(3, executor.Calls.Count);
        Assert.Single(executor.Calls.Distinct());
        Assert.Equal(2, signer.LastUnsigned!.RetryCount);
    }

    [Fact]
    public async Task Retries_exhaust_at_the_profile_bound()
    {
        FakeExecutor executor = new([
            Fail(new ModelProviderException("timeout", retryable: true)),
            Fail(new ModelProviderException("timeout", retryable: true)),
            Fail(new ModelProviderException("timeout", retryable: true)),
            Ok(ValidOutput),
        ]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", CreateRequest(), Ct));

        Assert.Equal("timeout", failure.Outcome);
        Assert.Equal(3, executor.Calls.Count);
    }

    [Theory]
    [InlineData("not-json{", "malformed_output")]
    [InlineData("""{"summary":"s","steps":[],"proposedChanges":[],"extra":1}""", "schema_invalid")]
    [InlineData("""{"summary":"s","steps":[1],"proposedChanges":[]}""", "schema_invalid")]
    [InlineData("""{"summary":"s","steps":[]}""", "schema_invalid")]
    public async Task Invalid_provider_output_maps_to_explicit_outcomes(
        string output,
        string expectedOutcome)
    {
        FakeExecutor executor = new([Ok(output)]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", CreateRequest(), Ct));

        Assert.Equal(expectedOutcome, failure.Outcome);
    }

    [Theory]
    [InlineData("content_filter", "content_filtered")]
    [InlineData("length", "incomplete_output")]
    public async Task Non_stop_finish_reasons_map_to_safe_outcomes(
        string finishReason,
        string expectedOutcome)
    {
        FakeExecutor executor = new([
            () => Task.FromResult(new ModelProviderResponse(
                ValidOutput, 10, 10, "provider-id", finishReason)),
        ]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", CreateRequest(), Ct));

        Assert.Equal(expectedOutcome, failure.Outcome);
    }

    [Fact]
    public async Task Cancellation_propagates_as_cancellation()
    {
        using CancellationTokenSource cancelled = new();
        await cancelled.CancelAsync();
        AzureOpenAiModelAccessBroker broker = CreateBroker(
            new FakeExecutor([Ok(ValidOutput)]),
            new CapturingReceiptSigner());

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            broker.CompleteAsync("subject-a", CreateRequest(), cancelled.Token));
    }

    [Fact]
    public async Task Failure_surfaces_carry_no_provider_body_material()
    {
        FakeExecutor executor = new([
            Ok("{\"leaked\":\"" + PayloadCanary + "\"}"),
        ]);
        AzureOpenAiModelAccessBroker broker = CreateBroker(executor, new CapturingReceiptSigner());

        ModelAccessFailedException failure =
            await Assert.ThrowsAsync<ModelAccessFailedException>(() =>
                broker.CompleteAsync("subject-a", CreateRequest(), Ct));

        Assert.DoesNotContain(PayloadCanary, failure.ToString());
        Assert.DoesNotContain(PayloadCanary, failure.Message);
    }

    [Fact]
    public void The_profile_rejects_unapproved_deployments_and_regions()
    {
        PolicySnapshot policy = CreatePolicy();
        ProductionOptions production = CreateProduction();
        SupportPlaneOptions supportPlane = CreateSupportPlane();
        _ = ModelAccessProfile.Resolve(policy, production, supportPlane);

        Assert.Throws<InvalidOperationException>(() => ModelAccessProfile.Resolve(
            policy with { ApprovedModelDeployments = ["another-deployment"] },
            production,
            supportPlane));
        Assert.Throws<InvalidOperationException>(() => ModelAccessProfile.Resolve(
            policy with { AllowedRegions = ["eastus"] },
            production,
            supportPlane));
    }

    private static AzureOpenAiModelAccessBroker CreateBroker(
        FakeExecutor executor,
        IModelReceiptSigner signer) => new(
        _ => Task.FromResult(ModelAccessProfile.Resolve(
            CreatePolicy(),
            CreateProduction(),
            CreateSupportPlane())),
        executor,
        signer,
        TimeProvider.System);

    private static PolicySnapshot CreatePolicy() => new(
        "policy_01J00000000000000000000000",
        7,
        false,
        512 * 1024,
        64,
        ["westeurope"],
        ["desktop-planner"],
        "transient_no_store",
        DateTimeOffset.Parse("2026-07-20T10:00:00.000Z"));

    private static ProductionOptions CreateProduction() => new()
    {
        ManagedIdentityClientId = Guid.Parse("44444444-4444-4444-4444-444444444444"),
        AppConfigurationEndpoint = "https://configuration.azconfig.io/",
        KeyVaultUri = "https://signing.vault.azure.net/",
        ReceiptSigningKeyName = "model-receipt-signing",
        SqlServer = "authority.database.windows.net",
        SqlDatabase = "desktop-authority",
        ModelEndpoint = "https://models.openai.azure.com/",
        ModelDeployment = "desktop-planner",
        ProviderProfileHash = Hash,
        ModelProfileHash = Hash,
        ModelCapabilityHash = Hash,
        DeploymentHash = Hash,
    };

    private static SupportPlaneOptions CreateSupportPlane() => new()
    {
        Authority =
            "https://login.microsoftonline.com/11111111-1111-1111-1111-111111111111/v2.0",
        Audience = "api://22222222-2222-2222-2222-222222222222",
        ApprovedDesktopClientId = "33333333-3333-3333-3333-333333333333",
        Region = "westeurope",
    };

    private static ModelAccessRequest CreateRequest(
        string relativeLabel = "src/example.cs",
        string classification = "source")
    {
        const string content = "review this bounded context";
        ModelContextItem item = new(
            "context-item-1",
            relativeLabel,
            "implementation",
            "csharp",
            "sha256:" + Convert.ToHexStringLower(SHA256.HashData(
                Encoding.UTF8.GetBytes(content))),
            Encoding.UTF8.GetByteCount(content),
            classification,
            content);
        ModelContextConsent consent = new(
            "sapphirus.model-context-consent.v1",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            Hash,
            Hash,
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            Hash,
            "lease_01J00000000000000000000000",
            Hash,
            "policy_01J00000000000000000000000",
            7,
            Hash,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            DateTimeOffset.Parse("2026-07-20T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-20T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-20T10:05:00.000Z"),
            Hash,
            Hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                Hash,
                Hash,
                "ZXhhbXBsZQ"));
        return new ModelAccessRequest(
            "desktop-model-access-request.v1",
            consent.RequestId,
            "windows_local",
            consent.RegistrationId,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            Hash,
            Hash,
            consent,
            [item],
            "transient_no_store",
            "interactive-standard");
    }

    private static Func<Task<ModelProviderResponse>> Ok(string output) => () =>
        Task.FromResult(new ModelProviderResponse(output, 1000, 500, "provider-id", "stop"));

    private static Func<Task<ModelProviderResponse>> Fail(Exception exception) => () =>
        Task.FromException<ModelProviderResponse>(exception);

    private sealed class FakeExecutor(IReadOnlyList<Func<Task<ModelProviderResponse>>> outcomes)
        : IModelProviderExecutor
    {
        public List<ModelProviderRequest> Calls { get; } = [];

        public Task<ModelProviderResponse> ExecuteAsync(
            ModelProviderRequest request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            Calls.Add(request);
            return outcomes[Math.Min(Calls.Count - 1, outcomes.Count - 1)]();
        }
    }

    private sealed class CapturingReceiptSigner : IModelReceiptSigner
    {
        public UnsignedModelAccessResult? LastUnsigned { get; private set; }

        public Task<ModelAccessResult> SignAsync(
            string subject,
            ModelAccessRequest request,
            UnsignedModelAccessResult result,
            CancellationToken cancellationToken)
        {
            LastUnsigned = result;
            return Task.FromResult(new ModelAccessResult(
                "desktop-model-access-result.v1",
                request.RequestId,
                result.OutputSchemaId,
                result.PayloadJson,
                result.PayloadHash,
                null!));
        }
    }
}
