using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using Sapphirus.DesktopSupportApi.Observability;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>Where normalized events go. The HTTP writer is one implementation.</summary>
public interface IModelStreamSink
{
    ValueTask WriteAsync(IModelStreamEvent streamEvent, CancellationToken cancellationToken);
}

/// <summary>
/// One fully resolved streaming call. Everything provider-facing here came
/// from <see cref="ResolvedModelRoute"/>, which is server-only; the desktop
/// contributed a profile identifier, a catalog epoch, and consent.
/// </summary>
public sealed record ModelStreamCall(
    string Subject,
    ModelReceiptAuthorization Authorization,
    string RunId,
    ResolvedModelRoute Route,
    string DeploymentInventoryHash,
    string CredentialMechanismId,
    ModelProviderRequest ProviderRequest,
    string ModelRole,
    string BudgetClass,
    string? RequestedVariantId);

/// <summary>The evidence one run produced, terminal event included.</summary>
public sealed record ModelStreamRunResult(
    IModelStreamEvent Terminal,
    int EventCount,
    int FinalSequence,
    ModelAccessReceiptV2? Receipt,
    string ProblemCode)
{
    public bool Succeeded => Receipt is not null;
}

/// <summary>
/// Drives one streaming model call end to end: open the Responses stream,
/// normalize it, forward safe deltas promptly, and mint exactly one terminal
/// event.
///
/// <para>
/// Three rules are load bearing. Exactly one terminal event is emitted, ever —
/// the normalizer refuses a second. A cancelled run is never retried and its
/// cancellation reaches the provider by tearing the session down, so the plane
/// stops paying for tokens the user will not see. And a retry is attempted
/// only while nothing has been forwarded: retrying after the first delta would
/// replay text the desktop already rendered.
/// </para>
/// <para>
/// A transcript the normalizer cannot represent ends the run as
/// <c>output_schema_violation</c> on the wire — the closed stream vocabulary
/// has no "invalid stream" member — while the support-plane problem code and
/// the metric both record <c>model_stream_invalid</c>, so the desktop gets a
/// code it can branch on and an operator gets the truthful one.
/// </para>
/// </summary>
public sealed class ModelStreamingCoordinator(
    IModelStreamTransport transport,
    IModelReceiptSignerV2 receiptSigner,
    TimeProvider timeProvider,
    ModelStreamLimits limits,
    string credentialMechanismId,
    SupportPlaneTelemetry? telemetry = null,
    string credentialVersionId = "v1",
    string projectEndpointHash = "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    string applicationRevision = "unset")
{
    /// <summary>
    /// How this coordinator's provider calls are authenticated. It is folded
    /// into the receipt's credential-binding hash, never logged, and never the
    /// credential itself.
    /// </summary>
    public string CredentialMechanismId { get; } = credentialMechanismId;

    /// <summary>
    /// Non-secret credential version label folded into the receipt's
    /// credential-binding hash alongside <see cref="CredentialMechanismId"/>.
    /// </summary>
    public string CredentialVersionId { get; } = credentialVersionId;

    /// <summary>
    /// Non-secret hash of the Foundry project endpoint folded into the
    /// receipt's credential-binding hash.
    /// </summary>
    public string ProjectEndpointHash { get; } = projectEndpointHash;

    /// <summary>
    /// Non-secret application revision folded into the receipt's
    /// credential-binding hash.
    /// </summary>
    public string ApplicationRevision { get; } = applicationRevision;

    /// <summary>Reviewed prose. Never copied from a provider response.</summary>
    internal const string TranscriptRejectedDetail =
        "The provider transcript could not be normalized safely.";

    internal const string ProviderUnavailableDetail =
        "The model provider did not complete the request.";

    public async Task<ModelStreamRunResult> RunAsync(
        ModelStreamCall call,
        IModelStreamSink sink,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(call);
        ArgumentNullException.ThrowIfNull(sink);

        FoundryModelStreamNormalizer normalizer = new(
            call.Authorization.RequestId,
            call.RunId,
            call.Route.ProfileId,
            call.Route.VariantId,
            timeProvider,
            limits,
            reported => ModelUsageProjector.Project(reported, call.Route.PriceProfile));
        DateTimeOffset startedAt = timeProvider.GetUtcNow();
        long startedTicks = Stopwatch.GetTimestamp();
        int attempts = 0;
        int maximumAttempts = Math.Max(1, call.Route.RetryPolicy.MaximumAttempts);

        while (true)
        {
            attempts++;
            FoundryStreamSession? session = null;
            try
            {
                session = await transport
                    .OpenAsync(call.ProviderRequest, cancellationToken)
                    .ConfigureAwait(false);
                return await ConsumeAsync(
                        call,
                        sink,
                        normalizer,
                        session,
                        startedAt,
                        startedTicks,
                        attempts - 1,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (ModelProviderException failure)
                when (failure.Retryable
                    && normalizer.LastSequence == 0
                    && attempts < maximumAttempts)
            {
                // Nothing has been forwarded yet, so re-dispatching the exact
                // same request bytes on the exact same route cannot duplicate
                // anything the desktop has already rendered.
                telemetry?.RecordProviderStatusClass("retry");
            }
            catch (ModelProviderException failure)
            {
                return await TerminateAsync(
                        sink,
                        normalizer,
                        ProviderFailureTerminal(normalizer, failure.Outcome),
                        SupportProblemCodes.ForProviderOutcome(failure.Outcome).Code,
                        startedTicks,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (ModelStreamInvalidException)
            {
                return await TerminateAsync(
                        sink,
                        normalizer,
                        normalizer.Fail(
                            ModelStreamFailureCodes.OutputSchemaViolation,
                            retryable: false,
                            TranscriptRejectedDetail),
                        SupportProblemCodes.ModelStreamInvalid,
                        startedTicks,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return await CancelAsync(
                        sink,
                        normalizer,
                        startedTicks,
                        cancellationToken)
                    .ConfigureAwait(false);
            }
            finally
            {
                if (session is not null)
                {
                    // Disposing tears the provider connection down. That is
                    // how a desktop disconnect stops the model mid-answer.
                    await session.DisposeAsync().ConfigureAwait(false);
                }
            }
        }
    }

    private async Task<ModelStreamRunResult> ConsumeAsync(
        ModelStreamCall call,
        IModelStreamSink sink,
        FoundryModelStreamNormalizer normalizer,
        FoundryStreamSession session,
        DateTimeOffset startedAt,
        long startedTicks,
        int retryCount,
        CancellationToken cancellationToken)
    {
        ModelStreamProviderOutcome outcome = ModelStreamProviderOutcome.None;
        string outcomeCode = "";
        bool retryableOutcome = false;

        await foreach (FoundrySseFrame frame in FoundrySseReader
            .ReadAsync(session.Body, limits, cancellationToken)
            .ConfigureAwait(false))
        {
            ModelStreamStep step = normalizer.Accept(frame);
            foreach (IModelStreamEvent forwarded in step.Events)
            {
                await sink.WriteAsync(forwarded, cancellationToken).ConfigureAwait(false);
            }
            if (step.Outcome == ModelStreamProviderOutcome.None)
            {
                continue;
            }
            outcome = step.Outcome;
            outcomeCode = step.Code;
            retryableOutcome = step.Retryable;
            break;
        }

        if (outcome == ModelStreamProviderOutcome.None)
        {
            // The body ended without a terminal frame. A run whose ending the
            // plane never saw cannot be reported as finished.
            throw new ModelStreamInvalidException(
                ModelStreamInvalidReasons.MissingTerminal);
        }
        if (outcome == ModelStreamProviderOutcome.Refused)
        {
            return await TerminateAsync(
                    sink,
                    normalizer,
                    normalizer.Refuse(outcomeCode),
                    outcomeCode == ModelStreamRefusalCodes.ContentFiltered
                        ? SupportProblemCodes.ModelContentFiltered
                        : SupportProblemCodes.ModelRefused,
                    startedTicks,
                    cancellationToken)
                .ConfigureAwait(false);
        }
        if (outcome == ModelStreamProviderOutcome.Failed)
        {
            return await TerminateAsync(
                    sink,
                    normalizer,
                    normalizer.Fail(outcomeCode, retryableOutcome, ProviderUnavailableDetail),
                    SupportProblemCodes.FoundryProjectUnavailable,
                    startedTicks,
                    cancellationToken)
                .ConfigureAwait(false);
        }

        return await CompleteAsync(
                call,
                sink,
                normalizer,
                session,
                startedAt,
                startedTicks,
                retryCount,
                cancellationToken)
            .ConfigureAwait(false);
    }

    private async Task<ModelStreamRunResult> CompleteAsync(
        ModelStreamCall call,
        IModelStreamSink sink,
        FoundryModelStreamNormalizer normalizer,
        FoundryStreamSession session,
        DateTimeOffset startedAt,
        long startedTicks,
        int retryCount,
        CancellationToken cancellationToken)
    {
        string transcript = normalizer.Transcript;
        if (transcript.Length == 0)
        {
            // A completed run with no visible answer is not a success.
            return await TerminateAsync(
                    sink,
                    normalizer,
                    normalizer.Fail(
                        ModelStreamFailureCodes.OutputSchemaViolation,
                        retryable: false,
                        TranscriptRejectedDetail),
                    SupportProblemCodes.ModelOutputSchemaViolation,
                    startedTicks,
                    cancellationToken)
                .ConfigureAwait(false);
        }

        DateTimeOffset completedAt = timeProvider.GetUtcNow();
        long outputBytes = Encoding.UTF8.GetByteCount(transcript);
        ResolvedReceiptBinding binding = ModelReceiptBinding.Create(
            call.Route,
            call.DeploymentInventoryHash,
            call.CredentialMechanismId,
            call.Subject,
            CredentialVersionId,
            ProjectEndpointHash,
            ApplicationRevision);

        // The lane is minted before the terminal event, so its final sequence
        // is the sequence that terminal event will carry.
        int finalSequence = normalizer.LastSequence + 1;
        UnsignedModelAccessResultV2 unsigned = new(
            call.RunId,
            new StreamingChatLane(finalSequence, finalSequence),
            transcript,
            HashText(transcript),
            binding,
            call.Route.ProfileId,
            call.RequestedVariantId,
            normalizer.ReportedModel,
            ModelUsageProjector.Project(normalizer.ReportedUsage, call.Route.PriceProfile),
            outputBytes,
            retryCount,
            [],
            normalizer.ProviderResponseId ?? session.ProviderRequestId,
            startedAt,
            completedAt,
            ModelTerminalStatuses.Succeeded);

        ModelAccessReceiptV2 receipt = await receiptSigner
            .SignAsync(call.Subject, call.Authorization, unsigned, cancellationToken)
            .ConfigureAwait(false);
        if (!ModelResultGuards.ValidateReceiptV2(
            call.Authorization,
            receipt,
            binding,
            call.Route.Region,
            unsigned.PayloadHash,
            outputBytes))
        {
            throw new InvalidOperationException(
                "The signed streaming receipt was not bound to the authorized request.");
        }

        ModelStreamCompletedEvent terminal = normalizer.Complete(new ModelStreamReceiptSummary(
            receipt.ReceiptId,
            receipt.ReceiptHash,
            receipt.TerminalStatus,
            binding.ResolvedProfileId,
            binding.ResolvedVariantId,
            outputBytes,
            completedAt));
        await sink.WriteAsync(terminal, cancellationToken).ConfigureAwait(false);
        RecordTerminal(terminal, startedTicks);
        telemetry?.RecordReceiptOutcome("signed");
        telemetry?.RecordUsageV2(
            receipt.Usage,
            retryCount,
            call.ModelRole,
            call.BudgetClass);
        return new ModelStreamRunResult(
            terminal,
            normalizer.EventCount,
            terminal.Sequence,
            receipt,
            "");
    }

    /// <summary>
    /// Emits the one cancelled terminal event. A cancelled run is never
    /// retried and never produces a success receipt.
    /// </summary>
    private async Task<ModelStreamRunResult> CancelAsync(
        IModelStreamSink sink,
        FoundryModelStreamNormalizer normalizer,
        long startedTicks,
        CancellationToken cancellationToken)
    {
        ModelStreamCancelledEvent terminal =
            normalizer.Cancel(ModelStreamCancellationSources.User);
        telemetry?.RecordStreamCancellation(ModelStreamCancellationSources.User);
        // The sink write runs detached from the cancelled token: the desktop
        // may still be listening even when the provider read was abandoned,
        // and a cancellation the client never learns about is a hung run.
        await WriteQuietlyAsync(sink, terminal).ConfigureAwait(false);
        RecordTerminal(terminal, startedTicks);
        telemetry?.RecordReceiptOutcome("withheld");
        _ = cancellationToken;
        return new ModelStreamRunResult(
            terminal,
            normalizer.EventCount,
            terminal.Sequence,
            null,
            SupportProblemCodes.ConnectedOperationTimeout);
    }

    private async Task<ModelStreamRunResult> TerminateAsync(
        IModelStreamSink sink,
        FoundryModelStreamNormalizer normalizer,
        IModelStreamEvent terminal,
        string problemCode,
        long startedTicks,
        CancellationToken cancellationToken)
    {
        await WriteQuietlyAsync(sink, terminal, cancellationToken).ConfigureAwait(false);
        RecordTerminal(terminal, startedTicks);
        telemetry?.RecordReceiptOutcome("withheld");
        return new ModelStreamRunResult(
            terminal,
            normalizer.EventCount,
            terminal.Sequence,
            null,
            problemCode);
    }

    private IModelStreamEvent ProviderFailureTerminal(
        FoundryModelStreamNormalizer normalizer,
        string outcome) => outcome switch
        {
            ModelProviderOutcomes.ContentFiltered =>
                normalizer.Refuse(ModelStreamRefusalCodes.ContentFiltered),
            ModelProviderOutcomes.ProviderRefusal =>
                normalizer.Refuse(ModelStreamRefusalCodes.PolicyBlocked),
            ModelProviderOutcomes.CapabilityUnsupported =>
                normalizer.Refuse(ModelStreamRefusalCodes.ToolNotPermitted),
            ModelProviderOutcomes.ContextLimit =>
                normalizer.Refuse(ModelStreamRefusalCodes.ContextRejected),
            ModelProviderOutcomes.AuthenticationFailed =>
                normalizer.Refuse(ModelStreamRefusalCodes.NotEntitled),
            ModelProviderOutcomes.DeploymentMissing =>
                normalizer.Fail(ModelStreamFailureCodes.ProfileUnavailable, false),
            ModelProviderOutcomes.RateLimited =>
                normalizer.Fail(ModelStreamFailureCodes.RateLimited, true),
            ModelProviderOutcomes.QuotaExhausted =>
                normalizer.Fail(ModelStreamFailureCodes.BudgetExhausted, false),
            ModelProviderOutcomes.Timeout =>
                normalizer.Fail(ModelStreamFailureCodes.ProviderTimeout, true),
            ModelProviderOutcomes.StreamInvalid => normalizer.Fail(
                ModelStreamFailureCodes.OutputSchemaViolation,
                false,
                TranscriptRejectedDetail),
            _ => normalizer.Fail(
                ModelStreamFailureCodes.ProviderUnavailable,
                true,
                ProviderUnavailableDetail),
        };

    private void RecordTerminal(IModelStreamEvent terminal, long startedTicks) =>
        telemetry?.RecordStreamRun(
            terminal.Type,
            Stopwatch.GetElapsedTime(startedTicks).TotalMilliseconds);

    /// <summary>
    /// Writes a terminal event without letting a broken client socket mask the
    /// outcome the run actually reached.
    /// </summary>
    private static async ValueTask WriteQuietlyAsync(
        IModelStreamSink sink,
        IModelStreamEvent terminal,
        CancellationToken cancellationToken = default)
    {
        try
        {
            await sink.WriteAsync(terminal, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            // The desktop is already gone.
        }
        catch (IOException)
        {
            // The desktop is already gone.
        }
    }

    private static string HashText(string value) => "sha256:" + Convert.ToHexStringLower(
        SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
