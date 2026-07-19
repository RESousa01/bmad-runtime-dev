using System.Security.Cryptography;
using System.Text;
using Microsoft.Data.SqlClient;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Durable single-use consent authority shared across API replicas. The
/// primary key over (subject hash, registration, consumption hash) is the
/// authority: exactly one replica ever observes a successful insert.
/// </summary>
public sealed class SqlConsentConsumptionStore(SqlConnectionFactory connectionFactory)
    : IContextConsentConsumptionStore
{
    public async ValueTask<ContextConsentConsumption> ConsumeAsync(
        ContextConsentConsumptionRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!ContractIds.Is(request.RegistrationId, "dreg")
            || string.IsNullOrWhiteSpace(request.SubjectPartition)
            || string.IsNullOrWhiteSpace(request.DecisionId)
            || string.IsNullOrWhiteSpace(request.RequestId)
            || !RequestGuards.IsSha256(request.ConsumptionHash)
            || request.ConsumedAt == default)
        {
            throw new ArgumentException(
                "The consent consumption authority is invalid.",
                nameof(request));
        }

        string subjectHash = "sha256:" + Convert.ToHexStringLower(
            SHA256.HashData(Encoding.UTF8.GetBytes(request.SubjectPartition)));
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        try
        {
            await using SqlCommand consume = connection.CreateCommand();
            consume.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
            consume.CommandText =
                """
                INSERT INTO dbo.desktop_context_consent_consumptions
                    (subject_hash, registration_id, consumption_hash, decision_id,
                     request_id, consumed_at)
                VALUES
                    (@subjectHash, @registrationId, @consumptionHash, @decisionId,
                     @requestId, @consumedAt);
                """;
            consume.Parameters.AddWithValue("@subjectHash", subjectHash);
            consume.Parameters.AddWithValue("@registrationId", request.RegistrationId);
            consume.Parameters.AddWithValue("@consumptionHash", request.ConsumptionHash);
            consume.Parameters.AddWithValue("@decisionId", request.DecisionId);
            consume.Parameters.AddWithValue("@requestId", request.RequestId);
            consume.Parameters.AddWithValue(
                "@consumedAt",
                request.ConsumedAt.ToUniversalTime());
            await consume.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
            return ContextConsentConsumption.Consumed;
        }
        catch (SqlException exception) when (SqlDeviceRegistry.IsDuplicateKey(exception))
        {
            return ContextConsentConsumption.AlreadyConsumed;
        }
    }
}
