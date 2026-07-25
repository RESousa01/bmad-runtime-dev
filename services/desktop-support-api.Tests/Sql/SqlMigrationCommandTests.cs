using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;
using Sapphirus.DesktopSupportApi.Configuration;
using Sapphirus.DesktopSupportApi.Sql;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Sql;

public sealed class SqlMigrationCommandTests
{
    [Fact]
    public void Select_accepts_only_the_exact_migration_command()
    {
        Assert.Equal(
            SqlMigrationCommandSelection.MigrateSql,
            SqlMigrationCommand.Select(["migrate-sql"]));
        Assert.Equal(
            SqlMigrationCommandSelection.WebServer,
            SqlMigrationCommand.Select(["--urls", "http://127.0.0.1:8080"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select(["migrate-sql", "unexpected"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select(["migrate-sql-now"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select(["migrate_sql"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select(["--migrate-sql"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select(["MIGRATE-SQL"]));
        Assert.Equal(
            SqlMigrationCommandSelection.Invalid,
            SqlMigrationCommand.Select([
                "--urls",
                "http://127.0.0.1:8080",
                "migrate-sql",
            ]));
    }

    [Fact]
    public async Task Execute_runs_migration_once_and_returns_success()
    {
        int invocations = 0;

        int exitCode = await SqlMigrationCommand.ExecuteAsync(
            new ProductionHostEnvironment(),
            _ =>
            {
                invocations++;
                return Task.CompletedTask;
            },
            TextWriter.Null,
            CancellationToken.None);

        Assert.Equal(0, exitCode);
        Assert.Equal(1, invocations);
    }

    [Fact]
    public async Task Execute_returns_nonzero_and_redacts_failure_details()
    {
        StringWriter error = new();

        int exitCode = await SqlMigrationCommand.ExecuteAsync(
            new ProductionHostEnvironment(),
            _ => throw new InvalidOperationException("tcp:private.database.windows.net"),
            error,
            CancellationToken.None);

        Assert.NotEqual(0, exitCode);
        Assert.Contains("failed", error.ToString(), StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("private.database.windows.net", error.ToString());
    }

    [Fact]
    public async Task Execute_rejects_nonproduction_without_invoking_migration()
    {
        bool invoked = false;

        int exitCode = await SqlMigrationCommand.ExecuteAsync(
            new ProductionHostEnvironment { EnvironmentName = Environments.Development },
            _ =>
            {
                invoked = true;
                return Task.CompletedTask;
            },
            TextWriter.Null,
            CancellationToken.None);

        Assert.NotEqual(0, exitCode);
        Assert.False(invoked);
    }

    [Fact]
    public void Migration_identity_must_be_present_and_distinct_from_runtime_identity()
    {
        Guid runtimeIdentity = Guid.Parse("44444444-4444-4444-4444-444444444444");

        Assert.Throws<InvalidOperationException>(() => new ProductionOptions
        {
            ManagedIdentityClientId = runtimeIdentity,
        }.ValidateSqlMigrationIdentity());
        Assert.Throws<InvalidOperationException>(() => new ProductionOptions
        {
            ManagedIdentityClientId = runtimeIdentity,
            SqlMigrationIdentityClientId = runtimeIdentity,
        }.ValidateSqlMigrationIdentity());

        new ProductionOptions
        {
            ManagedIdentityClientId = runtimeIdentity,
            SqlMigrationIdentityClientId =
                Guid.Parse("55555555-5555-5555-5555-555555555555"),
        }.ValidateSqlMigrationIdentity();
    }

    [Fact]
    public void Migration_connection_factory_selects_only_the_migration_identity()
    {
        Guid runtimeIdentity = Guid.Parse("44444444-4444-4444-4444-444444444444");
        Guid migrationIdentity = Guid.Parse("55555555-5555-5555-5555-555555555555");
        ProductionOptions options = new()
        {
            ManagedIdentityClientId = runtimeIdentity,
            SqlMigrationIdentityClientId = migrationIdentity,
        };

        SqlConnectionFactory factory = SqlConnectionFactory.ForMigration(options);
        SqlConnectionFactory runtimeFactory = new(options);

        Assert.Equal(migrationIdentity, factory.ManagedIdentityClientId);
        Assert.NotEqual(runtimeIdentity, factory.ManagedIdentityClientId);
        Assert.Equal(runtimeIdentity, runtimeFactory.ManagedIdentityClientId);
        Assert.NotEqual(migrationIdentity, runtimeFactory.ManagedIdentityClientId);
    }

    [Fact]
    public void Normal_production_configuration_binds_without_a_migration_identity()
    {
        const string hash =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        IConfiguration configuration = new ConfigurationBuilder()
            .SetBasePath(AppContext.BaseDirectory)
            .AddJsonFile("appsettings.json")
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["Sapphirus:ManagedIdentityClientId"] =
                    "44444444-4444-4444-4444-444444444444",
                ["Sapphirus:AppConfigurationEndpoint"] =
                    "https://configuration.azconfig.io/",
                ["Sapphirus:KeyVaultUri"] = "https://signing.vault.azure.net/",
                ["Sapphirus:ReceiptSigningKeyName"] = "model-receipt-signing",
                ["Sapphirus:SqlServer"] = "authority.database.windows.net",
                ["Sapphirus:SqlDatabase"] = "desktop-authority",
                ["Sapphirus:ModelEndpoint"] = "https://models.openai.azure.com/",
                ["Sapphirus:ModelDeployment"] = "desktop-planner",
                ["Sapphirus:ProviderProfileHash"] = hash,
                ["Sapphirus:ModelProfileHash"] = hash,
                ["Sapphirus:ModelCapabilityHash"] = hash,
                ["Sapphirus:DeploymentHash"] = hash,
            })
            .Build();

        ProductionOptions options = configuration
            .GetSection(SupportPlaneOptions.SectionName)
            .Get<ProductionOptions>() ?? throw new InvalidOperationException();

        Assert.Equal(Guid.Empty, options.SqlMigrationIdentityClientId);
        Assert.Throws<InvalidOperationException>(
            options.ValidateSqlMigrationIdentity);
    }

    private sealed class ProductionHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Production;
        public string ApplicationName { get; set; } = "Tests";
        public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
