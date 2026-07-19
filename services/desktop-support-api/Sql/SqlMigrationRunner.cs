using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Data.SqlClient;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Applies embedded schema migrations exactly once, in name order, each in
/// its own transaction. Intended to run as an explicit deployment step under
/// the migration identity; the runtime identity holds DML permissions only
/// and cannot alter schema (see infra/desktop-support/sql-grants.md).
/// </summary>
public sealed class SqlMigrationRunner(SqlConnectionFactory connectionFactory)
{
    private const string ResourcePrefix = "Sapphirus.DesktopSupportApi.Sql.Migrations.";

    public async Task ApplyAsync(CancellationToken cancellationToken)
    {
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        await EnsureMigrationTableAsync(connection, cancellationToken).ConfigureAwait(false);
        foreach ((string name, string script) in ReadEmbeddedMigrations())
        {
            string scriptHash = "sha256:" + Convert.ToHexStringLower(
                SHA256.HashData(Encoding.UTF8.GetBytes(script)));
            string? appliedHash = await FindAppliedHashAsync(
                connection,
                name,
                cancellationToken).ConfigureAwait(false);
            if (appliedHash is not null)
            {
                if (!string.Equals(appliedHash, scriptHash, StringComparison.Ordinal))
                {
                    throw new InvalidOperationException(
                        $"Migration '{name}' was already applied with different content.");
                }
                continue;
            }
            await using SqlTransaction transaction = (SqlTransaction)await connection
                .BeginTransactionAsync(cancellationToken)
                .ConfigureAwait(false);
            await using (SqlCommand apply = connection.CreateCommand())
            {
                apply.Transaction = transaction;
                apply.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
                apply.CommandText = script;
                await apply.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
            }
            await using (SqlCommand record = connection.CreateCommand())
            {
                record.Transaction = transaction;
                record.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
                record.CommandText =
                    """
                    INSERT INTO dbo.desktop_schema_migrations
                        (migration_name, script_sha256, applied_at)
                    VALUES (@name, @hash, SYSDATETIMEOFFSET());
                    """;
                record.Parameters.AddWithValue("@name", name);
                record.Parameters.AddWithValue("@hash", scriptHash);
                await record.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
            }
            await transaction.CommitAsync(cancellationToken).ConfigureAwait(false);
        }
    }

    private static async Task EnsureMigrationTableAsync(
        SqlConnection connection,
        CancellationToken cancellationToken)
    {
        await using SqlCommand command = connection.CreateCommand();
        command.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        command.CommandText =
            """
            IF OBJECT_ID(N'dbo.desktop_schema_migrations', N'U') IS NULL
            BEGIN
                CREATE TABLE dbo.desktop_schema_migrations (
                    migration_name NVARCHAR(128) NOT NULL,
                    script_sha256 NVARCHAR(96) NOT NULL,
                    applied_at DATETIMEOFFSET(3) NOT NULL,
                    CONSTRAINT pk_desktop_schema_migrations
                        PRIMARY KEY (migration_name)
                );
            END
            """;
        await command.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
    }

    private static async Task<string?> FindAppliedHashAsync(
        SqlConnection connection,
        string name,
        CancellationToken cancellationToken)
    {
        await using SqlCommand command = connection.CreateCommand();
        command.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        command.CommandText =
            """
            SELECT script_sha256 FROM dbo.desktop_schema_migrations
            WHERE migration_name = @name;
            """;
        command.Parameters.AddWithValue("@name", name);
        object? applied = await command
            .ExecuteScalarAsync(cancellationToken)
            .ConfigureAwait(false);
        return applied as string;
    }

    private static IReadOnlyList<(string Name, string Script)> ReadEmbeddedMigrations()
    {
        Assembly assembly = typeof(SqlMigrationRunner).Assembly;
        List<(string Name, string Script)> migrations = [];
        foreach (string resource in assembly
            .GetManifestResourceNames()
            .Where(static name => name.StartsWith(ResourcePrefix, StringComparison.Ordinal))
            .OrderBy(static name => name, StringComparer.Ordinal))
        {
            using Stream stream = assembly.GetManifestResourceStream(resource)
                ?? throw new InvalidOperationException(
                    $"Embedded migration '{resource}' could not be read.");
            using StreamReader reader = new(stream, Encoding.UTF8);
            migrations.Add((resource[ResourcePrefix.Length..], reader.ReadToEnd()));
        }
        if (migrations.Count == 0)
        {
            throw new InvalidOperationException("No embedded schema migrations were found.");
        }
        return migrations;
    }
}
