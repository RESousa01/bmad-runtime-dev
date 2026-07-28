using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.AspNetCore.Mvc;
using Sapphirus.DesktopSupportApi.Observability;
using Sapphirus.DesktopSupportApi.Policy;

namespace Sapphirus.DesktopSupportApi.Model;

/// <summary>
/// Verifies the version two consent envelope for a streaming request.
///
/// It is a separate seam from the version one verifier because a version two
/// decision has a different signed shape: no deployment hash, and a profile,
/// catalog epoch, variant, request mode, and context snapshot instead.
/// </summary>
public interface IModelStreamConsentVerifier
{
    ValueTask<ContextConsentVerification> VerifyAsync(
        string subject,
        RegisteredDevice device,
        ModelAccessRequestV2 request,
        CancellationToken cancellationToken);
}

/// <summary>
/// The fail-closed default, mirroring
/// <see cref="UnavailableContextConsentVerifier"/>.
///
/// Installation-key verification over the version two envelope has no provider
/// yet, and a model-egress route must never treat "cannot verify" as "verified".
/// The route therefore refuses every request until a real verifier is composed.
/// </summary>
public sealed class UnavailableModelStreamConsentVerifier : IModelStreamConsentVerifier
{
    public ValueTask<ContextConsentVerification> VerifyAsync(
        string subject,
        RegisteredDevice device,
        ModelAccessRequestV2 request,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return ValueTask.FromResult(ContextConsentVerification.Unavailable);
    }
}

/// <summary>The server-side route decision for one streaming request.</summary>
public interface IModelStreamRouteResolver
{
    ValueTask<ModelRouteResolution> ResolveAsync(
        string subject,
        ModelAccessRequestV2 request,
        CancellationToken cancellationToken);

    /// <summary>
    /// The inventory the route was verified against. It is recorded on the
    /// receipt so an auditor can tell which snapshot the deployment existed in.
    /// </summary>
    string DeploymentInventoryHash { get; }
}

/// <summary>
/// Composes the effective catalog, the entitlement snapshot, and the verified
/// policy into one resolved route. It is the only place a desktop-supplied
/// profile identifier becomes a deployment.
/// </summary>
public sealed class EffectiveCatalogStreamRouteResolver(
    EffectiveModelCatalogService catalogService,
    IModelEntitlementSource entitlementSource,
    IModelCatalogPolicySource policySource,
    string region,
    SupportPlaneTelemetry? telemetry = null) : IModelStreamRouteResolver
{
    private string _deploymentInventoryHash = "";

    public string DeploymentInventoryHash => _deploymentInventoryHash;

    public async ValueTask<ModelRouteResolution> ResolveAsync(
        string subject,
        ModelAccessRequestV2 request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        ModelCatalogResult catalog = await catalogService
            .BuildAsync(subject, request.ModelProfileId, cancellationToken)
            .ConfigureAwait(false);
        if (!catalog.IsSuccess)
        {
            telemetry?.RecordRouteResolutionFailure(ModelRouteErrorCodes.CatalogStale);
            return new ModelRouteResolution(null, ModelRouteErrorCodes.CatalogStale);
        }
        _deploymentInventoryHash = catalog.Catalog!.Projection.DeploymentSnapshotHash;
        telemetry?.RecordEffectiveProfileCount(catalog.Catalog.Entries.Count);

        ModelEntitlementSnapshot entitlement = await entitlementSource
            .GetAsync(subject, cancellationToken)
            .ConfigureAwait(false);
        PolicySnapshot policy = await policySource
            .GetSnapshotAsync(cancellationToken)
            .ConfigureAwait(false);

        ModelRouteResolution resolution = ModelRouteResolver.Resolve(
            ToRouteRequest(request),
            catalog.Catalog,
            entitlement,
            policy,
            region);
        if (!resolution.IsSuccess)
        {
            telemetry?.RecordRouteResolutionFailure(resolution.ErrorCode);
        }
        return resolution;
    }

    private static ModelRouteRequest ToRouteRequest(ModelAccessRequestV2 request) => new(
        request.RequestId,
        request.RegistrationId,
        request.Purpose,
        request.ModelRole,
        request.RequestMode,
        request.ModelProfileId,
        request.RequestedVariantId,
        request.CatalogVersion,
        request.ModelProfileHash,
        request.ModelCapabilityHash,
        request.CanonicalOutputSchemaId,
        request.CanonicalOutputSchemaHash,
        request.RetentionMode,
        request.BudgetClass,
        UsesImageInput: false,
        UsesPdfInput: false,
        AttachmentBytes: 0,
        request.Tools.Length,
        EstimatedInputTokens: EstimateInputTokens(request),
        RequestedOutputTokens: 0,
        new ModelRouteConsent(
            request.Consent.ModelProfileId,
            request.Consent.RequestedVariantId,
            request.Consent.CatalogVersion,
            request.Consent.RequestMode,
            request.Consent.Purpose,
            request.Consent.ModelRole,
            request.Consent.CanonicalOutputSchemaId,
            request.Consent.CanonicalOutputSchemaHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.Region,
            request.Consent.RetentionMode));

    /// <summary>
    /// A deliberately conservative four-bytes-per-token estimate. It exists so
    /// the context budget is checked before egress rather than after; the
    /// authoritative count comes back from the provider on the receipt.
    /// </summary>
    private static int EstimateInputTokens(ModelAccessRequestV2 request) =>
        (int)Math.Min(int.MaxValue, (request.TotalContextBytes + 3) / 4);
}

