using System.Text.Json;
using Microsoft.Data.SqlClient;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Durable request idempotency shared across API replicas. A committed row
/// replays the stored response for the same subject/key/fingerprint; a
/// different fingerprint conflicts. Model calls must use
/// <see cref="SqlModelCallIdempotencyStore"/> — this store refuses to persist
/// model results because their payload is transient content.
/// </summary>
public sealed class SqlIdempotencyStore(SqlConnectionFactory connectionFactory)
    : IIdempotencyStore
{
    public async Task<T> ExecuteAsync<T>(
        string subject,
        string key,
        string requestFingerprint,
        Func<Task<T>> operation,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operation);
        cancellationToken.ThrowIfCancellationRequested();
        if (typeof(ModelAccessResult).IsAssignableFrom(typeof(T)))
        {
            throw new InvalidOperationException(
                "Model results must flow through the model-call idempotency store.");
        }

        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        while (true)
        {
            (string State, string? ResponseType, string? ResponseJson, string Fingerprint)?
                existing = await ReadAsync(connection, subject, key, cancellationToken)
                    .ConfigureAwait(false);
            if (existing is not null)
            {
                if (!string.Equals(
                    existing.Value.Fingerprint,
                    requestFingerprint,
                    StringComparison.Ordinal))
                {
                    throw new IdempotencyConflictException();
                }
                if (existing.Value.State != "completed")
                {
                    // Another replica holds the claim; the request cannot be
                    // replayed or re-executed while its outcome is unknown.
                    throw new IdempotencyConflictException();
                }
                if (!string.Equals(
                    existing.Value.ResponseType,
                    typeof(T).FullName,
                    StringComparison.Ordinal)
                    || existing.Value.ResponseJson is null)
                {
                    throw new InvalidOperationException(
                        "Idempotency key was reused for another response type.");
                }
                return JsonSerializer.Deserialize<T>(existing.Value.ResponseJson)
                    ?? throw new InvalidOperationException(
                        "A stored idempotent response could not be replayed.");
            }

            try
            {
                await using SqlCommand claim = connection.CreateCommand();
                claim.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
                claim.CommandText =
                    """
                    INSERT INTO dbo.desktop_request_idempotency
                        (subject, idempotency_key, request_fingerprint, state,
                         response_type, response_json, created_at, completed_at)
                    VALUES
                        (@subject, @key, @fingerprint, N'started', NULL, NULL,
                         SYSDATETIMEOFFSET(), NULL);
                    """;
                claim.Parameters.AddWithValue("@subject", subject);
                claim.Parameters.AddWithValue("@key", key);
                claim.Parameters.AddWithValue("@fingerprint", requestFingerprint);
                await claim.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
            }
            catch (SqlException exception) when (SqlDeviceRegistry.IsDuplicateKey(exception))
            {
                continue;
            }

            try
            {
                T value = await operation().ConfigureAwait(false)
                    ?? throw new InvalidOperationException(
                        "An idempotent operation returned null.");
                await using SqlCommand complete = connection.CreateCommand();
                complete.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
                complete.CommandText =
                    """
                    UPDATE dbo.desktop_request_idempotency
                    SET state = N'completed',
                        response_type = @responseType,
                        response_json = @responseJson,
                        completed_at = SYSDATETIMEOFFSET()
                    WHERE subject = @subject AND idempotency_key = @key;
                    """;
                complete.Parameters.AddWithValue("@subject", subject);
                complete.Parameters.AddWithValue("@key", key);
                complete.Parameters.AddWithValue("@responseType", typeof(T).FullName);
                complete.Parameters.AddWithValue(
                    "@responseJson",
                    JsonSerializer.Serialize(value));
                await complete
                    .ExecuteNonQueryAsync(CancellationToken.None)
                    .ConfigureAwait(false);
                return value;
            }
            catch
            {
                await ReleaseClaimAsync(connection, subject, key).ConfigureAwait(false);
                throw;
            }
        }
    }

    private static async Task<(string State, string? ResponseType, string? ResponseJson,
        string Fingerprint)?> ReadAsync(
        SqlConnection connection,
        string subject,
        string key,
        CancellationToken cancellationToken)
    {
        await using SqlCommand select = connection.CreateCommand();
        select.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        select.CommandText =
            """
            SELECT state, response_type, response_json, request_fingerprint
            FROM dbo.desktop_request_idempotency
            WHERE subject = @subject AND idempotency_key = @key;
            """;
        select.Parameters.AddWithValue("@subject", subject);
        select.Parameters.AddWithValue("@key", key);
        await using SqlDataReader reader = await select
            .ExecuteReaderAsync(cancellationToken)
            .ConfigureAwait(false);
        if (!await reader.ReadAsync(cancellationToken).ConfigureAwait(false))
        {
            return null;
        }
        return (
            reader.GetString(0),
            reader.IsDBNull(1) ? null : reader.GetString(1),
            reader.IsDBNull(2) ? null : reader.GetString(2),
            reader.GetString(3));
    }

    private static async Task ReleaseClaimAsync(
        SqlConnection connection,
        string subject,
        string key)
    {
        await using SqlCommand release = connection.CreateCommand();
        release.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        release.CommandText =
            """
            DELETE FROM dbo.desktop_request_idempotency
            WHERE subject = @subject AND idempotency_key = @key
              AND state = N'started';
            """;
        release.Parameters.AddWithValue("@subject", subject);
        release.Parameters.AddWithValue("@key", key);
        await release.ExecuteNonQueryAsync(CancellationToken.None).ConfigureAwait(false);
    }
}
