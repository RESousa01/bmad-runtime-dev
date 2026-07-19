using Microsoft.Data.SqlClient;
using Sapphirus.DesktopSupportApi.Configuration;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Produces pooled, Entra-only authority connections. Production always
/// authenticates with the designated user-assigned managed identity; the
/// raw-connection-string path exists only for integration tests.
/// </summary>
public sealed class SqlConnectionFactory
{
    public const int CommandTimeoutSeconds = 30;

    private readonly string _connectionString;

    public SqlConnectionFactory(ProductionOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        SqlConnectionStringBuilder builder = new()
        {
            DataSource = "tcp:" + options.SqlServer + ",1433",
            InitialCatalog = options.SqlDatabase,
            Authentication = SqlAuthenticationMethod.ActiveDirectoryManagedIdentity,
            UserID = options.ManagedIdentityClientId.ToString("D"),
            Encrypt = SqlConnectionEncryptOption.Mandatory,
            TrustServerCertificate = false,
            Pooling = true,
            ConnectTimeout = 15,
            CommandTimeout = CommandTimeoutSeconds,
        };
        _connectionString = builder.ConnectionString;
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
