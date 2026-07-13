using System.Text.Json.Serialization;
using System.Threading.RateLimiting;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Http.Json;
using Microsoft.IdentityModel.Tokens;
using Sapphirus.DesktopSupportApi;

WebApplicationBuilder builder = WebApplication.CreateBuilder(args);
SupportPlaneOptions options = builder.Configuration
    .GetSection(SupportPlaneOptions.SectionName)
    .Get<SupportPlaneOptions>() ?? new SupportPlaneOptions();
options.Validate(builder.Environment);

builder.Services.Configure<JsonOptions>(json =>
{
    json.SerializerOptions.PropertyNamingPolicy = System.Text.Json.JsonNamingPolicy.CamelCase;
    json.SerializerOptions.UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow;
});
builder.Services.AddProblemDetails();
builder.Services.AddExceptionHandler<SafeApiExceptionHandler>();
builder.Services
    .AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
    .AddJwtBearer(jwt =>
    {
        jwt.Authority = options.Authority;
        jwt.Audience = options.Audience;
        jwt.MapInboundClaims = false;
        jwt.RequireHttpsMetadata = true;
        jwt.TokenValidationParameters = new TokenValidationParameters
        {
            ValidateIssuer = true,
            ValidateAudience = true,
            ValidateLifetime = true,
            ValidateIssuerSigningKey = true,
            ClockSkew = TimeSpan.FromMinutes(2),
        };
    });
builder.Services.AddAuthorization(authorization =>
{
    foreach (string scope in DesktopScopes.All)
    {
        authorization.AddPolicy(scope, policy =>
        {
            policy.RequireAuthenticatedUser();
            policy.RequireAssertion(context =>
                ScopeAuthorization.IsApprovedDesktopPrincipal(context.User, scope, options));
        });
    }
});
builder.Services.AddRateLimiter(rateLimiter =>
{
    rateLimiter.RejectionStatusCode = StatusCodes.Status429TooManyRequests;
    rateLimiter.AddPolicy("desktop", httpContext =>
        RateLimitPartition.GetTokenBucketLimiter(
            ScopeAuthorization.RateLimitPartition(httpContext.User),
            _ => new TokenBucketRateLimiterOptions
            {
                TokenLimit = 120,
                TokensPerPeriod = 60,
                ReplenishmentPeriod = TimeSpan.FromMinutes(1),
                QueueLimit = 0,
                AutoReplenishment = true,
            }));
});

builder.Services.AddSingleton(options);
builder.Services.AddSingleton<IDeviceRegistry, MemoryDeviceRegistry>();
builder.Services.AddSingleton<IReceiptStore, MemoryReceiptStore>();
builder.Services.AddSingleton<IIdempotencyStore>(_ => new MemoryIdempotencyStore(
    options.IdempotencyMaximumEntries,
    TimeSpan.FromMinutes(options.IdempotencyRetentionMinutes),
    TimeProvider.System));
builder.Services.AddSingleton<ISignedPolicyService, DevelopmentSignedPolicyService>();
builder.Services.AddSingleton<IModelAccessBroker, DevelopmentModelAccessBroker>();
builder.Services.AddSingleton<IContextConsentVerifier, UnavailableContextConsentVerifier>();

WebApplication app = builder.Build();
app.UseExceptionHandler();
app.UseHttpsRedirection();
app.UseAuthentication();
app.UseRateLimiter();
app.UseAuthorization();

RouteGroupBuilder desktop = app.MapGroup("/desktop/v1")
    .RequireRateLimiting("desktop");

