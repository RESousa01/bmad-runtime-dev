using Microsoft.Data.SqlClient;
using Sapphirus.DesktopSupportApi.Configuration;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Produces pooled, Entra-only authority connections. Production callers
/// must explicitly select either the runtime or migration user-assigned
/// managed identity; the raw-connection-string path exists only for
/// integration tests.
/// </summary>
public sealed class SqlConnectionFactory
{
    public const int CommandTimeoutSeconds = 30;

    private readonly string _connectionString;
    internal Guid ManagedIdentityClientId { get; }

    public SqlConnectionFactory(ProductionOptions options)
        : this(options, options?.ManagedIdentityClientId ?? Guid.Empty)
    {
    }

    internal SqlConnectionFactory(
        ProductionOptions options,
        Guid managedIdentityClientId)
    {
        ArgumentNullException.ThrowIfNull(options);
        if (managedIdentityClientId == Guid.Empty)
        {
            throw new InvalidOperationException(
                "A managed identity client ID is required for SQL.");
        }
        ManagedIdentityClientId = managedIdentityClientId;
        SqlConnectionStringBuilder builder = new()
        {
            DataSource = "tcp:" + options.SqlServer + ",1433",
            InitialCatalog = options.SqlDatabase,
            Authentication = SqlAuthenticationMethod.ActiveDirectoryManagedIdentity,
            UserID = managedIdentityClientId.ToString("D"),
            Encrypt = SqlConnectionEncryptOption.Mandatory,
            TrustServerCertificate = false,
            Pooling = true,
            ConnectTimeout = 15,
            CommandTimeout = CommandTimeoutSeconds,
        };
        _connectionString = builder.ConnectionString;
    }

    internal static SqlConnectionFactory ForMigration(ProductionOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        options.ValidateSqlMigrationIdentity();
        return new SqlConnectionFactory(options, options.SqlMigrationIdentityClientId);
    }

    internal SqlConnectionFactory(string connectionString)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(connectionString);
        _connectionString = connectionString;
    }

    public async Task<SqlConnection> OpenAsync(CancellationToken cancellationToken)
    {
        SqlConnection connection = new(_connectionString);
        try
        {
            await connection.OpenAsync(cancellationToken).ConfigureAwait(false);
            return connection;
        }
        catch
        {
            await connection.DisposeAsync().ConfigureAwait(false);
            throw;
        }
    }
}