/// <summary>
/// <c>POST /desktop/v1/model-access/streams</c>.
///
/// <para>
/// This is the one fixed, reviewed streaming route. It is a constant here and
/// in the generated contracts; the renderer never supplies a URL, and there is
/// no path parameter, query key, or header through which a caller could reach a
/// different provider surface.
/// </para>
/// <para>
/// It carries the same authorization posture as the existing model-access call:
/// the approved desktop client, the model-invocation scope, and the desktop
/// rate-limit partition. Seeing the catalog needs only the read scope; spending
/// tokens needs this one.
/// </para>
/// <para>
/// The response body is a server-sent event stream of normalized
/// <c>ModelStreamEvent</c> values and nothing else. A failure before the stream
/// opens is an RFC 7807 problem document from the reviewed
/// <see cref="SupportProblemCodes"/> vocabulary; a failure after it opens is a
/// terminal stream event, because a half-written stream cannot become a status
/// code.
/// </para>
/// </summary>
public static class ModelStreamEndpoint
{
    /// <summary>The decided fixed route, relative to the /desktop/v1 group.</summary>
    public const string RoutePattern = "/model-access/streams";

    /// <summary>The full route, as encoded in the reviewed route allowlist.</summary>
    public const string FullRoute = "/desktop/v1" + RoutePattern;

    /// <summary>The bootstrap capability that advertises this route.</summary>
    public const string BootstrapCapability = "model_stream";

    public const string EventStreamContentType = "text/event-stream";

    /// <summary>Named so the desktop can branch before parsing the payload.</summary>
    public const string EventNameField = "event";

    private static readonly JsonSerializerOptions EventJson = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    public static RouteHandlerBuilder MapModelStream(this RouteGroupBuilder group)
    {
        ArgumentNullException.ThrowIfNull(group);
        return group.MapPost(RoutePattern, HandleAsync)
            .RequireAuthorization(DesktopScopes.ModelInvoke)
            .DisableAntiforgery();
    }

