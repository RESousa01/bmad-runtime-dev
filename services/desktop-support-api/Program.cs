using System.Text.Json.Serialization;
using System.Threading.RateLimiting;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Http.Json;
using Microsoft.IdentityModel.Tokens;
using Sapphirus.DesktopSupportApi;
using Sapphirus.DesktopSupportApi.Configuration;

WebApplicationBuilder builder = WebApplication.CreateBuilder(args);
SupportPlaneOptions options = builder.Configuration
    .GetSection(SupportPlaneOptions.SectionName)
    .Get<SupportPlaneOptions>() ?? new SupportPlaneOptions();
options.Validate(builder.Environment);

builder.Services.Configure<JsonOptions>(json =>
{
    json.SerializerOptions.AllowDuplicateProperties = false;
    json.SerializerOptions.PropertyNameCaseInsensitive = false;
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
builder.Services.AddSingleton(provider =>
    new Sapphirus.DesktopSupportApi.Model.ModelAccessCoordinator(
        provider.GetRequiredService<IDeviceRegistry>(),
        provider.GetRequiredService<IModelAccessBroker>(),
        provider.GetRequiredService<IContextConsentVerifier>(),
        provider.GetRequiredService<IContextConsentConsumptionStore>(),
        provider.GetRequiredService<IModelCallIdempotencyStore>(),
        options,
        TimeProvider.System));
if (builder.Environment.IsDevelopment())
{
    builder.Services.AddSingleton<IDeviceRegistry, MemoryDeviceRegistry>();
    builder.Services.AddSingleton<IIdempotencyStore>(_ => new MemoryIdempotencyStore(
        options.IdempotencyMaximumEntries,
        TimeSpan.FromMinutes(options.IdempotencyRetentionMinutes),
        TimeProvider.System));
    builder.Services.AddSingleton<IModelCallIdempotencyStore>(_ =>
        new MemoryModelCallIdempotencyStore(
            options.IdempotencyMaximumEntries,
            TimeSpan.FromMinutes(options.IdempotencyRetentionMinutes),
            TimeProvider.System));
    builder.Services.AddSingleton<ISignedPolicyService, DevelopmentSignedPolicyService>();
    builder.Services.AddSingleton<IModelReceiptSigner, DevelopmentModelReceiptSigner>();
    builder.Services.AddSingleton<IModelAccessBroker, DevelopmentModelAccessBroker>();
    builder.Services.AddSingleton<IContextConsentVerifier>(
        new Sapphirus.DesktopSupportApi.Security.InstallationConsentVerifier(TimeProvider.System));
    builder.Services.AddSingleton<IContextConsentConsumptionStore>(_ =>
        !string.IsNullOrWhiteSpace(options.DevelopmentConsentStorePath)
        ? new DevelopmentFileContextConsentConsumptionStore(options.DevelopmentConsentStorePath)
        : new UnavailableContextConsentConsumptionStore());
}
else
{
    builder.AddProductionComposition(options);
}

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
        SupportPlaneOptions configuration,
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
        ActiveDeviceRegistration? device = await registry.FindActiveAsync(
            subject,
            request.RegistrationId,
            cancellationToken);
        if (device is null)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "device_registration_unavailable",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        using CancellationTokenSource operationCancellation =
            CancellationTokenSource.CreateLinkedTokenSource(
                cancellationToken,
                device.RevocationToken);
        operationCancellation.CancelAfter(
            TimeSpan.FromSeconds(configuration.ConnectedOperationTimeoutSeconds));
        try
        {
            SignedEntitlementLease lease = await idempotency.ExecuteAsync(
                subject,
                key,
                RequestGuards.Fingerprint(request),
                async () =>
                {
                    SignedEntitlementLease created = await CancellableOperation.WaitAsync(
                        signer.CreateLeaseAsync(
                            subject,
                            request.RegistrationId,
                            operationCancellation.Token),
                        operationCancellation.Token);
                    return await registry.CommitLeaseIfActiveAsync(
                        device,
                        created,
                        operationCancellation.Token);
                },
                operationCancellation.Token);
            return Results.Ok(lease);
        }
        catch (DeviceRegistrationRevokedException)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "device_registration_unavailable",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        catch (OperationCanceledException) when (
            device.RevocationToken.IsCancellationRequested
            && !cancellationToken.IsCancellationRequested)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "device_registration_unavailable",
                    StatusCodes.Status403Forbidden),
                statusCode: StatusCodes.Status403Forbidden);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "connected_operation_timeout",
                    StatusCodes.Status504GatewayTimeout),
                statusCode: StatusCodes.Status504GatewayTimeout);
        }
    })
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapGet("/policy/current", async (
        ISignedPolicyService signer,
        SupportPlaneOptions configuration,
        CancellationToken cancellationToken) =>
    {
        using CancellationTokenSource operationCancellation =
            CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        operationCancellation.CancelAfter(
            TimeSpan.FromSeconds(configuration.ConnectedOperationTimeoutSeconds));
        try
        {
            SignedDesktopPolicy policy = await CancellableOperation.WaitAsync(
                signer.CurrentPolicyAsync(operationCancellation.Token),
                operationCancellation.Token);
            return Results.Ok(policy);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            return Results.Json(
                RequestGuards.SafeProblem(
                    "connected_operation_timeout",
                    StatusCodes.Status504GatewayTimeout),
                statusCode: StatusCodes.Status504GatewayTimeout);
        }
    })
    .RequireAuthorization(DesktopScopes.Access);

desktop.MapPost("/model-access/calls", async (
        ModelAccessRequest request,
        HttpContext context,
        Sapphirus.DesktopSupportApi.Model.ModelAccessCoordinator coordinator,
        CancellationToken cancellationToken) =>
    {
        Sapphirus.DesktopSupportApi.Model.ModelAccessCoordinationResult outcome =
            await coordinator.ExecuteAsync(
                request,
                RequestGuards.IdempotencyKey(context.Request),
                ScopeAuthorization.SubjectPartition(context.User),
                cancellationToken);
        return outcome.StatusCode == StatusCodes.Status200OK
            ? Results.Ok(outcome.Body)
            : Results.Json(outcome.Body, statusCode: outcome.StatusCode);
    })
    .RequireAuthorization(DesktopScopes.ModelInvoke)
    .DisableAntiforgery();

desktop.MapGet("/model-access/receipts/{receiptId}", async (
        string receiptId,
        HttpContext context,
        IDeviceRegistry registry,
        CancellationToken cancellationToken) =>
    {
        string subject = ScopeAuthorization.SubjectPartition(context.User);
        ModelAccessReceipt? receipt = await registry.GetReceiptAsync(
            subject,
            receiptId,
            cancellationToken);
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
