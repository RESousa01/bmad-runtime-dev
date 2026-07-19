using System.ClientModel;
using System.Security.Cryptography;
using System.Text;
using Azure.AI.OpenAI;
using OpenAI.Chat;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// A model access failure with a stable safe outcome code. Neither the
/// outcome nor the message ever carries provider request or response bodies.
/// </summary>
public sealed class ModelAccessFailedException(string outcome)
    : Exception("Model access failed: " + outcome)
{
    public string Outcome { get; } = outcome;
}

/// <summary>A provider failure with a safe outcome and retry class.</summary>
public sealed class ModelProviderException(string outcome, bool retryable)
    : Exception("Model provider failure: " + outcome)
{
    public string Outcome { get; } = outcome;
    public bool Retryable { get; } = retryable;
}

/// <summary>The exact bytes of one provider call; retries reuse it as-is.</summary>
public sealed record ModelProviderRequest(
    string Deployment,
    string ApiVersion,
    string SystemInstruction,
    string UserPayloadJson,
    string OutputSchemaJson,
    int MaximumOutputBytes);

public sealed record ModelProviderResponse(
    string OutputJson,
    long InputTokens,
    long OutputTokens,
    string? ProviderRequestId,
    string FinishReason);

/// <summary>Executes one fixed-profile provider call.</summary>
public interface IModelProviderExecutor
{
    Task<ModelProviderResponse> ExecuteAsync(
        ModelProviderRequest request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The fixed-profile managed-identity model broker. The profile — endpoint,
/// deployment, API version, credential, region, retention, schema — comes
/// exclusively from verified policy and validated configuration; request
/// data is context only. Retries are bounded, limited to transient provider
/// outcomes, and never change request bytes or profile.
/// </summary>
public sealed class AzureOpenAiModelAccessBroker(
    Func<CancellationToken, Task<ModelAccessProfile>> profileResolver,
    IModelProviderExecutor executor,
    IModelReceiptSigner receiptSigner,
    TimeProvider timeProvider) : IModelAccessBroker
{
    private static readonly string[] RetryableOutcomes =
        ["timeout", "rate_limited", "provider_unavailable"];

    public async Task<ModelAccessResult> CompleteAsync(
        string subject,
        ModelAccessRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        ModelAccessProfile profile = await profileResolver(cancellationToken)
            .ConfigureAwait(false);
        if (request.Purpose != profile.ApprovedPurpose
            || request.ModelRole != profile.ApprovedModelRole
            || request.CanonicalOutputSchemaId != profile.CanonicalOutputSchemaId
            || request.RetentionMode != profile.RetentionMode
            || request.Consent.Region != profile.Region)
        {
            throw new ModelAccessFailedException("profile_mismatch");
        }
        CanonicalPrompt prompt = CanonicalPromptProjector.Project(request, profile);
        ModelProviderRequest providerRequest = new(
            profile.Deployment,
            profile.ApiVersion,
            prompt.SystemInstruction,
            prompt.UserPayloadJson,
            CanonicalModelOutputValidator.OutputSchemaJson,
            profile.MaximumOutputBytes);

        DateTimeOffset startedAt = timeProvider.GetUtcNow();
        int attempts = 0;
        ModelProviderResponse response;
        while (true)
        {
            attempts++;
            try
            {
                response = await executor
                    .ExecuteAsync(providerRequest, cancellationToken)
                    .ConfigureAwait(false);
                break;
            }
            catch (ModelProviderException failure)
                when (failure.Retryable
                    && RetryableOutcomes.Contains(failure.Outcome, StringComparer.Ordinal)
                    && attempts < profile.MaximumAttempts)
            {
                // Retry the identical request bytes under the same profile.
            }
            catch (ModelProviderException failure)
            {
                throw new ModelAccessFailedException(failure.Outcome);
            }
        }
        if (response.FinishReason != "stop")
        {
            throw new ModelAccessFailedException(
                response.FinishReason == "content_filter"
                    ? "content_filtered"
                    : "incomplete_output");
        }

        CanonicalModelOutput output = CanonicalModelOutputValidator.Validate(
            response.OutputJson,
            profile.MaximumOutputBytes);
        DateTimeOffset completedAt = timeProvider.GetUtcNow();
        UnsignedModelAccessResult unsigned = new(
            request.CanonicalOutputSchemaId,
            output.PayloadJson,
            output.PayloadHash,
            output.SchemaProjectionHash,
            CredentialBindingHash(profile, subject),
            new ModelAccessUsage(
                response.InputTokens,
                response.OutputTokens,
                profile.ComputeCostMicrounits(response.InputTokens, response.OutputTokens),
                profile.Currency),
            attempts - 1,
            [],
            response.ProviderRequestId,
            startedAt,
            completedAt,
            "succeeded");
        return await receiptSigner
            .SignAsync(subject, request, unsigned, cancellationToken)
            .ConfigureAwait(false);
    }

    private static string CredentialBindingHash(ModelAccessProfile profile, string subject) =>
        "sha256:" + Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(
            $"credential-binding:{profile.Deployment}:{profile.PriceProfileVersion}:{subject}")));
}

/// <summary>
/// Production executor over the managed-identity Azure OpenAI client.
/// Provider storage, hosted tools, background execution, and URL access are
/// all disabled; only the strict canonical JSON schema output is requested.
/// SDK failures map to safe outcomes with no body material.
/// </summary>
public sealed class AzureOpenAiProviderExecutor(AzureOpenAIClient client)
    : IModelProviderExecutor
{
    public async Task<ModelProviderResponse> ExecuteAsync(
        ModelProviderRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        ChatClient chat = client.GetChatClient(request.Deployment);
        ChatCompletionOptions options = new()
        {
            ResponseFormat = ChatResponseFormat.CreateJsonSchemaFormat(
                "sapphirus_bmad_method_help_proposal_v1",
                BinaryData.FromString(request.OutputSchemaJson),
                jsonSchemaIsStrict: true),
            StoredOutputEnabled = false,
            AllowParallelToolCalls = false,
        };
        try
        {
            ClientResult<ChatCompletion> completion = await chat.CompleteChatAsync(
                [
                    new SystemChatMessage(request.SystemInstruction),
                    new UserChatMessage(request.UserPayloadJson),
                ],
                options,
                cancellationToken).ConfigureAwait(false);
            ChatCompletion value = completion.Value;
            string finishReason = value.FinishReason switch
            {
                ChatFinishReason.Stop => "stop",
                ChatFinishReason.ContentFilter => "content_filter",
                _ => "incomplete",
            };
            return new ModelProviderResponse(
                value.Content.Count == 1 ? value.Content[0].Text : "",
                value.Usage?.InputTokenCount ?? 0,
                value.Usage?.OutputTokenCount ?? 0,
                value.Id,
                finishReason);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (ClientResultException failure)
        {
            throw failure.Status switch
            {
                408 => new ModelProviderException("timeout", retryable: true),
                429 => new ModelProviderException("rate_limited", retryable: true),
                403 => new ModelProviderException("quota_exhausted", retryable: false),
                >= 500 => new ModelProviderException(
                    "provider_unavailable",
                    retryable: true),
                _ => new ModelProviderException("provider_refusal", retryable: false),
            };
        }
    }
}
