using Microsoft.Data.SqlClient;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Thrown when a model-call idempotency key has a started claim whose outcome
/// is unknown. The caller must not retry with the same key until an operator
/// resolves the uncertainty; retrying cannot broaden authority.
/// </summary>
public sealed class ModelCallIdempotencyUncertainException : Exception;

/// <summary>
/// Durable model-call idempotency. Only the completion marker (receipt id and
/// request/result hashes) is ever persisted — never the model payload. An
/// interrupted call leaves a started claim that fails closed instead of
/// returning a success-shaped result.
/// </summary>
public sealed class SqlModelCallIdempotencyStore(SqlConnectionFactory connectionFactory)
    : IModelCallIdempotencyStore
{
    public async Task<ModelCallIdempotencyResult> ExecuteAsync(
        string subject,
        string key,
        string requestFingerprint,
        Func<CancellationToken, Task<ModelAccessResult>> acquireResult,
        Func<ModelAccessResult, CancellationToken, Task<ModelAccessResult>> commitLocalResult,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(acquireResult);
        ArgumentNullException.ThrowIfNull(commitLocalResult);
        cancellationToken.ThrowIfCancellationRequested();

        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        while (true)
        {
            (string State, string Fingerprint, ModelCallCompletionMarker? Marker)? existing =
                await ReadAsync(connection, subject, key, cancellationToken)
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
                if (existing.Value.Marker is ModelCallCompletionMarker marker)
                {
                    return ModelCallIdempotencyResult.Replay(marker);
                }
                throw new ModelCallIdempotencyUncertainException();
            }

            try
            {
                await using SqlCommand claim = connection.CreateCommand();
                claim.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
                claim.CommandText =
                    """
                    INSERT INTO dbo.desktop_model_call_idempotency
                        (subject, idempotency_key, request_fingerprint, state,
                         receipt_id, request_hash, result_hash, started_at,
                         completed_at)
                    VALUES
                        (@subject, @key, @fingerprint, N'started', NULL, NULL,
                         NULL, SYSDATETIMEOFFSET(), NULL);
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

            ModelAccessResult acquired;
            try
            {
                Task<ModelAccessResult> acquisitionTask = acquireResult(cancellationToken)
                    ?? throw new InvalidOperationException(
                        "A model-result acquisition returned no task.");
                acquired = await acquisitionTask.ConfigureAwait(false);
                cancellationToken.ThrowIfCancellationRequested();
            }
            catch
            {
                // Nothing was committed; releasing the claim keeps the original
                // retry authority without broadening it.
                await ReleaseClaimAsync(connection, subject, key).ConfigureAwait(false);
                throw;
            }

            // From the first commit attempt onward the outcome may exist
            // durably, so the claim is never released: a failure here leaves
            // the started row as terminal uncertainty and later calls fail
            // closed instead of re-executing the model call.
            Task<ModelAccessResult> commitTask = commitLocalResult(acquired, cancellationToken)
                ?? throw new InvalidOperationException(
                    "A local model-result commit returned no task.");
            ModelAccessResult result = await commitTask.ConfigureAwait(false);
            await using SqlCommand complete = connection.CreateCommand();
            complete.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
            complete.CommandText =
                """
                UPDATE dbo.desktop_model_call_idempotency
                SET state = N'completed',
                    receipt_id = @receiptId,
                    request_hash = @requestHash,
                    result_hash = @resultHash,
                    completed_at = SYSDATETIMEOFFSET()
                WHERE subject = @subject AND idempotency_key = @key
                  AND state = N'started';
                """;
            complete.Parameters.AddWithValue("@subject", subject);
            complete.Parameters.AddWithValue("@key", key);
            complete.Parameters.AddWithValue("@receiptId", result.Receipt.ReceiptId);
            complete.Parameters.AddWithValue("@requestHash", result.Receipt.RequestHash);
            complete.Parameters.AddWithValue("@resultHash", result.Receipt.ResultHash);
            await complete
                .ExecuteNonQueryAsync(CancellationToken.None)
                .ConfigureAwait(false);
            return ModelCallIdempotencyResult.Fresh(result);
        }
    }

    private static async Task<(string State, string Fingerprint,
        ModelCallCompletionMarker? Marker)?> ReadAsync(
        SqlConnection connection,
        string subject,
        string key,
        CancellationToken cancellationToken)
    {
        await using SqlCommand select = connection.CreateCommand();
        select.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        select.CommandText =
            """
            SELECT state, request_fingerprint, receipt_id, request_hash, result_hash
            FROM dbo.desktop_model_call_idempotency
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
        string state = reader.GetString(0);
        ModelCallCompletionMarker? marker =
            state == "completed" && !reader.IsDBNull(2)
                ? new ModelCallCompletionMarker(
                    reader.GetString(2),
                    reader.GetString(3),
                    reader.GetString(4))
                : null;
        return (state, reader.GetString(1), marker);
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
            DELETE FROM dbo.desktop_model_call_idempotency
            WHERE subject = @subject AND idempotency_key = @key
              AND state = N'started';
            """;
        release.Parameters.AddWithValue("@subject", subject);
        release.Parameters.AddWithValue("@key", key);
        await release.ExecuteNonQueryAsync(CancellationToken.None).ConfigureAwait(false);
    }
}
