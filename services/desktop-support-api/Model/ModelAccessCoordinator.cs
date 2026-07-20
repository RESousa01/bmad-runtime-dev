using Sapphirus.DesktopSupportApi.Observability;
using Sapphirus.DesktopSupportApi.Sql;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>One coordinated model-access outcome for the route to render.</summary>
public sealed record ModelAccessCoordinationResult(int StatusCode, object Body)
{
    public static ModelAccessCoordinationResult Problem(string code, int status) =>
        new(status, RequestGuards.SafeProblem(code, status));
}

/// <summary>
/// Coordinates one model-access call through explicit stages:
/// validate -> load active registration -> verify consent -> reserve
/// idempotency -> consume consent -> broker -> validate/sign (inside the
/// broker) -> transactional active-registration commit. Server-owned
/// deadlines and local revocation tokens bound latency, but the final SQL
/// concurrency checks are the cross-replica authority. No provider payload
/// is ever persisted; replays expose only the safe completion marker.
/// </summary>
public sealed class ModelAccessCoordinator(
    IDeviceRegistry registry,
    IModelAccessBroker broker,
    IContextConsentVerifier consentVerifier,
    IContextConsentConsumptionStore consentConsumptionStore,
    IModelCallIdempotencyStore idempotency,
    SupportPlaneOptions configuration,
    TimeProvider timeProvider,
    SupportPlaneTelemetry? telemetry = null)
{
    public async Task<ModelAccessCoordinationResult> ExecuteAsync(
        ModelAccessRequest request,
        string? idempotencyKey,
        string subject,
        CancellationToken cancellationToken)
    {
        // Stage: validate the request shape and its consent binding.
        if (idempotencyKey is null)
        {
            return ModelAccessCoordinationResult.Problem(
                "idempotency_key_invalid",
                StatusCodes.Status400BadRequest);
        }
        if (!RequestGuards.ValidateModelRequest(
            request,
            out string errorCode,
            out string recomputedManifestHash))
        {
            telemetry?.RecordAdmissionDenial(errorCode);
            return ModelAccessCoordinationResult.Problem(
                errorCode,
                StatusCodes.Status400BadRequest);
        }

        // Stage: load the active registration (subject partitioned).
        ActiveDeviceRegistration? device = await registry.FindActiveAsync(
            subject,
            request.RegistrationId,
            cancellationToken).ConfigureAwait(false);
        if (device is null)
        {
            return ModelAccessCoordinationResult.Problem(
                "device_registration_unavailable",
                StatusCodes.Status403Forbidden);
        }

        using CancellationTokenSource operationCancellation =
            CancellationTokenSource.CreateLinkedTokenSource(
                cancellationToken,
                device.RevocationToken);
        operationCancellation.CancelAfter(
            TimeSpan.FromSeconds(configuration.ConnectedOperationTimeoutSeconds));
        try
        {
            // Stage: verify the installation-key consent envelope.
            ContextConsentVerification consent = await CancellableOperation.WaitAsync(
                consentVerifier.VerifyAsync(
                    new ContextConsentVerificationRequest(
                        subject,
                        device.Registration,
                        request,
                        recomputedManifestHash),
                    operationCancellation.Token).AsTask(),
                operationCancellation.Token).ConfigureAwait(false);
            if (consent == ContextConsentVerification.Unavailable)
            {
                return ModelAccessCoordinationResult.Problem(
                    "consent_binding_unavailable",
                    StatusCodes.Status503ServiceUnavailable);
            }
            if (consent != ContextConsentVerification.Verified)
            {
                return ModelAccessCoordinationResult.Problem(
                    "consent_binding_rejected",
                    StatusCodes.Status403Forbidden);
            }

            // Stage: reserve idempotency; inside the reservation, consume
            // consent exactly once, then broker; the local commit callback
            // re-checks active registration transactionally.
            ModelCallIdempotencyResult execution = await idempotency.ExecuteAsync(
                subject,
                idempotencyKey,
                RequestGuards.Fingerprint(request),
                async token =>
                {
                    // Consent is consumed before provider egress; a provider
                    // failure never restores it.
                    ContextConsentConsumption consumption = await consentConsumptionStore
                        .ConsumeAsync(
                            new ContextConsentConsumptionRequest(
                                subject,
                                request.RegistrationId,
                                request.Consent.DecisionId,
                                request.RequestId,
                                request.Consent.ConsumptionHash,
                                timeProvider.GetUtcNow()),
                            token).ConfigureAwait(false);
                    if (consumption == ContextConsentConsumption.Unavailable)
                    {
                        throw new ContextConsentConsumptionUnavailableException();
                    }
                    if (consumption == ContextConsentConsumption.AlreadyConsumed)
                    {
                        throw new ContextConsentAlreadyConsumedException();
                    }
                    return await broker
                        .CompleteAsync(subject, request, token)
                        .ConfigureAwait(false);
                },
                (completed, token) => registry.CommitModelResultIfActiveAsync(
                    device,
                    request,
                    completed,
                    configuration.Region,
                    token),
                operationCancellation.Token).ConfigureAwait(false);

            // Stage: render. Replays expose only the safe completion marker.
            if (execution.PriorCompletion is ModelCallCompletionMarker completion)
            {
                telemetry?.RecordReplayObserved();
                return new ModelAccessCoordinationResult(
                    StatusCodes.Status409Conflict,
                    new
                    {
                        type = "https://errors.sapphirus.invalid/model_call_already_completed",
                        title = "The model call already completed.",
                        status = StatusCodes.Status409Conflict,
                        code = "model_call_already_completed",
                        receiptId = completion.ReceiptId,
                        requestHash = completion.RequestHash,
                        resultHash = completion.ResultHash,
                    });
            }
            telemetry?.RecordReceiptIssued();
            if (execution.Result is { Receipt: not null } fresh)
            {
                telemetry?.RecordUsage(
                    fresh.Receipt.Usage.InputTokens,
                    fresh.Receipt.Usage.OutputTokens,
                    fresh.Receipt.Usage.CostMicrounits,
                    fresh.Receipt.RetryCount,
                    request.ModelRole,
                    request.BudgetClass);
            }
            return new ModelAccessCoordinationResult(
                StatusCodes.Status200OK,
                execution.Result!);
        }
        catch (DeviceRegistrationRevokedException)
        {
            telemetry?.RecordRevocationObserved();
            return ModelAccessCoordinationResult.Problem(
                "device_registration_unavailable",
                StatusCodes.Status403Forbidden);
        }
        catch (ContextConsentAlreadyConsumedException)
        {
            return ModelAccessCoordinationResult.Problem(
                "consent_already_consumed",
                StatusCodes.Status409Conflict);
        }
        catch (ContextConsentConsumptionUnavailableException)
        {
            return ModelAccessCoordinationResult.Problem(
                "consent_consumption_unavailable",
                StatusCodes.Status503ServiceUnavailable);
        }
        catch (ModelCallIdempotencyUncertainException)
        {
            // Terminal uncertainty: a prior attempt may have completed after
            // provider acceptance. The same request authority can never fund
            // a new provider call; an operator must resolve the marker.
            return ModelAccessCoordinationResult.Problem(
                "model_call_uncertain",
                StatusCodes.Status409Conflict);
        }
        catch (ModelAccessFailedException failure)
        {
            return ModelAccessCoordinationResult.Problem(
                failure.Outcome,
                failure.Outcome switch
                {
                    "context_rejected" or "profile_mismatch" =>
                        StatusCodes.Status400BadRequest,
                    "rate_limited" => StatusCodes.Status429TooManyRequests,
                    "timeout" => StatusCodes.Status504GatewayTimeout,
                    "quota_exhausted" or "provider_unavailable" =>
                        StatusCodes.Status503ServiceUnavailable,
                    _ => StatusCodes.Status502BadGateway,
                });
        }
        catch (OperationCanceledException) when (
            device.RevocationToken.IsCancellationRequested
            && !cancellationToken.IsCancellationRequested)
        {
            return ModelAccessCoordinationResult.Problem(
                "device_registration_unavailable",
                StatusCodes.Status403Forbidden);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            return ModelAccessCoordinationResult.Problem(
                "connected_operation_timeout",
                StatusCodes.Status504GatewayTimeout);
        }
    }
}
