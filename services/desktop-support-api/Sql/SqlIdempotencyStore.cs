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
    /// <summary>
    /// A started claim older than this is treated as abandoned by a crashed
    /// replica and may be taken over. Request-level operations are
    /// retry-safe; model calls use the dedicated marker store, which never
    /// takes over an uncertain claim.
    /// </summary>
    public static readonly TimeSpan StaleClaimTakeover = TimeSpan.FromMinutes(10);

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
                    if (await TryReclaimStaleClaimAsync(
                        connection,
                        subject,
                        key,
                        cancellationToken).ConfigureAwait(false))
                    {
                        continue;
                    }
                    // Another replica holds a live claim; the request cannot
                    // be replayed or re-executed while its outcome is unknown.
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

    private static async Task<bool> TryReclaimStaleClaimAsync(
        SqlConnection connection,
        string subject,
        string key,
        CancellationToken cancellationToken)
    {
        await using SqlCommand reclaim = connection.CreateCommand();
        reclaim.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        reclaim.CommandText =
            """
            DELETE FROM dbo.desktop_request_idempotency
            WHERE subject = @subject AND idempotency_key = @key
              AND state = N'started'
              AND created_at < DATEADD(second, -@staleSeconds, SYSDATETIMEOFFSET());
            """;
        reclaim.Parameters.AddWithValue("@subject", subject);
        reclaim.Parameters.AddWithValue("@key", key);
        reclaim.Parameters.AddWithValue(
            "@staleSeconds",
            (int)StaleClaimTakeover.TotalSeconds);
        return await reclaim
            .ExecuteNonQueryAsync(cancellationToken)
            .ConfigureAwait(false) == 1;
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

/// <summary>
/// Durable single-use reservations for streaming runs, shared across replicas.
///
/// <para>
/// It reuses <c>dbo.desktop_model_call_idempotency</c> rather than adding a
/// table, and that reuse is a decision rather than a shortcut. The row shape is
/// already exactly right — a subject-partitioned key, the request fingerprint,
/// a started/completed state, and a receipt marker with no payload — and
/// sharing the table means one idempotency key can never fund both a structured
/// call and a stream: the fingerprints differ, so the second one conflicts. No
/// migration is therefore required, and the forward-only migration history stays
/// at <c>0001_support_authority.sql</c>.
/// </para>
/// <para>
/// The one behavioural difference from
/// <see cref="SqlModelCallIdempotencyStore"/> is that this store never releases
/// a claim. That store releases when acquisition fails before any commit, which
/// is safe there because nothing was spent. Here the route consumes consent
/// before the reservation is ever used, so releasing on failure would hand the
/// key back over consent that is already gone. A started row that never
/// completes stays as terminal uncertainty, and it is reported as such.
/// </para>
/// <para>
/// Only the receipt marker is ever persisted. No transcript, no prompt, and no
/// provider body reaches this table.
/// </para>
/// </summary>
public sealed class SqlModelStreamIdempotencyStore(SqlConnectionFactory connectionFactory)
    : IModelStreamIdempotencyStore
{
    public async Task<ModelStreamReservation> ReserveAsync(
        string subject,
        string key,
        string requestFingerprint,
        CancellationToken cancellationToken)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        ArgumentException.ThrowIfNullOrWhiteSpace(requestFingerprint);
        cancellationToken.ThrowIfCancellationRequested();

        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        while (true)
        {
            (string Fingerprint, ModelCallCompletionMarker? Marker)? existing =
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
                return existing.Value.Marker is ModelCallCompletionMarker marker
                    ? ModelStreamReservation.Replay(marker)
                    : ModelStreamReservation.Uncertain;
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
                return ModelStreamReservation.Fresh;
            }
            catch (SqlException exception) when (SqlDeviceRegistry.IsDuplicateKey(exception))
            {
                // Another replica claimed the key between the read and the
                // insert. Re-read and report what it left behind.
                continue;
            }
        }
    }

    public async Task CompleteAsync(
        string subject,
        string key,
        ModelCallCompletionMarker completion,
        CancellationToken cancellationToken)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(subject);
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        ArgumentNullException.ThrowIfNull(completion);
        cancellationToken.ThrowIfCancellationRequested();
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
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
        complete.Parameters.AddWithValue("@receiptId", completion.ReceiptId);
        complete.Parameters.AddWithValue("@requestHash", completion.RequestHash);
        complete.Parameters.AddWithValue("@resultHash", completion.ResultHash);
        // The run is over; a disconnected desktop must not stop the outcome
        // from being recorded.
        await complete
            .ExecuteNonQueryAsync(CancellationToken.None)
            .ConfigureAwait(false);
    }

    private static async Task<(string Fingerprint, ModelCallCompletionMarker? Marker)?>
        ReadAsync(
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
        ModelCallCompletionMarker? marker =
            reader.GetString(0) == "completed" && !reader.IsDBNull(2)
                ? new ModelCallCompletionMarker(
                    reader.GetString(2),
                    reader.GetString(3),
                    reader.GetString(4))
                : null;
        return (reader.GetString(1), marker);
    }
}