    internal static async Task<IResult> HandleAsync(
        ModelAccessRequestV2 request,
        HttpContext context,
        [FromServices] IDeviceRegistry registry,
        [FromServices] IModelStreamConsentVerifier consentVerifier,
        [FromServices] IModelStreamRouteResolver? routeResolver,
        [FromServices] ModelStreamingCoordinator? coordinator,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(context);

        if (!RequestGuards.ValidateStreamRequest(request, out string errorCode))
        {
            return Problem(errorCode, StatusCodes.Status400BadRequest);
        }
        if (routeResolver is null || coordinator is null)
        {
            // The Foundry adapter is not composed in this environment. Saying
            // so is more useful than a five hundred, and it is the truth: the
            // project surface is unavailable to this process.
            return Problem(
                SupportProblemCodes.FoundryProjectUnavailable,
                StatusCodes.Status503ServiceUnavailable);
        }

        string subject = ScopeAuthorization.SubjectPartition(context.User);
        ActiveDeviceRegistration? device = await registry
            .FindActiveAsync(subject, request.RegistrationId, cancellationToken)
            .ConfigureAwait(false);
        if (device is null)
        {
            return Problem(
                "device_registration_unavailable",
                StatusCodes.Status403Forbidden);
        }

        ContextConsentVerification consent = await consentVerifier
            .VerifyAsync(subject, device.Registration, request, cancellationToken)
            .ConfigureAwait(false);
        if (consent == ContextConsentVerification.Unavailable)
        {
            return Problem(
                "consent_binding_unavailable",
                StatusCodes.Status503ServiceUnavailable);
        }
        if (consent != ContextConsentVerification.Verified)
        {
            return Problem("consent_binding_rejected", StatusCodes.Status403Forbidden);
        }

        ModelRouteResolution resolution = await routeResolver
            .ResolveAsync(subject, request, cancellationToken)
            .ConfigureAwait(false);
        if (!resolution.IsSuccess)
        {
            (string code, int status) = ForRouteError(resolution.ErrorCode);
            return Problem(code, status);
        }

        ResolvedModelRoute route = resolution.Route!;
        ModelStreamCall call = new(
            subject,
            Authorize(request),
            ContractIds.FromEntropy("run", RandomNumberGenerator.GetBytes(16)),
            route,
            routeResolver.DeploymentInventoryHash,
            coordinator.CredentialMechanismId,
            BuildProviderRequest(request, route),
            request.ModelRole,
            request.BudgetClass,
            request.RequestedVariantId);

        return new ModelStreamResult(call, coordinator, device);
    }

    /// <summary>
    /// Lifts the authorization facts a receipt binds itself to. The request
    /// fingerprint is taken over the whole document, so any edit between
    /// admission and receipt changes the hash.
    /// </summary>
    internal static ModelReceiptAuthorization Authorize(ModelAccessRequestV2 request) => new(
        request.RequestId,
        RequestGuards.Fingerprint(request),
        request.RegistrationId,
        request.Consent.TenantHash,
        request.Consent.SubjectHash,
        request.LocalEgressManifestHash,
        request.Consent.InvocationBindingHash,
        request.Consent.ConsumptionHash,
        request.Consent.ConsentEnvelopeHash,
        request.Consent.ConsentDisclosureHash,
        request.RetentionMode,
        request.TotalContextBytes);

