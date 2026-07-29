using Azure.Monitor.OpenTelemetry.AspNetCore;
using Microsoft.Extensions.Hosting;
using OpenTelemetry.Trace;
using Sapphirus.DesktopSupportApi.Foundry;
using Sapphirus.DesktopSupportApi.Health;
using Sapphirus.DesktopSupportApi.Model;
using Sapphirus.DesktopSupportApi.Sql;
using Sapphirus.DesktopSupportApi.Policy;
using Sapphirus.DesktopSupportApi.Signing;

namespace Sapphirus.DesktopSupportApi.Configuration;

public static class ProductionComposition
{
    public static void AddProductionComposition(
        this WebApplicationBuilder builder,
        SupportPlaneOptions supportPlane)
    {
        ProductionOptions production =
            builder.GetValidatedProductionOptions(supportPlane);
        builder.Services.AddProductionAzureClients(production);
        builder.Services.AddSingleton<IPolicySettingsSource>(provider =>
            new AppConfigurationPolicySettingsSource(
                provider.GetRequiredService<ProductionAzureClients>().AppConfiguration));
        builder.Services.AddSingleton(provider => new AppConfigurationPolicyProvider(
            provider.GetRequiredService<IPolicySettingsSource>(),
            TimeProvider.System));
        builder.Services.AddSingleton<IHashSigner>(provider => new KeyVaultHashSigner(
            provider.GetRequiredService<ProductionAzureClients>().KeyVault
                .GetCryptographyClient(production.ReceiptSigningKeyName),
            production.ValidatedKeyVaultUri
                + "keys/" + production.ReceiptSigningKeyName));
        builder.Services.AddSingleton<ISignedPolicyService>(provider =>
            new AzureSignedPolicyService(
                provider.GetRequiredService<AppConfigurationPolicyProvider>(),
                provider.GetRequiredService<IHashSigner>(),
                TimeProvider.System));
        builder.Services.AddSingleton(provider =>
            new AzureModelReceiptSigner(
                provider.GetRequiredService<IHashSigner>(),
                supportPlane));
        builder.Services.AddSingleton<IModelReceiptSigner>(provider =>
            provider.GetRequiredService<AzureModelReceiptSigner>());
        builder.Services.AddSingleton<IModelReceiptSignerV2>(provider =>
            provider.GetRequiredService<AzureModelReceiptSigner>());

        // The legacy fixed Chat Completions broker is registered only when the
        // legacy model path is configured. A Foundry-only revision composes
        // and starts without it rather than failing at construction, which is
        // what lets the template stop injecting the legacy values before the
        // Responses adapter replaces the executor.
        if (production.HasLegacyModelPath)
        {
            builder.Services.AddSingleton<IModelProviderExecutor>(provider =>
                new AzureOpenAiProviderExecutor(
                    provider.GetRequiredService<ProductionAzureClients>().Model
                        ?? throw new InvalidOperationException(
                            "The legacy model client is unavailable despite a configured legacy model path.")));
            builder.Services.AddSingleton<IModelAccessBroker>(provider =>
                new AzureOpenAiModelAccessBroker(
                    async cancellationToken => ModelAccessProfile.Resolve(
                        await provider
                            .GetRequiredService<AppConfigurationPolicyProvider>()
                            .GetSnapshotAsync(cancellationToken)
                            .ConfigureAwait(false),
                        production,
                        supportPlane),
                    provider.GetRequiredService<IModelProviderExecutor>(),
                    provider.GetRequiredService<IModelReceiptSigner>(),
                    TimeProvider.System));
        }

        // Foundry deployment catalog (FND-021..FND-023): discovery runs on its
        // own user-assigned identity and its own bounded transport, and the
        // cache is the only thing the rest of the plane reads. Warming happens
        // in the background so a cold or unreachable provider shows up as an
        // unhealthy catalog rather than a host that will not start.
        //
        // The credential mechanism composed above is always managed identity.
        // The development API-key mechanism is registered only by
        // DevelopmentFoundryComposition, whose own environment guard and whose
        // authenticator constructor both refuse anything but Development and
        // Test. The assertion below is a third, structural net.
        AssertNoDevelopmentFoundryCredential(builder.Services);

        // Note on the *inbound* boundary (desktop -> plane): unlike the
        // Foundry credential above, the static desktop token is not
        // forbidden here. It is a legitimate, explicitly opt-in production
        // mechanism (SupportPlaneOptions.StaticDesktopTokenEnabled) for an
        // operator who will never hold Entra app-registration rights for
        // this internal application. See
        // Sapphirus.DesktopSupportApi.Security.DesktopAuthenticationComposition
        // for the opt-in gate and Program.cs for the one-time startup log
        // that makes an active static-token run loud rather than silent.

        builder.Services.AddSingleton(provider => new FoundryDeploymentCatalogCache(
            provider.GetRequiredService<IFoundryDeploymentDiscovery>(),
            TimeProvider.System,
            production.FoundryDeploymentRefreshInterval,
            production.FoundryCatalogMaximumStaleDuration));
        builder.Services.AddHostedService(provider => new FoundryCatalogRefreshService(
            provider.GetRequiredService<FoundryDeploymentCatalogCache>(),
            TimeProvider.System,
            production.FoundryDeploymentRefreshInterval));
        builder.Services.AddSingleton<FoundryCatalogHealthCheck>(provider =>
            new FoundryCatalogHealthCheck(
                provider.GetRequiredService<FoundryDeploymentCatalogCache>()));

        // Signed organization profile registry and effective catalog
        // (FND-024..FND-026). The registry reads only the configured profile
        // key prefix plus the registry-version key that deliberately sits
        // outside it, and it recomputes the publisher's registry and capability
        // hashes from the values it actually read.
        builder.Services.AddSingleton<IModelProfileSettingsSource>(provider =>
            new AppConfigurationModelProfileSettingsSource(
                provider.GetRequiredService<ProductionAzureClients>().AppConfiguration,
                production.ModelProfileConfigurationPrefix,
                AppConfigurationModelProfileSettingsSource.DefaultLabel));
        builder.Services.AddSingleton<AppConfigurationModelProfileRegistry>(provider =>
            new AppConfigurationModelProfileRegistry(
                provider.GetRequiredService<IModelProfileSettingsSource>(),
                TimeProvider.System));
        builder.Services.AddSingleton<IModelProfileRegistry>(provider =>
            provider.GetRequiredService<AppConfigurationModelProfileRegistry>());
        builder.Services.AddSingleton<IModelCatalogDeploymentSource>(provider =>
            new FoundryCatalogDeploymentSource(
                provider.GetRequiredService<FoundryDeploymentCatalogCache>()));
        builder.Services.AddSingleton<IModelCatalogPolicySource>(provider =>
            new AppConfigurationCatalogPolicySource(
                provider.GetRequiredService<AppConfigurationPolicyProvider>()));
        builder.Services.AddSingleton<IModelEntitlementSource>(provider =>
            new RegistryModelEntitlementSource(
                provider.GetRequiredService<IModelProfileRegistry>()));
        builder.Services.AddSingleton(provider =>
            new EffectiveModelCatalogService(
                provider.GetRequiredService<IModelProfileRegistry>(),
                provider.GetRequiredService<IModelCatalogDeploymentSource>(),
                provider.GetRequiredService<IModelCatalogPolicySource>(),
                provider.GetRequiredService<IModelEntitlementSource>(),
                TimeProvider.System,
                supportPlane.Region,
                // An untrusted fixture registry can never be composed here.
                // This is the structural refusal; the fixture registry's own
                // constructor is the first one.
                untrustedRegistryPermitted: false));

        // Foundry Responses adapter and the streaming surface (FND-028..FND-031).
        //
        // The transport is its own named client: a model call needs a far
        // longer header budget than an inventory read, and a streaming read
        // must not inherit a buffered client's response size cap. The
        // credential goes through the same authenticator seam discovery uses,
        // so this composition never sees a token or a key.
        builder.Services.AddFoundryResponsesHttpClient(production.FoundryResponsesEndpoint);
        builder.Services.AddSingleton(provider => new FoundryResponsesProviderExecutor(
            provider.GetRequiredService<IHttpClientFactory>()
                .CreateClient(FoundryResponsesProviderExecutor.HttpClientName),
            provider.GetRequiredService<IFoundryRequestAuthenticator>()));
        builder.Services.AddSingleton<IModelStreamTransport>(provider =>
            provider.GetRequiredService<FoundryResponsesProviderExecutor>());
        builder.Services.AddSingleton(provider => new ModelStreamingCoordinator(
            provider.GetRequiredService<IModelStreamTransport>(),
            provider.GetRequiredService<IModelReceiptSignerV2>(),
            TimeProvider.System,
            ModelStreamLimits.Default,
            provider.GetRequiredService<FoundryResponsesProviderExecutor>()
                .CredentialMechanismId,
            provider.GetRequiredService<Observability.SupportPlaneTelemetry>()));
        builder.Services.AddSingleton<IModelStreamRouteResolver>(provider =>
            new EffectiveCatalogStreamRouteResolver(
                provider.GetRequiredService<EffectiveModelCatalogService>(),
                provider.GetRequiredService<IModelEntitlementSource>(),
                provider.GetRequiredService<IModelCatalogPolicySource>(),
                supportPlane.Region,
                provider.GetRequiredService<Observability.SupportPlaneTelemetry>()));

        // Durable SQL authority stores (Task 7): the transactional
        // active/epoch checks in these adapters are the cross-replica
        // authority for revocation, consent single-use, and idempotency.
        builder.Services.AddSingleton(new SqlConnectionFactory(production));
        builder.Services.AddSingleton<IDeviceRegistry>(provider =>
            new SqlDeviceRegistry(provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IIdempotencyStore>(provider =>
            new SqlIdempotencyStore(provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IModelCallIdempotencyStore>(provider =>
            new SqlModelCallIdempotencyStore(
                provider.GetRequiredService<SqlConnectionFactory>()));
        // Streaming reservations share the model-call table, so no migration is
        // required; the fingerprint check means one key can never fund both a
        // structured call and a stream.
        builder.Services.AddSingleton<IModelStreamIdempotencyStore>(provider =>
            new SqlModelStreamIdempotencyStore(
                provider.GetRequiredService<SqlConnectionFactory>()));
        builder.Services.AddSingleton<IContextConsentConsumptionStore>(provider =>
            new SqlConsentConsumptionStore(
                provider.GetRequiredService<SqlConnectionFactory>()));
        // The version two stream consent verifier is composed once, for every
        // environment, in Program.cs; it needs no Azure resource, so there is
        // nothing environment-specific to register here.
        builder.Services.AddSingleton<IContextConsentVerifier>(
            new Security.InstallationConsentVerifier(TimeProvider.System));

        // Privacy-safe observability (Task 8): dependency health probes and
        // trace redaction; Azure Monitor export activates only when the
        // platform injects a connection string.
        //
        // Readiness reports the Foundry dependency classes; it never gates the
        // non-model routes. Bootstrap, device registration, entitlements, and
        // policy keep serving through a Foundry outage, which is the whole
        // point of reporting these separately rather than folding them into
        // one aggregate.
        builder.Services.AddHealthChecks()
            .AddCheck<SqlAuthorityHealthCheck>(
                AzureDependencyHealthChecks.SqlDependency)
            .AddCheck<PolicyConfigurationHealthCheck>(
                AzureDependencyHealthChecks.ConfigurationDependency)
            .AddCheck<FoundryCatalogHealthCheck>(
                FoundryCatalogHealthCheck.FoundryCatalogDependency)
            .AddCheck<FoundryProjectConfigurationHealthCheck>(
                FoundryHealthChecks.ProjectConfigurationDependency)
            .AddCheck<FoundryCredentialHealthCheck>(
                FoundryHealthChecks.CredentialDependency)
            .AddCheck<FoundryEffectiveProfilesHealthCheck>(
                FoundryHealthChecks.EffectiveProfilesDependency)
            .AddCheck<FoundryResponsesReachabilityHealthCheck>(
                FoundryHealthChecks.ResponsesReachabilityDependency);
        builder.Services.AddSingleton(new FoundryProjectConfigurationHealthCheck(production));
        builder.Services.AddSingleton(provider => new FoundryCredentialHealthCheck(
            provider.GetRequiredService<IFoundryRequestAuthenticator>(),
            TimeProvider.System));
        builder.Services.AddSingleton(provider => new FoundryEffectiveProfilesHealthCheck(
            provider.GetRequiredService<IModelProfileRegistry>()));
        builder.Services.AddSingleton(provider =>
            new FoundryResponsesReachabilityHealthCheck(
                provider.GetRequiredService<IHttpClientFactory>()
                    .CreateClient(FoundryResponsesProviderExecutor.HttpClientName),
                provider.GetRequiredService<IFoundryRequestAuthenticator>(),
                TimeProvider.System));
        builder.Services.AddSingleton<SqlAuthorityHealthCheck>();
        builder.Services.AddSingleton<PolicyConfigurationHealthCheck>(provider =>
            new PolicyConfigurationHealthCheck(
                provider.GetRequiredService<AppConfigurationPolicyProvider>()));
        if (!string.IsNullOrWhiteSpace(
            builder.Configuration["APPLICATIONINSIGHTS_CONNECTION_STRING"]))
        {
            builder.Services.AddOpenTelemetry()
                .UseAzureMonitor()
                .WithTracing(tracing => tracing.AddProcessor(
                    new Observability.PrivacyRedactionProcessor()));
        }
    }

    /// <summary>
    /// Refuses to compose a production graph that carries the development
    /// Foundry credential.
    ///
    /// <see cref="DevelopmentFoundryComposition"/> registers the development
    /// authenticator under its concrete type as well as under the seam, which
    /// is what makes it visible here without building the container.
    /// </summary>
    internal const string DevelopmentCredentialForbiddenCode =
        "production_foundry_development_credential_forbidden";

    internal static void AssertNoDevelopmentFoundryCredential(IServiceCollection services)
    {
        ArgumentNullException.ThrowIfNull(services);
        foreach (ServiceDescriptor descriptor in services)
        {
            if (descriptor.ServiceType == typeof(DevelopmentFoundryApiKeyAuthenticator)
                || descriptor.ImplementationType
                    == typeof(DevelopmentFoundryApiKeyAuthenticator)
                || descriptor.ImplementationInstance
                    is DevelopmentFoundryApiKeyAuthenticator)
            {
                throw new InvalidOperationException(DevelopmentCredentialForbiddenCode);
            }
        }
    }

    internal static ProductionOptions GetValidatedProductionOptions(
        this WebApplicationBuilder builder,
        SupportPlaneOptions supportPlane)
    {
        ProductionOptions production = builder.Configuration
            .GetSection(SupportPlaneOptions.SectionName)
            .Get<ProductionOptions>() ?? new ProductionOptions();
        production.Validate(supportPlane);
        return production;
    }
}
