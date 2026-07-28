using System.Text.Json.Serialization;
using System.Threading.RateLimiting;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Http.Json;
using Microsoft.IdentityModel.Tokens;
using Sapphirus.DesktopSupportApi;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Sql;

SqlMigrationCommandSelection command = SqlMigrationCommand.Select(args);
if (command == SqlMigrationCommandSelection.Invalid)
{
    Console.Error.WriteLine(
        $"Invalid migration command. Use exactly '{SqlMigrationCommand.Name}'.");
    return SqlMigrationCommand.InvalidArgumentsExitCode;
}

string[] hostArguments =
    command == SqlMigrationCommandSelection.WebServer ? args : [];
WebApplicationBuilder builder = WebApplication.CreateBuilder(hostArguments);

if (command == SqlMigrationCommandSelection.MigrateSql)
{
    return await SqlMigrationCommand.ExecuteAsync(
        builder.Environment,
        async cancellationToken =>
        {
            SupportPlaneOptions migrationSupport =
                GetValidatedSupportPlaneOptions(builder);
            ProductionOptions production =
                builder.GetValidatedProductionOptions(migrationSupport);
            SqlConnectionFactory connectionFactory =
                SqlConnectionFactory.ForMigration(production);
            await new SqlMigrationRunner(connectionFactory)
                .ApplyAsync(cancellationToken);
        },
        Console.Error,
        CancellationToken.None);
}

SupportPlaneOptions options = GetValidatedSupportPlaneOptions(builder);

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
builder.Services.AddSingleton<Sapphirus.DesktopSupportApi.Observability.SupportPlaneTelemetry>();

// The streaming route's version two consent verifier. It verifies the
// installation ES256 proof over the version two envelope exactly as the version
// one verifier does over its own, and additionally requires the decision and the
// request to agree on profile, catalog epoch, variant, request mode, context
// snapshot, and output schema. UnavailableModelStreamConsentVerifier remains the
// fail-closed stand-in for any composition that has no verifier at all.
builder.Services.AddSingleton<
    Sapphirus.DesktopSupportApi.Model.IModelStreamConsentVerifier>(
    new Sapphirus.DesktopSupportApi.Security.InstallationModelStreamConsentVerifier(
        TimeProvider.System));
builder.Services.AddSingleton(provider =>
    new Sapphirus.DesktopSupportApi.Model.ModelAccessCoordinator(
        provider.GetRequiredService<IDeviceRegistry>(),
        provider.GetRequiredService<IModelAccessBroker>(),
        provider.GetRequiredService<IContextConsentVerifier>(),
        provider.GetRequiredService<IContextConsentConsumptionStore>(),
        provider.GetRequiredService<IModelCallIdempotencyStore>(),
        options,
        TimeProvider.System,
        provider.GetRequiredService<Sapphirus.DesktopSupportApi.Observability.SupportPlaneTelemetry>()));
