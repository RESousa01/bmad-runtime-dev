using Sapphirus.DesktopSupportApi.Sql;
using Xunit;

namespace Sapphirus.DesktopSupportApi.Tests.Sql;

/// <summary>
/// Durable streaming reservations against the real schema.
///
/// <para>
/// The store shares <c>dbo.desktop_model_call_idempotency</c> with the
/// structured path, so these tests also pin the two consequences of that
/// choice: one key can never fund both a structured call and a stream, and no
/// migration beyond <c>0001_support_authority.sql</c> is needed for streaming to
/// be single use.
/// </para>
/// </summary>
public sealed class SqlModelStreamIdempotencyTests(LocalDbFixture fixture)
    : IClassFixture<LocalDbFixture>
{
    private const string ReceiptId = "receipt_01J00000000000000000000000";
    private const string Hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string OtherHash =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task A_free_key_is_reserved_once_and_replays_after_it_completes()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        ModelStreamReservation reserved = await store.ReserveAsync(
            subject,
            "stream-key",
            Hash,
            Ct);
        Assert.Equal(ModelStreamReservationState.Reserved, reserved.State);

        await store.CompleteAsync(
            subject,
            "stream-key",
            new ModelCallCompletionMarker(ReceiptId, Hash, OtherHash),
            Ct);

        ModelStreamReservation replay = await store.ReserveAsync(
            subject,
            "stream-key",
            Hash,
            Ct);
        Assert.Equal(ModelStreamReservationState.AlreadyCompleted, replay.State);
        Assert.Equal(ReceiptId, replay.PriorCompletion!.ReceiptId);
        Assert.Equal(OtherHash, replay.PriorCompletion.ResultHash);
    }

    /// <summary>
    /// The claim is never released. A run that ended without recording an
    /// outcome spent its consent, so the key is terminally uncertain rather than
    /// reservable again.
    /// </summary>
    [Fact]
    public async Task An_uncompleted_claim_is_reported_as_uncertain_and_never_reissued()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        Assert.Equal(
            ModelStreamReservationState.Reserved,
            (await store.ReserveAsync(subject, "stream-key", Hash, Ct)).State);

        for (int attempt = 0; attempt < 3; attempt++)
        {
            ModelStreamReservation retry = await store.ReserveAsync(
                subject,
                "stream-key",
                Hash,
                Ct);
            Assert.Equal(ModelStreamReservationState.Uncertain, retry.State);
            Assert.Null(retry.PriorCompletion);
        }
    }

    [Fact]
    public async Task A_key_presented_with_another_request_conflicts()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();
        await store.ReserveAsync(subject, "stream-key", Hash, Ct);

        await Assert.ThrowsAsync<IdempotencyConflictException>(() =>
            store.ReserveAsync(subject, "stream-key", OtherHash, Ct));
    }

    [Fact]
    public async Task Reservations_are_subject_partitioned()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore store = new(fixture.ConnectionFactory);
        await store.ReserveAsync(NewSubject(), "shared-key", Hash, Ct);

        Assert.Equal(
            ModelStreamReservationState.Reserved,
            (await store.ReserveAsync(NewSubject(), "shared-key", Hash, Ct)).State);
    }

    /// <summary>
    /// Exactly one replica wins the claim; the other is told the outcome is
    /// unknown rather than being handed a second reservation.
    /// </summary>
    [Fact]
    public async Task Concurrent_replicas_yield_exactly_one_reservation()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore first = new(fixture.ConnectionFactory);
        SqlModelStreamIdempotencyStore second = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        ModelStreamReservation[] outcomes = await Task.WhenAll(
            first.ReserveAsync(subject, "stream-key", Hash, Ct),
            second.ReserveAsync(subject, "stream-key", Hash, Ct));

        Assert.Single(
            outcomes,
            outcome => outcome.State == ModelStreamReservationState.Reserved);
        Assert.Single(
            outcomes,
            outcome => outcome.State == ModelStreamReservationState.Uncertain);
    }

    /// <summary>
    /// One idempotency key cannot fund both a structured call and a stream: the
    /// fingerprints differ, so the second surface conflicts.
    /// </summary>
    [Fact]
    public async Task A_structured_call_and_a_stream_cannot_share_one_key()
    {
        fixture.EnsureAvailable();
        SqlModelCallIdempotencyStore calls = new(fixture.ConnectionFactory);
        SqlModelStreamIdempotencyStore streams = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        await Assert.ThrowsAsync<ModelCallIdempotencyUncertainException>(async () =>
        {
            await streams.ReserveAsync(subject, "shared-key", Hash, Ct);
            await calls.ExecuteAsync(
                subject,
                "shared-key",
                Hash,
                _ => throw new InvalidOperationException("No provider call may happen."),
                (result, _) => Task.FromResult(result),
                Ct);
        });

        await Assert.ThrowsAsync<IdempotencyConflictException>(async () =>
        {
            await streams.ReserveAsync(subject, "other-key", Hash, Ct);
            await calls.ExecuteAsync(
                subject,
                "other-key",
                OtherHash,
                _ => throw new InvalidOperationException("No provider call may happen."),
                (result, _) => Task.FromResult(result),
                Ct);
        });
    }

    [Fact]
    public async Task Completing_a_key_that_was_never_reserved_records_nothing()
    {
        fixture.EnsureAvailable();
        SqlModelStreamIdempotencyStore store = new(fixture.ConnectionFactory);
        string subject = NewSubject();

        await store.CompleteAsync(
            subject,
            "stream-key",
            new ModelCallCompletionMarker(ReceiptId, Hash, OtherHash),
            Ct);

        Assert.Equal(
            ModelStreamReservationState.Reserved,
            (await store.ReserveAsync(subject, "stream-key", Hash, Ct)).State);
    }

    private static string NewSubject() => Guid.NewGuid().ToString("N");
}
