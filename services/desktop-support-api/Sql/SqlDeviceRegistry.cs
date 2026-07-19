using System.Data;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.Data.SqlClient;

namespace Sapphirus.DesktopSupportApi.Sql;

/// <summary>
/// Durable subject-partitioned device authority. The transactional
/// active-state/epoch check inside each commit is the authority for
/// revocation; the in-process revocation token is only a local optimization
/// and is never consulted for correctness.
/// </summary>
public sealed class SqlDeviceRegistry(SqlConnectionFactory connectionFactory) : IDeviceRegistry
{
    private readonly Guid _registryAuthority = Guid.NewGuid();

    public async Task<DeviceRegistrationResponse> RegisterAsync(
        string subject,
        DeviceRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        cancellationToken.ThrowIfCancellationRequested();
        if (!RequestGuards.TryGetInstallationPublicKeyHash(
            request.InstallationPublicKey,
            out string installationPublicKeyHash)
            || !string.Equals(
                installationPublicKeyHash,
                request.InstallationPublicKeyHash,
                StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "The installation public key is invalid.",
                nameof(request));
        }
        string stableInput = $"{subject}:{installationPublicKeyHash}";
        string registrationId = ContractIds.FromEntropy(
            "dreg",
            SHA256.HashData(Encoding.UTF8.GetBytes(stableInput)));

        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        while (true)
        {
            RegisteredDevice? existing = await FindRegistrationAsync(
                connection,
                null,
                subject,
                registrationId,
                acquireUpdateLock: false,
                cancellationToken).ConfigureAwait(false) is (RegisteredDevice device, _)
                ? device
                : null;
            if (existing is not null)
            {
                if (!existing.IsActive)
                {
                    throw new DeviceRegistrationRevokedException();
                }
                if (!string.Equals(
                    existing.InstallationPublicKeyHash,
                    installationPublicKeyHash,
                    StringComparison.Ordinal))
                {
                    throw new InvalidOperationException(
                        "A device registration identifier collision was detected.");
                }
                return existing.ToResponse();
            }

            RegisteredDevice registration = new(
                subject,
                registrationId,
                request.InstallationPublicKey,
                installationPublicKeyHash,
                request.ClientRelease,
                request.Platform,
                request.Architecture,
                request.TenantPolicyVersion,
                DateTimeOffset.UtcNow,
                DeviceRegistrationState.Active,
                null);
            try
            {
                await using SqlCommand insert = CreateCommand(
                    connection,
                    null,
                    """
                    INSERT INTO dbo.desktop_device_registrations
                        (subject, registration_id, installation_public_key,
                         installation_public_key_hash, client_release, platform,
                         architecture, tenant_policy_version, created_at, state,
                         revoked_at, epoch)
                    VALUES
                        (@subject, @registrationId, @publicKey, @publicKeyHash,
                         @clientRelease, @platform, @architecture, @policyVersion,
                         @createdAt, N'active', NULL, 1);
                    """);
                insert.Parameters.AddWithValue("@subject", subject);
                insert.Parameters.AddWithValue("@registrationId", registrationId);
                insert.Parameters.AddWithValue("@publicKey", registration.InstallationPublicKey);
                insert.Parameters.AddWithValue("@publicKeyHash", installationPublicKeyHash);
                insert.Parameters.AddWithValue("@clientRelease", registration.ClientRelease);
                insert.Parameters.AddWithValue("@platform", registration.Platform);
                insert.Parameters.AddWithValue("@architecture", registration.Architecture);
                insert.Parameters.AddWithValue("@policyVersion", registration.TenantPolicyVersion);
                insert.Parameters.AddWithValue("@createdAt", registration.CreatedAt);
                await insert.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
            }
            catch (SqlException exception) when (IsDuplicateKey(exception))
            {
                continue;
            }
            await WriteSecurityAuditAsync(
                connection,
                null,
                subject,
                registrationId,
                "device_registered",
                cancellationToken).ConfigureAwait(false);
            return registration.ToResponse();
        }
    }

