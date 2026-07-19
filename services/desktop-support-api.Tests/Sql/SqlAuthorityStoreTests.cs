using System.Text;
using Microsoft.Data.SqlClient;
using Sapphirus.DesktopSupportApi.Sql;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Sql;

public sealed class SqlAuthorityStoreTests(LocalDbFixture fixture)
    : IClassFixture<LocalDbFixture>
{
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string InstallationPublicKey =
        "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEbAS8_dPcjzdYutR-ZVr8kwBsm8PLq3nVCufbv0IrJY_PjRzLuCE1BsBTqhuAddhYYvXJEz8kEs03YhmxFqMgpQ";
    private const string PayloadCanary =
        "CANARY_PROMPT_OUTPUT_5f2a0e17c39b4d21";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task Registrations_are_subject_partitioned_and_ids_cannot_cross_subjects()
    {
        fixture.EnsureAvailable();
        SqlDeviceRegistry registry = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        DeviceRegistrationResponse response = await registry.RegisterAsync(
            subject,
            CreateRegistrationRequest(),
            Ct);

        DeviceRegistrationResponse replay = await registry.RegisterAsync(
            subject,
            CreateRegistrationRequest(),
            Ct);
        Assert.Equal(response.RegistrationId, replay.RegistrationId);

        Assert.NotNull(await registry.FindActiveAsync(subject, response.RegistrationId, Ct));
        Assert.Null(await registry.FindActiveAsync(
            NewSubject(),
            response.RegistrationId,
            Ct));
        Assert.Equal(
            DeviceRevocationOutcome.Unknown,
            await registry.RevokeAsync(NewSubject(), response.RegistrationId, Ct));
    }

    [Fact]
    public async Task Revocation_prevents_lease_commit_under_a_previously_acquired_lease()
    {
        fixture.EnsureAvailable();
        SqlDeviceRegistry registry = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        DeviceRegistrationResponse response = await registry.RegisterAsync(
            subject,
            CreateRegistrationRequest(),
            Ct);
        ActiveDeviceRegistration active = (await registry.FindActiveAsync(
            subject,
            response.RegistrationId,
            Ct))!;

        Assert.Equal(
            DeviceRevocationOutcome.Revoked,
            await registry.RevokeAsync(subject, response.RegistrationId, Ct));
        Assert.Equal(
            DeviceRevocationOutcome.AlreadyRevoked,
            await registry.RevokeAsync(subject, response.RegistrationId, Ct));

        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            registry.CommitLeaseIfActiveAsync(
                active,
                CreateLease(response.RegistrationId),
                Ct));
        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            registry.RegisterAsync(subject, CreateRegistrationRequest(), Ct));
    }

    [Fact]
    public async Task No_receipt_can_publish_after_revocation_and_no_content_is_stored()
    {
        fixture.EnsureAvailable();
        SqlDeviceRegistry registry = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        DeviceRegistrationResponse response = await registry.RegisterAsync(
            subject,
            CreateRegistrationRequest(),
            Ct);
        ActiveDeviceRegistration active = (await registry.FindActiveAsync(
            subject,
            response.RegistrationId,
            Ct))!;
        (ModelAccessRequest request, ModelAccessResult result) = CreateBoundModelResult(
            active.Registration);

        ModelAccessResult committed = await registry.CommitModelResultIfActiveAsync(
            active,
            request,
            result,
            request.Consent.Region,
            Ct);
        ModelAccessReceipt? stored = await registry.GetReceiptAsync(
            subject,
            committed.Receipt.ReceiptId,
            Ct);
        Assert.NotNull(stored);
        Assert.Equal(committed.Receipt.ReceiptId, stored.ReceiptId);
        Assert.Null(await registry.GetReceiptAsync(
            NewSubject(),
            committed.Receipt.ReceiptId,
            Ct));

        await registry.RevokeAsync(subject, response.RegistrationId, Ct);
        (ModelAccessRequest lateRequest, ModelAccessResult lateResult) =
            CreateBoundModelResult(active.Registration, receiptSeed: "late");
        await Assert.ThrowsAsync<DeviceRegistrationRevokedException>(() =>
            registry.CommitModelResultIfActiveAsync(
                active,
                lateRequest,
                lateResult,
                lateRequest.Consent.Region,
                Ct));
        Assert.Null(await registry.GetReceiptAsync(
            subject,
            lateResult.Receipt.ReceiptId,
            Ct));

        Assert.Empty(await FindCanaryColumnsAsync());
    }

    [Fact]
    public async Task Duplicate_consent_consumption_across_replicas_yields_one_success()
    {
        fixture.EnsureAvailable();
        SqlConsentConsumptionStore first = new(fixture.ConnectionFactory);
        SqlConsentConsumptionStore second = new(fixture.ConnectionFactory);
        ContextConsentConsumptionRequest consumption = new(
            NewSubject(),
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            Sha256("consumption-" + Guid.NewGuid().ToString("N")),
            DateTimeOffset.UtcNow);

        ContextConsentConsumption[] outcomes = await Task.WhenAll(
            first.ConsumeAsync(consumption, Ct).AsTask(),
            second.ConsumeAsync(consumption, Ct).AsTask());

        Assert.Single(outcomes, ContextConsentConsumption.Consumed);
        Assert.Single(outcomes, ContextConsentConsumption.AlreadyConsumed);
    }

    [Fact]
    public async Task Idempotency_converges_on_same_fingerprint_and_conflicts_on_another()
    {
        fixture.EnsureAvailable();
        SqlIdempotencyStore replicaOne = new(fixture.ConnectionFactory);
        SqlIdempotencyStore replicaTwo = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        int executions = 0;

        DeviceRegistrationResponse response = await replicaOne.ExecuteAsync(
            subject,
            "key-1",
            "fingerprint-1",
            () =>
            {
                executions++;
                return Task.FromResult(new DeviceRegistrationResponse(
                    "desktop-device-registration.v1",
                    "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "active",
                    DateTimeOffset.UtcNow));
            },
            Ct);
        DeviceRegistrationResponse replayed = await replicaTwo.ExecuteAsync(
            subject,
            "key-1",
            "fingerprint-1",
            () =>
            {
                executions++;
                return Task.FromResult(response with { Status = "replayed-never-used" });
            },
            Ct);

        Assert.Equal(1, executions);
        Assert.Equal(response.RegistrationId, replayed.RegistrationId);
        Assert.Equal(response.Status, replayed.Status);
        await Assert.ThrowsAsync<IdempotencyConflictException>(() =>
            replicaTwo.ExecuteAsync(
                subject,
                "key-1",
                "fingerprint-2",
                () => Task.FromResult(response),
                Ct));
    }

    [Fact]
    public async Task Failed_operations_release_the_idempotency_claim()
    {
        fixture.EnsureAvailable();
        SqlIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            store.ExecuteAsync<DeviceRegistrationResponse>(
                subject,
                "key-fail",
                "fingerprint-1",
                () => throw new InvalidOperationException("operation failed"),
                Ct));

        DeviceRegistrationResponse recovered = await store.ExecuteAsync(
            subject,
            "key-fail",
            "fingerprint-1",
            () => Task.FromResult(new DeviceRegistrationResponse(
                "desktop-device-registration.v1",
                "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
                "active",
                DateTimeOffset.UtcNow)),
            Ct);
        Assert.Equal("active", recovered.Status);
    }

    [Fact]
    public async Task Interrupted_model_calls_fail_closed_and_do_not_broaden_retry_authority()
    {
        fixture.EnsureAvailable();
        SqlModelCallIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        ModelAccessResult result = CreateUnboundModelResult();

        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            store.ExecuteAsync(
                subject,
                "model-key-1",
                "fingerprint-1",
                _ => Task.FromResult(result),
                (_, _) => throw new InvalidOperationException("commit interrupted"),
                Ct));

        await Assert.ThrowsAsync<ModelCallIdempotencyUncertainException>(() =>
            store.ExecuteAsync(
                subject,
                "model-key-1",
                "fingerprint-1",
                _ => Task.FromResult(result),
                (acquired, _) => Task.FromResult(acquired),
                Ct));
    }

    [Fact]
    public async Task Completed_model_calls_replay_only_the_marker()
    {
        fixture.EnsureAvailable();
        SqlModelCallIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        ModelAccessResult result = CreateUnboundModelResult();

        ModelCallIdempotencyResult fresh = await store.ExecuteAsync(
            subject,
            "model-key-2",
            "fingerprint-1",
            _ => Task.FromResult(result),
            (acquired, _) => Task.FromResult(acquired),
            Ct);
        Assert.NotNull(fresh.Result);
        Assert.Null(fresh.PriorCompletion);

        ModelCallIdempotencyResult replay = await store.ExecuteAsync(
            subject,
            "model-key-2",
            "fingerprint-1",
            _ => throw new InvalidOperationException("must not re-acquire"),
            (_, _) => throw new InvalidOperationException("must not re-commit"),
            Ct);
        Assert.Null(replay.Result);
        Assert.Equal(result.Receipt.ReceiptId, replay.PriorCompletion!.ReceiptId);

        Assert.Empty(await FindCanaryColumnsAsync());
    }

    [Fact]
    public async Task Store_entry_points_are_cancellation_aware()
    {
        fixture.EnsureAvailable();
        using CancellationTokenSource cancelled = new();
        await cancelled.CancelAsync();
        SqlDeviceRegistry registry = new(fixture.ConnectionFactory);
        SqlIdempotencyStore idempotency = new(fixture.ConnectionFactory);
        SqlConsentConsumptionStore consent = new(fixture.ConnectionFactory);

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            registry.RegisterAsync(NewSubject(), CreateRegistrationRequest(), cancelled.Token));
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            idempotency.ExecuteAsync(
                NewSubject(),
                "key",
                "fingerprint",
                () => Task.FromResult(1),
                cancelled.Token));
        await Assert.ThrowsAnyAsync<OperationCanceledException>(async () =>
            await consent.ConsumeAsync(
                new ContextConsentConsumptionRequest(
                    NewSubject(),
                    "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "decision_01J00000000000000000000000",
                    "request_01J00000000000000000000000",
                    Hash,
                    DateTimeOffset.UtcNow),
                cancelled.Token));
    }

    [Fact]
    public async Task Sql_text_is_parameterized_for_hostile_subject_values()
    {
        fixture.EnsureAvailable();
        SqlDeviceRegistry registry = new(fixture.ConnectionFactory);
        string hostileSubject = "sub'ject]; DROP TABLE dbo.desktop_device_registrations;--";

        DeviceRegistrationResponse response = await registry.RegisterAsync(
            hostileSubject,
            CreateRegistrationRequest(),
            Ct);

        Assert.NotNull(await registry.FindActiveAsync(
            hostileSubject,
            response.RegistrationId,
            Ct));
    }

    private static string NewSubject() => "subject-" + Guid.NewGuid().ToString("N");

    private static string Sha256(string value) => "sha256:" + Convert.ToHexStringLower(
        System.Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes(value)));

    private static DeviceRegistrationRequest CreateRegistrationRequest() => new(
        "desktop-device-registration.v1",
        InstallationPublicKey,
        RequestGuards.TryGetInstallationPublicKeyHash(InstallationPublicKey, out string hash)
            ? hash
            : throw new InvalidOperationException("test key hash"),
        "0.1.0-beta.1",
        "windows",
        "x64",
        1);

    private static SignedEntitlementLease CreateLease(string registrationId) => new(
        "desktop-entitlement-lease.v1",
        "lease_01J00000000000000000000000",
        registrationId,
        Hash,
        "windows_local",
        DateTimeOffset.UtcNow,
        DateTimeOffset.UtcNow,
        DateTimeOffset.UtcNow.AddHours(1),
        DateTimeOffset.UtcNow.AddHours(2),
        ["bmad_help"],
        Hash,
        "0.1.0",
        "lease-key-2026-07",
        "ZXhhbXBsZS1sZWFzZS1zaWduYXR1cmU");

    private async Task<IReadOnlyList<string>> FindCanaryColumnsAsync()
    {
        await using SqlConnection connection = await fixture.ConnectionFactory
            .OpenAsync(Ct)
            .ConfigureAwait(true);
        List<(string Table, string Column)> textColumns = [];
        await using (SqlCommand columns = connection.CreateCommand())
        {
            columns.CommandText =
                """
                SELECT TABLE_NAME, COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = 'dbo' AND DATA_TYPE LIKE '%char%';
                """;
            await using SqlDataReader reader = await columns.ExecuteReaderAsync(Ct);
            while (await reader.ReadAsync(Ct))
            {
                textColumns.Add((reader.GetString(0), reader.GetString(1)));
            }
        }
        List<string> matches = [];
        foreach ((string table, string column) in textColumns)
        {
            await using SqlCommand scan = connection.CreateCommand();
            scan.CommandText =
                $"SELECT COUNT(*) FROM dbo.[{table}] WHERE [{column}] LIKE @canary;";
            scan.Parameters.AddWithValue("@canary", "%" + PayloadCanary + "%");
            if ((int)(await scan.ExecuteScalarAsync(Ct))! > 0)
            {
                matches.Add($"{table}.{column}");
            }
        }
        return matches;
    }

    private static (ModelAccessRequest Request, ModelAccessResult Result)
        CreateBoundModelResult(RegisteredDevice device, string receiptSeed = "receipt")
    {
        ModelAccessRequest request = CreateModelRequest(
            device.RegistrationId,
            device.InstallationPublicKeyHash);
        string payload = "{\"summary\":\"" + PayloadCanary + "\"}";
        string payloadHash = Sha256(payload);
        DateTimeOffset now = DateTimeOffset.Parse("2026-07-16T10:00:01.000Z");
        string receiptId = "receipt_" + Convert.ToHexString(
            System.Security.Cryptography.SHA256.HashData(
                Encoding.UTF8.GetBytes(receiptSeed)))[..32];
        ModelAccessReceipt receipt = new(
            "sapphirus.model-access-receipt.v1",
            receiptId,
            request.RequestId,
            RequestGuards.Fingerprint(request),
            payloadHash,
            "windows_local",
            request.Consent.TenantHash,
            request.Consent.SubjectHash,
            request.RegistrationId,
            request.LocalEgressManifestHash,
            request.Consent.InvocationBindingHash,
            request.Consent.ConsumptionHash,
            request.Consent.ConsentEnvelopeHash,
            request.Consent.ConsentDisclosureHash,
            request.Consent.ProviderProfileHash,
            request.Consent.ModelProfileHash,
            request.Consent.ModelCapabilityHash,
            request.Consent.DeploymentHash,
            request.CanonicalOutputSchemaId,
            request.CanonicalOutputSchemaHash,
            Hash,
            Hash,
            request.RetentionMode,
            request.Consent.Region,
            request.Items.Sum(item => item.ByteCount),
            Encoding.UTF8.GetByteCount(payload),
            new ModelAccessUsage(2100, 300, 7200, "EUR"),
            0,
            [],
            "provider-request-opaque",
            now,
            now.AddSeconds(1),
            "succeeded",
            Hash,
            new ModelAccessReceiptProof(
                "support_plane_signature",
                "ES256",
                "https://support.sapphirus.example/",
                "sapphirus-desktop",
                "model-receipt-key-2026-07",
                Hash,
                "ZXhhbXBsZS1zdXBwb3J0LXBsYW5lLXNpZ25hdHVyZQ"));
        ModelAccessResult result = new(
            "desktop-model-access-result.v1",
            request.RequestId,
            request.CanonicalOutputSchemaId,
            payload,
            payloadHash,
            receipt);
        return (request, result);
    }

    private static ModelAccessResult CreateUnboundModelResult()
    {
        RegisteredDevice device = new(
            "unbound-subject",
            "dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA",
            InstallationPublicKey,
            RequestGuards.TryGetInstallationPublicKeyHash(
                InstallationPublicKey,
                out string hash)
                ? hash
                : "",
            "0.1.0-beta.1",
            "windows",
            "x64",
            1,
            DateTimeOffset.UtcNow,
            DeviceRegistrationState.Active,
            null);
        return CreateBoundModelResult(device, Guid.NewGuid().ToString("N")).Result;
    }

    private static ModelAccessRequest CreateModelRequest(
        string registrationId,
        string installationPublicKeyHash)
    {
        const string content = "review this bounded context";
        ModelContextItem item = new(
            "context-item-1",
            "src/example.cs",
            "implementation",
            "csharp",
            Sha256(content),
            Encoding.UTF8.GetByteCount(content),
            "source",
            content);
        ModelContextConsent unboundConsent = new(
            "sapphirus.model-context-consent.v1",
            "decision_01J00000000000000000000000",
            "request_01J00000000000000000000000",
            "invoke_01J00000000000000000000000",
            "windows_local",
            Hash,
            Hash,
            registrationId,
            installationPublicKeyHash,
            "lease_01J00000000000000000000000",
            Hash,
            "policy_01J00000000000000000000000",
            7,
            Hash,
            "bmad_help",
            "planner",
            "sapphirus.bmad-method-help-proposal.v1",
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            Hash,
            "westeurope",
            "transient_no_store",
            "interactive-standard",
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:00:00.000Z"),
            DateTimeOffset.Parse("2026-07-16T10:05:00.000Z"),
            Hash,
            Hash,
            new ModelContextConsentProof(
                "installation_signature",
                "ES256",
                "installation-key-2026-07",
                Hash,
                "ZXhhbXBsZS1kZXZpY2Utc2lnbmF0dXJl"));
        ModelAccessRequest request = new(
            "desktop-model-access-request.v1",
            unboundConsent.RequestId,
            "windows_local",
            registrationId,
            unboundConsent.Purpose,
            unboundConsent.ModelRole,
            unboundConsent.CanonicalOutputSchemaId,
            unboundConsent.CanonicalOutputSchemaHash,
            Hash,
            unboundConsent,
            [item],
            "transient_no_store",
            "interactive-standard");
        string manifestHash = RequestGuards.ComputeContextManifestHash(request);
        return request with
        {
            LocalEgressManifestHash = manifestHash,
            Consent = request.Consent with { ManifestHash = manifestHash },
        };
    }
}