builder.Services.AddHealthChecks()
    .AddCheck<Sapphirus.DesktopSupportApi.Health.SigningCompositionHealthCheck>(
        Sapphirus.DesktopSupportApi.Health.AzureDependencyHealthChecks.SigningDependency)
    .AddCheck<Sapphirus.DesktopSupportApi.Health.ModelCompositionHealthCheck>(
        Sapphirus.DesktopSupportApi.Health.AzureDependencyHealthChecks.ModelDependency);
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
    builder.Services.AddSingleton<IModelStreamIdempotencyStore>(_ =>
        new MemoryModelStreamIdempotencyStore(
            options.IdempotencyMaximumEntries,
            TimeSpan.FromMinutes(options.IdempotencyRetentionMinutes),
            TimeProvider.System));
    builder.Services.AddSingleton<ISignedPolicyService, DevelopmentSignedPolicyService>();
    builder.Services.AddSingleton<IModelReceiptSigner, DevelopmentModelReceiptSigner>();
    // Beside the version one signer and gated on the same flag. Without it the
    // streaming coordinator cannot be composed at all, which is what previously
    // made Development answer foundry_project_unavailable to every stream.
    builder.Services.AddSingleton<IModelReceiptSignerV2, DevelopmentModelReceiptSignerV2>();
    builder.Services.AddSingleton<IModelAccessBroker, DevelopmentModelAccessBroker>();
    builder.Services.AddSingleton<IContextConsentVerifier>(
        new Sapphirus.DesktopSupportApi.Security.InstallationConsentVerifier(TimeProvider.System));
    // Durable across restarts when the operator names a store path; otherwise
    // single-use for the lifetime of this process. Production composes the SQL
    // authority unconditionally, so the previous unavailable default protected
    // nothing there and only made a local model call impossible to complete.
    builder.Services.AddSingleton<IContextConsentConsumptionStore>(_ =>
        !string.IsNullOrWhiteSpace(options.DevelopmentConsentStorePath)
        ? new DevelopmentFileContextConsentConsumptionStore(options.DevelopmentConsentStorePath)
        : new MemoryContextConsentConsumptionStore());

    // Model catalog (FND-024..FND-026) in Development.
    //
    // The registry is the labelled untrusted fixture set: it exists so the
    // whole catalog architecture runs on a machine with no App Configuration
    // store, and both it and the entitlement source refuse any environment but
    // Development and Test.
    //
    // The deployment inventory is real when, and only when, the operator
    // supplied the development Foundry credential. Otherwise it reports an
    // unavailable inventory rather than an empty one, so the catalog fails
    // closed instead of appearing to say the organization deployed nothing.
    bool developmentFoundryComposed = builder.Services.TryAddDevelopmentFoundryCatalog(
        builder.Environment,
        builder.Configuration);
    builder.Services.AddSingleton<Sapphirus.DesktopSupportApi.Model.IModelProfileRegistry>(_ =>
        new Sapphirus.DesktopSupportApi.Model.DevelopmentFixtureModelProfileRegistry(
            builder.Environment.EnvironmentName,
            Sapphirus.DesktopSupportApi.Model.DevelopmentModelProfileFixtures.Build(
                options.Region),
            TimeProvider.System));
    if (developmentFoundryComposed)
    {
        builder.Services.AddSingleton<
            Sapphirus.DesktopSupportApi.Model.IModelCatalogDeploymentSource>(provider =>
            new Sapphirus.DesktopSupportApi.Model.FoundryCatalogDeploymentSource(
                provider.GetRequiredService<
                    Sapphirus.DesktopSupportApi.Foundry.FoundryDeploymentCatalogCache>()));
    }
    else
    {
        builder.Services.AddSingleton<
            Sapphirus.DesktopSupportApi.Model.IModelCatalogDeploymentSource,
            Sapphirus.DesktopSupportApi.Model.UnavailableModelCatalogDeploymentSource>();
    }
    builder.Services.AddSingleton<
        Sapphirus.DesktopSupportApi.Model.IModelCatalogPolicySource>(_ =>
        new Sapphirus.DesktopSupportApi.Model.StaticModelCatalogPolicySource(
            Sapphirus.DesktopSupportApi.Model.StaticModelCatalogPolicySource.DevelopmentSnapshot(
                options.Region,
                TimeProvider.System.GetUtcNow())));
    builder.Services.AddSingleton<
        Sapphirus.DesktopSupportApi.Model.IModelEntitlementSource>(provider =>
        new Sapphirus.DesktopSupportApi.Model.DevelopmentModelEntitlementSource(
            builder.Environment.EnvironmentName,
            provider.GetRequiredService<Sapphirus.DesktopSupportApi.Model.IModelProfileRegistry>()));
    builder.Services.AddSingleton(provider =>
        new Sapphirus.DesktopSupportApi.Model.EffectiveModelCatalogService(
            provider.GetRequiredService<Sapphirus.DesktopSupportApi.Model.IModelProfileRegistry>(),
            provider.GetRequiredService<
                Sapphirus.DesktopSupportApi.Model.IModelCatalogDeploymentSource>(),
            provider.GetRequiredService<
                Sapphirus.DesktopSupportApi.Model.IModelCatalogPolicySource>(),
            provider.GetRequiredService<
                Sapphirus.DesktopSupportApi.Model.IModelEntitlementSource>(),
            TimeProvider.System,
            options.Region,
            untrustedRegistryPermitted: true));
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
        [
            "windows_local",
            "transient_no_store",
            Sapphirus.DesktopSupportApi.Model.ModelCatalogEndpoint.BootstrapCapability,
            Sapphirus.DesktopSupportApi.Model.ModelStreamEndpoint.BootstrapCapability,
        ],
        DateTimeOffset.UtcNow)))
    .RequireAuthorization(DesktopScopes.Access);

// The catalog is a read. It requires the approved desktop client and the
// desktop read scope, and deliberately not the model-invocation scope.
desktop.MapModelCatalog();

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

// The one fixed, reviewed streaming route. It is never renderer-supplied, and
// it carries the same authorization posture as the model-access call above:
// the approved desktop client and the model-invocation scope.
desktop.MapModelStream();

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

app.MapHealthChecks("/healthz/live", new Microsoft.AspNetCore.Diagnostics.HealthChecks.HealthCheckOptions
{
    Predicate = _ => false,
    ResponseWriter = Sapphirus.DesktopSupportApi.Health.AzureDependencyHealthChecks.WriteSafeResponseAsync,
});
app.MapHealthChecks("/healthz/ready", new Microsoft.AspNetCore.Diagnostics.HealthChecks.HealthCheckOptions
{
    ResponseWriter = Sapphirus.DesktopSupportApi.Health.AzureDependencyHealthChecks.WriteSafeResponseAsync,
});

await app.RunAsync();
return 0;

static SupportPlaneOptions GetValidatedSupportPlaneOptions(
    WebApplicationBuilder builder)
{
    SupportPlaneOptions options = builder.Configuration
        .GetSection(SupportPlaneOptions.SectionName)
        .Get<SupportPlaneOptions>() ?? new SupportPlaneOptions();
    options.Validate(builder.Environment);
    return options;
}

public partial class Program;