    /// <summary>
    /// Projects the request onto the provider request. The deployment, the
    /// operating point, and the output budget all come from the resolved route;
    /// the request contributes only reviewed context and declared tool names.
    /// </summary>
    internal static ModelProviderRequest BuildProviderRequest(
        ModelAccessRequestV2 request,
        ResolvedModelRoute route)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(route);
        return new ModelProviderRequest(
            route.DeploymentName,
            ApiVersion: "",
            SystemInstruction: StreamingSystemInstruction,
            UserPayloadJson: ProjectContext(request),
            OutputSchemaJson: "",
            MaximumOutputBytes: route.MaxOutputBytes,
            ProfileId: route.ProfileId,
            VariantId: route.VariantId,
            RequestMode: ModelRequestModes.StreamingChat,
            ReasoningEffort: route.ReasoningEffort,
            Verbosity: route.Verbosity,
            MaximumOutputTokens: route.MaxOutputTokens,
            Tools: []);
    }

    /// <summary>
    /// Reviewed prose. It is a server constant, never assembled from request
    /// data, so no caller can rewrite the model's operating instructions.
    /// </summary>
    internal const string StreamingSystemInstruction =
        "You are the Sapphirus desktop assistant. Use only the provided "
        + "context items. Answer in plain text. Do not browse, do not call "
        + "tools that were not declared, and do not reference external "
        + "resources.";

    /// <summary>
    /// The canonical context projection. It writes a closed key set, so a local
    /// path, an Azure resource identifier, a consent document, or unreviewed
    /// workspace content has no property to arrive in.
    /// </summary>
    internal static string ProjectContext(ModelAccessRequestV2 request)
    {
        ArrayBufferWriter<byte> buffer = new();
        using (Utf8JsonWriter writer = new(buffer, new JsonWriterOptions
        {
            Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        }))
        {
            writer.WriteStartObject();
            writer.WritePropertyName("items");
            writer.WriteStartArray();
            foreach (ModelContextItem item in request.Items)
            {
                writer.WriteStartObject();
                writer.WriteString("content", item.Content);
                writer.WriteString("language", item.Language);
                writer.WriteString("relativeLabel", item.RelativeLabel);
                writer.WriteString("semanticRole", item.SemanticRole);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteString("purpose", request.Purpose);
            writer.WriteEndObject();
            writer.Flush();
        }
        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }

    internal static (string Code, int Status) ForRouteError(string errorCode) => errorCode switch
    {
        ModelRouteErrorCodes.CatalogStale =>
            (SupportProblemCodes.CatalogStale, StatusCodes.Status503ServiceUnavailable),
        ModelRouteErrorCodes.ProfileUnavailable =>
            (SupportProblemCodes.ProfileUnavailable, StatusCodes.Status503ServiceUnavailable),
        ModelRouteErrorCodes.ProfileNotEntitled =>
            (SupportProblemCodes.ProfileNotEntitled, StatusCodes.Status403Forbidden),
        ModelRouteErrorCodes.DeploymentMissing =>
            (SupportProblemCodes.ModelDeploymentMissing,
                StatusCodes.Status503ServiceUnavailable),
        ModelRouteErrorCodes.ContextLimitExceeded =>
            (SupportProblemCodes.ModelContextLimitExceeded,
                StatusCodes.Status413PayloadTooLarge),
        _ => (SupportProblemCodes.ModelCapabilityMismatch, StatusCodes.Status400BadRequest),
    };

    private static IResult Problem(string code, int status) =>
        Results.Json(RequestGuards.SafeProblem(code, status), statusCode: status);

    /// <summary>
    /// Serializes one event as a named server-sent event frame. Serialization
    /// goes through the concrete runtime type, so every closed variant emits
    /// exactly its own fields and nothing widens by inheritance.
    /// </summary>
    internal static byte[] RenderFrame(IModelStreamEvent streamEvent)
    {
        ArgumentNullException.ThrowIfNull(streamEvent);
        string payload = JsonSerializer.Serialize(
            streamEvent,
            streamEvent.GetType(),
            EventJson);
        return Encoding.UTF8.GetBytes(
            $"{EventNameField}: {streamEvent.Type}\ndata: {payload}\n\n");
    }

    /// <summary>
    /// Streams normalized events to the desktop. The request-aborted token is
    /// the desktop disconnect signal; it reaches the provider through the
    /// coordinator, which tears the Responses call down and emits exactly one
    /// cancelled terminal event.
    /// </summary>
    private sealed class ModelStreamResult(
        ModelStreamCall call,
        ModelStreamingCoordinator coordinator,
        ActiveDeviceRegistration device) : IResult
    {
        public async Task ExecuteAsync(HttpContext httpContext)
        {
            ArgumentNullException.ThrowIfNull(httpContext);
            httpContext.Response.StatusCode = StatusCodes.Status200OK;
            httpContext.Response.ContentType = EventStreamContentType;
            httpContext.Response.Headers.CacheControl = "no-store";
            // The stream is per-subject and must never be buffered by an
            // intermediary that would deliver it as one document.
            httpContext.Response.Headers["X-Accel-Buffering"] = "no";

            using CancellationTokenSource operationCancellation =
                CancellationTokenSource.CreateLinkedTokenSource(
                    httpContext.RequestAborted,
                    device.RevocationToken);
            HttpResponseStreamSink sink = new(httpContext.Response);
            await coordinator
                .RunAsync(call, sink, operationCancellation.Token)
                .ConfigureAwait(false);
        }
    }

    /// <summary>Writes one frame and flushes it, so deltas arrive promptly.</summary>
    private sealed class HttpResponseStreamSink(HttpResponse response) : IModelStreamSink
    {
        public async ValueTask WriteAsync(
            IModelStreamEvent streamEvent,
            CancellationToken cancellationToken)
        {
            await response.Body
                .WriteAsync(RenderFrame(streamEvent), cancellationToken)
                .ConfigureAwait(false);
            await response.Body.FlushAsync(cancellationToken).ConfigureAwait(false);
        }
    }
}