desktop.MapGet("/bootstrap", (SupportPlaneOptions configuration) => Results.Ok(new BootstrapResponse(
        "sapphirus.desktop-bootstrap.v1",
        configuration.Region,
        "1",
        "1",
        ["windows_local", "transient_no_store"],
        DateTimeOffset.UtcNow)))
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapPost("/devices/registrations", async (
        DeviceRegistrationRequest request,
        HttpContext context,
        IDeviceRegistry registry,
        IIdempotencyStore idempotency,
        CancellationToken cancellationToken) =>
    {
        string? key = RequestGuards.IdempotencyKey(context.Request);
        if (key is null)
        {
            return Results.BadRequest(RequestGuards.SafeProblem("idempotency_key_invalid"));
        }
        if (!RequestGuards.ValidateDeviceRegistration(request, out string errorCode))
        {
            return Results.BadRequest(RequestGuards.SafeProblem(errorCode));
        }
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        DeviceRegistrationResponse response = await idempotency.ExecuteAsync(
            subject,
            key,
            RequestGuards.Fingerprint(request),
            () => registry.RegisterAsync(subject, request, cancellationToken),
            cancellationToken);
        return Results.Ok(response);
    })
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapDelete("/devices/registrations/{registrationId}", async (
        string registrationId,
        HttpContext context,
        IDeviceRegistry registry,
        CancellationToken cancellationToken) =>
    {
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        if (!RequestGuards.IsRegistrationId(registrationId))
        {
            return Results.NotFound();
        }
        DeviceRevocationOutcome outcome = await registry.RevokeAsync(
            subject,
            registrationId,
            cancellationToken);
        return outcome == DeviceRevocationOutcome.Unknown
            ? Results.NotFound()
            : Results.NoContent();
    })
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapGet("/entitlements/current", (SupportPlaneOptions configuration) =>
    Results.Ok(new EntitlementSummaryResponse(
        "windows_local",
        ["local_runtime", "model_access"],
        false,
        [],
        configuration.ReleaseChannel)))
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapPost("/entitlements/leases", async (
        LeaseRequest request,
        HttpContext context,
        IDeviceRegistry registry,
        ISignedPolicyService signer,
        IIdempotencyStore idempotency,
        CancellationToken cancellationToken) =>
    {
        string? key = RequestGuards.IdempotencyKey(context.Request);
        if (key is null
            || request is null
            || !RequestGuards.IsRegistrationId(request.RegistrationId))
        {
            return Results.BadRequest(RequestGuards.SafeProblem("request_invalid"));
        }
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        RegisteredDevice? device = await registry.FindAsync(
            subject,
            request.RegistrationId,
            cancellationToken);
        if (device is null || !device.IsActive)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "device_registration_unavailable",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        SignedEntitlementLease lease = await idempotency.ExecuteAsync(
            subject,
            key,
            RequestGuards.Fingerprint(request),
            () => signer.CreateLeaseAsync(subject, request.RegistrationId, cancellationToken),
            cancellationToken);
        return Results.Ok(lease);
    })
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapGet("/policy/current", async (
        ISignedPolicyService signer,
        CancellationToken cancellationToken) =>
    Results.Ok(await signer.CurrentPolicyAsync(cancellationToken)))
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapPost("/model-access/calls", async (
        ModelAccessRequest request,
        HttpContext context,
        IDeviceRegistry registry,
        IModelAccessBroker broker,
        IContextConsentVerifier consentVerifier,
        IReceiptStore receipts,
        IIdempotencyStore idempotency,
        CancellationToken cancellationToken) =>
    {
        string? key = RequestGuards.IdempotencyKey(context.Request);
        if (key is null)
        {
            return Results.BadRequest(RequestGuards.SafeProblem("idempotency_key_invalid"));
        }
        if (!RequestGuards.ValidateModelRequest(
            request,
            out string errorCode,
            out string recomputedManifestHash))
        {
            return Results.BadRequest(RequestGuards.SafeProblem(errorCode));
        }
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        RegisteredDevice? device = await registry.FindAsync(
            subject,
            request.RegistrationId,
            cancellationToken);
        if (device is null || !device.IsActive)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "device_registration_unavailable",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        ContextConsentVerification consent = await consentVerifier.VerifyAsync(
            new ContextConsentVerificationRequest(
                subject,
                device,
                request,
                recomputedManifestHash),
            cancellationToken);
        if (consent == ContextConsentVerification.Unavailable)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "consent_binding_unavailable",
                    StatusCodes.Status503ServiceUnavailable),
                statusCode: StatusCodes.Status503ServiceUnavailable);
        }
        if (consent != ContextConsentVerification.Verified)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "consent_binding_rejected",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        ModelAccessResult result = await idempotency.ExecuteAsync(
            subject,
            key,
            RequestGuards.Fingerprint(request),
            async () =>
            {
                ModelAccessResult completed = await broker.CompleteAsync(
                    subject,
                    request,
                    cancellationToken);
                await receipts.AddAsync(subject, completed.Receipt, cancellationToken);
                return completed;
            },
            cancellationToken);
        return Results.Ok(result);
    })
    .RequireAuthorization(DesktopScopes.ModelInvoke)
    .DisableAntiforgery();

desktop.MapGet("/model-access/receipts/{receiptId}", async (
        string receiptId,
        HttpContext context,
        IReceiptStore receipts,
        CancellationToken cancellationToken) =>
    {
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        ModelAccessReceipt? receipt = await receipts.GetAsync(subject, receiptId, cancellationToken);
        return receipt is null ? Results.NotFound() : Results.Ok(receipt);
    })
    .RequireAuthorization(DesktopScopes.ModelInvoke);

desktop.MapGet("/releases/current", (SupportPlaneOptions configuration) =>
    Results.Ok(new ReleaseResponse(
        "sapphirus.desktop-release.v1",
        configuration.ReleaseChannel,
        "0.1.0-beta.1",
        "win-x64",
        "not-configured-in-development",
        "not-configured-in-development")))
    .RequireAuthorization(DesktopScopes.Access);

app.Run();

public partial class Program;