    public async Task<ActiveDeviceRegistration?> FindActiveAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        (RegisteredDevice Registration, long Epoch)? found = await FindRegistrationAsync(
            connection,
            null,
            subject,
            registrationId,
            acquireUpdateLock: false,
            cancellationToken).ConfigureAwait(false);
        return found is (RegisteredDevice registration, long epoch) && registration.IsActive
            ? new ActiveDeviceRegistration(
                registration,
                epoch,
                _registryAuthority,
                CancellationToken.None)
            : null;
    }

    public async Task<SignedEntitlementLease> CommitLeaseIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        SignedEntitlementLease lease,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operationLease);
        ArgumentNullException.ThrowIfNull(lease);
        cancellationToken.ThrowIfCancellationRequested();
        RequireAuthority(operationLease);
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        await using SqlTransaction transaction = (SqlTransaction)await connection
            .BeginTransactionAsync(IsolationLevel.ReadCommitted, cancellationToken)
            .ConfigureAwait(false);
        await RequireActiveEpochAsync(
            connection,
            transaction,
            operationLease,
            cancellationToken).ConfigureAwait(false);

        string leaseHash = "sha256:" + Convert.ToHexStringLower(
            SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(lease)));
        await using (SqlCommand audit = CreateCommand(
            connection,
            transaction,
            """
            INSERT INTO dbo.desktop_entitlement_lease_audit
                (subject, lease_id, registration_id, lease_hash, issued_at,
                 expires_at, recorded_at)
            VALUES
                (@subject, @leaseId, @registrationId, @leaseHash, @issuedAt,
                 @expiresAt, SYSDATETIMEOFFSET());
            """))
        {
            audit.Parameters.AddWithValue(
                "@subject",
                operationLease.Registration.Subject);
            audit.Parameters.AddWithValue("@leaseId", lease.LeaseId);
            audit.Parameters.AddWithValue("@registrationId", lease.RegistrationId);
            audit.Parameters.AddWithValue("@leaseHash", leaseHash);
            audit.Parameters.AddWithValue("@issuedAt", lease.IssuedAt);
            audit.Parameters.AddWithValue("@expiresAt", lease.ExpiresAt);
            await audit.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
        }
        await transaction.CommitAsync(cancellationToken).ConfigureAwait(false);
        return lease;
    }

    public async Task<ModelAccessResult> CommitModelResultIfActiveAsync(
        ActiveDeviceRegistration operationLease,
        ModelAccessRequest request,
        ModelAccessResult result,
        string expectedRegion,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(operationLease);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(result);
        cancellationToken.ThrowIfCancellationRequested();
        RequireAuthority(operationLease);
        ModelResultGuards.ValidateOrThrow(
            operationLease.Registration,
            request,
            result,
            expectedRegion);

        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        await using SqlTransaction transaction = (SqlTransaction)await connection
            .BeginTransactionAsync(IsolationLevel.ReadCommitted, cancellationToken)
            .ConfigureAwait(false);
        await RequireActiveEpochAsync(
            connection,
            transaction,
            operationLease,
            cancellationToken).ConfigureAwait(false);
        try
        {
            // Only the receipt (required proof material) is persisted; the
            // result payload never reaches storage.
            await using SqlCommand insert = CreateCommand(
                connection,
                transaction,
                """
                INSERT INTO dbo.desktop_model_access_receipts
                    (subject, receipt_id, request_id, request_hash, result_hash,
                     receipt_json, recorded_at)
                VALUES
                    (@subject, @receiptId, @requestId, @requestHash, @resultHash,
                     @receiptJson, SYSDATETIMEOFFSET());
                """);
            insert.Parameters.AddWithValue(
                "@subject",
                operationLease.Registration.Subject);
            insert.Parameters.AddWithValue("@receiptId", result.Receipt.ReceiptId);
            insert.Parameters.AddWithValue("@requestId", result.Receipt.RequestId);
            insert.Parameters.AddWithValue("@requestHash", result.Receipt.RequestHash);
            insert.Parameters.AddWithValue("@resultHash", result.Receipt.ResultHash);
            insert.Parameters.AddWithValue(
                "@receiptJson",
                JsonSerializer.Serialize(result.Receipt));
            await insert.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
        }
        catch (SqlException exception) when (IsDuplicateKey(exception))
        {
            throw new InvalidOperationException(
                "A model receipt identifier collision was detected.");
        }
        await transaction.CommitAsync(cancellationToken).ConfigureAwait(false);
        return result;
    }

    public async Task<ModelAccessReceipt?> GetReceiptAsync(
        string subject,
        string receiptId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        await using SqlCommand select = CreateCommand(
            connection,
            null,
            """
            SELECT receipt_json FROM dbo.desktop_model_access_receipts
            WHERE subject = @subject AND receipt_id = @receiptId;
            """);
        select.Parameters.AddWithValue("@subject", subject);
        select.Parameters.AddWithValue("@receiptId", receiptId);
        object? receiptJson = await select
            .ExecuteScalarAsync(cancellationToken)
            .ConfigureAwait(false);
        return receiptJson is string json
            ? JsonSerializer.Deserialize<ModelAccessReceipt>(json)
            : null;
    }

    public async Task<DeviceRevocationOutcome> RevokeAsync(
        string subject,
        string registrationId,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        await using SqlConnection connection = await connectionFactory
            .OpenAsync(cancellationToken)
            .ConfigureAwait(false);
        await using SqlCommand revoke = CreateCommand(
            connection,
            null,
            """
            UPDATE dbo.desktop_device_registrations
            SET state = N'revoked',
                revoked_at = SYSDATETIMEOFFSET(),
                epoch = epoch + 1
            WHERE subject = @subject
              AND registration_id = @registrationId
              AND state = N'active';
            """);
        revoke.Parameters.AddWithValue("@subject", subject);
        revoke.Parameters.AddWithValue("@registrationId", registrationId);
        int revoked = await revoke
            .ExecuteNonQueryAsync(cancellationToken)
            .ConfigureAwait(false);
        if (revoked == 1)
        {
            await WriteSecurityAuditAsync(
                connection,
                null,
                subject,
                registrationId,
                "device_revoked",
                cancellationToken).ConfigureAwait(false);
            return DeviceRevocationOutcome.Revoked;
        }
        return await FindRegistrationAsync(
            connection,
            null,
            subject,
            registrationId,
            acquireUpdateLock: false,
            cancellationToken).ConfigureAwait(false) is null
            ? DeviceRevocationOutcome.Unknown
            : DeviceRevocationOutcome.AlreadyRevoked;
    }

    private void RequireAuthority(ActiveDeviceRegistration operationLease)
    {
        if (operationLease.RegistryAuthority != _registryAuthority)
        {
            throw new DeviceRegistrationRevokedException();
        }
    }

    private static async Task RequireActiveEpochAsync(
        SqlConnection connection,
        SqlTransaction transaction,
        ActiveDeviceRegistration operationLease,
        CancellationToken cancellationToken)
    {
        (RegisteredDevice Registration, long Epoch)? current = await FindRegistrationAsync(
            connection,
            transaction,
            operationLease.Registration.Subject,
            operationLease.Registration.RegistrationId,
            acquireUpdateLock: true,
            cancellationToken).ConfigureAwait(false);
        if (current is not (RegisteredDevice registration, long epoch)
            || !registration.IsActive
            || epoch != operationLease.Epoch)
        {
            throw new DeviceRegistrationRevokedException();
        }
    }

    private static async Task<(RegisteredDevice Registration, long Epoch)?>
        FindRegistrationAsync(
            SqlConnection connection,
            SqlTransaction? transaction,
            string subject,
            string registrationId,
            bool acquireUpdateLock,
            CancellationToken cancellationToken)
    {
        string hints = acquireUpdateLock ? " WITH (UPDLOCK, HOLDLOCK)" : "";
        await using SqlCommand select = CreateCommand(
            connection,
            transaction,
            $"""
            SELECT installation_public_key, installation_public_key_hash,
                   client_release, platform, architecture, tenant_policy_version,
                   created_at, state, revoked_at, epoch
            FROM dbo.desktop_device_registrations{hints}
            WHERE subject = @subject AND registration_id = @registrationId;
            """);
        select.Parameters.AddWithValue("@subject", subject);
        select.Parameters.AddWithValue("@registrationId", registrationId);
        await using SqlDataReader reader = await select
            .ExecuteReaderAsync(cancellationToken)
            .ConfigureAwait(false);
        if (!await reader.ReadAsync(cancellationToken).ConfigureAwait(false))
        {
            return null;
        }
        RegisteredDevice registration = new(
            subject,
            registrationId,
            reader.GetString(0),
            reader.GetString(1),
            reader.GetString(2),
            reader.GetString(3),
            reader.GetString(4),
            reader.GetInt64(5),
            reader.GetDateTimeOffset(6),
            reader.GetString(7) == "active"
                ? DeviceRegistrationState.Active
                : DeviceRegistrationState.Revoked,
            reader.IsDBNull(8) ? null : reader.GetDateTimeOffset(8));
        return (registration, reader.GetInt64(9));
    }

    private static async Task WriteSecurityAuditAsync(
        SqlConnection connection,
        SqlTransaction? transaction,
        string subject,
        string registrationId,
        string eventType,
        CancellationToken cancellationToken)
    {
        string subjectHash = "sha256:" + Convert.ToHexStringLower(
            SHA256.HashData(Encoding.UTF8.GetBytes(subject)));
        await using SqlCommand audit = CreateCommand(
            connection,
            transaction,
            """
            INSERT INTO dbo.desktop_security_audit
                (subject_hash, registration_id, event_type, occurred_at)
            VALUES (@subjectHash, @registrationId, @eventType, SYSDATETIMEOFFSET());
            """);
        audit.Parameters.AddWithValue("@subjectHash", subjectHash);
        audit.Parameters.AddWithValue("@registrationId", registrationId);
        audit.Parameters.AddWithValue("@eventType", eventType);
        await audit.ExecuteNonQueryAsync(cancellationToken).ConfigureAwait(false);
    }

    private static SqlCommand CreateCommand(
        SqlConnection connection,
        SqlTransaction? transaction,
        string commandText)
    {
        SqlCommand command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandTimeout = SqlConnectionFactory.CommandTimeoutSeconds;
        command.CommandText = commandText;
        return command;
    }

    internal static bool IsDuplicateKey(SqlException exception) =>
        exception.Errors
            .OfType<SqlError>()
            .Any(static error => error.Number is 2627 or 2601);
}
