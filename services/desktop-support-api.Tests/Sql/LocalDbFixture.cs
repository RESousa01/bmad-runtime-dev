using Microsoft.Data.SqlClient;
using Sapphirus.DesktopSupportApi.Sql;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Sql;

/// <summary>
/// Provisions a throwaway LocalDB database and applies the embedded schema
/// migrations. Tests call <see cref="EnsureAvailable"/> first and skip when
/// no LocalDB engine exists on the machine.
/// </summary>
public sealed class LocalDbFixture : IAsyncLifetime
{
    private const string MasterConnectionString =
        @"Server=(localdb)\MSSQLLocalDB;Database=master;Integrated Security=True;Encrypt=False;Connect Timeout=20";

    private readonly string _databaseName =
        "sapphirus_authority_test_" + Guid.NewGuid().ToString("N");

    public string? UnavailableReason { get; private set; }

    public SqlConnectionFactory ConnectionFactory { get; private set; } = null!;

    public async ValueTask InitializeAsync()
    {
        try
        {
            await using (SqlConnection master = new(MasterConnectionString))
            {
                await master.OpenAsync();
                await using SqlCommand create = master.CreateCommand();
                create.CommandText = $"CREATE DATABASE [{_databaseName}];";
                await create.ExecuteNonQueryAsync();
            }
            ConnectionFactory = new SqlConnectionFactory(
                @$"Server=(localdb)\MSSQLLocalDB;Database={_databaseName};Integrated Security=True;Encrypt=False;Connect Timeout=20");
            await new SqlMigrationRunner(ConnectionFactory)
                .ApplyAsync(CancellationToken.None);
        }
        catch (Exception exception) when (
            exception is SqlException or PlatformNotSupportedException or InvalidOperationException)
        {
            UnavailableReason =
                "SQL LocalDB is not available on this machine: " + exception.Message;
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (UnavailableReason is not null)
        {
            return;
        }
        SqlConnection.ClearAllPools();
        await using SqlConnection master = new(MasterConnectionString);
        await master.OpenAsync();
        await using SqlCommand drop = master.CreateCommand();
        drop.CommandText =
            $"""
            ALTER DATABASE [{_databaseName}] SET SINGLE_USER WITH ROLLBACK IMMEDIATE;
            DROP DATABASE [{_databaseName}];
            """;
        await drop.ExecuteNonQueryAsync();
    }

    public void EnsureAvailable()
    {
        if (UnavailableReason is not null)
        {
            Assert.Skip(UnavailableReason);
        }
    }
}
