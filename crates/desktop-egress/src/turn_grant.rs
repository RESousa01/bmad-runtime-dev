//! Turn-scoped egress accounting (D7).
//!
//! ADR-0001 made every model call individually consented, which worked while a
//! turn *was* one call. ADR-0009 lets a turn loop — the model asks for a tool,
//! the host runs it, the result re-enters the transcript, and the model is
//! called again. That breaks the old shape in two different ways, and only one
//! of them is about prompting.
//!
//! # Why a decision is not reused
//!
//! Two separate reasons, and it is worth not confusing them.
//!
//! *Mechanically*, reuse is already impossible: `consume` moves a decision from
//! `Pending` to `Consumed` and a second attempt returns
//! `DecisionAlreadyConsumed`, and `PendingContextDecision` is deliberately not
//! `Clone`. That is what enforces single use — not anything in this module.
//!
//! *Semantically*, reuse would be wrong even if it were possible. A decision is
//! bound to a manifest hash, and that hash covers the manifest's items —
//! including the transcript, which changes every iteration as tool calls and
//! results join it. (It changes rather than merely grows: `prompt_window` is
//! byte-bounded and slides once full. The per-call `created_at`/`expires_at` in
//! the manifest would diverge regardless.) So a decision minted for iteration 1
//! describes context that iteration 2 is no longer sending, and honouring it
//! there would turn "the user approved this context" into "the user approved
//! something, once, a while ago".
//!
//! # What this does instead
//!
//! The human approves a **turn**, with bounds. Every call inside it still mints
//! and consumes its own correctly-bound, single-use decision — the existing
//! machinery is untouched. This type adds the layer above: it enforces the
//! bounds the human agreed to, and records what each call actually sent so the
//! turn's total egress can be answered afterwards rather than estimated.
//!
//! ADR-0009 re-bought ADR-0001's *authorisation* half and dropped its
//! *accounting* half. This is the accounting half.

use desktop_runtime::{ContractId, Sha256Digest, UnixMillis};

use crate::EgressError;

/// Hard ceiling on model calls in one turn, independent of what a caller asks
/// for. A loop that has not converged in this many calls is not going to.
pub const MAX_TURN_MODEL_CALLS: u32 = 16;

/// One model call that actually happened, with what it sent.
///
/// Recorded after the call is authorised, so the ledger reflects egress that
/// occurred rather than egress that was planned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnEgressEntry {
    pub manifest_hash: Sha256Digest,
    pub item_count: u32,
    pub context_bytes: u64,
    pub at: UnixMillis,
}

/// The outcome of writing one call into the ledger.
///
/// A ledger that refuses to record something which already happened is worse
/// than useless — it reports less egress than occurred. So recording never
/// fails; it reports whether the call was inside the bounds the human granted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRecord {
    WithinGrant,
    /// The call happened outside the grant — the window closed while it was in
    /// flight, or the budget was already spent. It is on the ledger regardless,
    /// because the bytes left the machine.
    Overrun,
}

/// One human approval, covering every model call in a single turn.
///
/// Deliberately **not** `Clone`, matching
/// [`PendingContextDecision`](crate::PendingContextDecision) and
/// [`DecisionConsumption`](crate::DecisionConsumption). A clonable grant is not
/// a budget: copying one with fifteen calls spent would yield a fresh copy with
/// fifteen spent, and the ceiling would become a convention the caller may
/// maintain rather than a property the type enforces.
///
/// ```compile_fail
/// fn duplicate(grant: desktop_egress::TurnEgressGrant) {
///     let second_budget = grant.clone();
/// }
/// ```
#[derive(Debug)]
pub struct TurnEgressGrant {
    turn_id: ContractId,
    granted_at: UnixMillis,
    expires_at: UnixMillis,
    max_calls: u32,
    calls: Vec<TurnEgressEntry>,
    overruns: u32,
}

impl TurnEgressGrant {
    /// Opens a grant for one turn.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::InvalidTurnGrant`] when the window is empty or
    /// backwards, or when no calls are permitted. `max_calls` is clamped down
    /// to [`MAX_TURN_MODEL_CALLS`]; a caller cannot ask for more.
    pub fn open(
        turn_id: ContractId,
        granted_at: UnixMillis,
        expires_at: UnixMillis,
        max_calls: u32,
    ) -> Result<Self, EgressError> {
        if expires_at.0 <= granted_at.0 || max_calls == 0 {
            return Err(EgressError::InvalidTurnGrant);
        }
        Ok(Self {
            turn_id,
            granted_at,
            expires_at,
            max_calls: max_calls.min(MAX_TURN_MODEL_CALLS),
            calls: Vec::new(),
            overruns: 0,
        })
    }

    /// Checks that one more model call is within the bounds the human granted.
    ///
    /// Call this *before* minting the per-call decision, so an out-of-bounds
    /// iteration never reaches the provider.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::TurnGrantExpired`] once the window has closed and
    /// [`EgressError::TurnGrantExhausted`] once the call budget is spent.
    pub fn authorize_call(&self, at: UnixMillis) -> Result<(), EgressError> {
        if at.0 > self.expires_at.0 || at.0 < self.granted_at.0 {
            return Err(EgressError::TurnGrantExpired);
        }
        if self.calls_made() >= self.max_calls {
            return Err(EgressError::TurnGrantExhausted);
        }
        Ok(())
    }

    /// Records a call that was **sent**, and never refuses.
    ///
    /// Egress is caused by the send, not by the send succeeding, and not by the
    /// send being within budget. The obvious-looking alternative — re-check the
    /// bounds and reject — loses exactly the entries that matter most: a call
    /// authorised at t=59s on a sixty-second grant, whose round trip returns at
    /// t=62s, has already put bytes on the wire. Dropping it would make the
    /// ledger under-report precisely when a turn misbehaved.
    ///
    /// The budget is enforced by [`Self::authorize_call`], *before* the send.
    /// This is the record of what happened afterwards.
    pub fn record_call(&mut self, entry: TurnEgressEntry) -> CallRecord {
        let within = self.authorize_call(entry.at).is_ok();
        self.calls.push(entry);
        if within {
            CallRecord::WithinGrant
        } else {
            self.overruns = self.overruns.saturating_add(1);
            CallRecord::Overrun
        }
    }

    /// How many recorded calls fell outside the granted bounds. Non-zero means
    /// the turn sent more, or later, than the human approved — which the user
    /// is entitled to see rather than have quietly dropped.
    #[must_use]
    pub const fn overruns(&self) -> u32 {
        self.overruns
    }

    #[must_use]
    pub fn turn_id(&self) -> &ContractId {
        &self.turn_id
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.expires_at
    }

    /// What this turn has sent so far, oldest first. This is the ledger the
    /// user is entitled to see.
    #[must_use]
    pub fn entries(&self) -> &[TurnEgressEntry] {
        &self.calls
    }

    #[must_use]
    pub fn calls_made(&self) -> u32 {
        // `calls` can never exceed `max_calls`, which is a u32.
        u32::try_from(self.calls.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn calls_remaining(&self) -> u32 {
        self.max_calls.saturating_sub(self.calls_made())
    }

    /// Total bytes of **reviewed context** this turn has sent.
    ///
    /// Not the size of the HTTP bodies: the request envelope, the instruction
    /// block and the provenance fence are all excluded, so the real number on
    /// the wire is somewhat larger. This is a lower bound on egress, and should
    /// be described as reviewed-context bytes rather than "what left the
    /// machine".
    ///
    /// Summed across calls without deduplication: the transcript is re-sent
    /// each iteration, and for an egress ledger what left the machine is the
    /// honest number, not what was distinct.
    #[must_use]
    pub fn total_context_bytes(&self) -> u64 {
        self.calls.iter().fold(0_u64, |total, entry| {
            total.saturating_add(entry.context_bytes)
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{CallRecord, TurnEgressEntry, TurnEgressGrant, MAX_TURN_MODEL_CALLS};
    use crate::EgressError;
    use desktop_runtime::{sha256_bytes, ContractId, UnixMillis};

    fn grant(max_calls: u32) -> TurnEgressGrant {
        TurnEgressGrant::open(
            ContractId::new("turn_01").expect("turn id"),
            UnixMillis(1_000),
            UnixMillis(61_000),
            max_calls,
        )
        .expect("grant")
    }

    fn entry(at: u64, bytes: u64) -> TurnEgressEntry {
        TurnEgressEntry {
            manifest_hash: sha256_bytes(format!("manifest-{at}").as_bytes()),
            item_count: 2,
            context_bytes: bytes,
            at: UnixMillis(at),
        }
    }

    #[test]
    fn a_turn_spends_its_call_budget_and_then_refuses() {
        let mut grant = grant(3);
        assert_eq!(grant.calls_remaining(), 3);

        for index in 0..3 {
            assert_eq!(
                grant.record_call(entry(1_100 + index, 512)),
                CallRecord::WithinGrant
            );
        }
        assert_eq!(grant.calls_made(), 3);
        assert_eq!(grant.calls_remaining(), 0);

        // The gate refuses the fourth call — this is what stops it being sent.
        assert_eq!(
            grant.authorize_call(UnixMillis(1_200)),
            Err(EgressError::TurnGrantExhausted)
        );
        // But if one is sent anyway, the ledger records it as an overrun rather
        // than pretending it did not happen.
        assert_eq!(grant.record_call(entry(1_200, 512)), CallRecord::Overrun);
        assert_eq!(grant.overruns(), 1);
        assert_eq!(grant.total_context_bytes(), 4 * 512);
    }

    #[test]
    fn a_turn_that_outlives_its_window_stops_sending() {
        let mut grant = grant(8);
        assert_eq!(
            grant.record_call(entry(1_100, 128)),
            CallRecord::WithinGrant
        );

        assert_eq!(
            grant.authorize_call(UnixMillis(61_001)),
            Err(EgressError::TurnGrantExpired)
        );
    }

    #[test]
    fn a_call_still_in_flight_when_the_window_closes_is_recorded_not_dropped() {
        // The case that makes "reject on record" wrong: authorised just inside
        // the window, returning just outside it. The bytes are already gone.
        let mut grant = grant(8);
        grant
            .authorize_call(UnixMillis(60_999))
            .expect("authorised inside the window");

        assert_eq!(grant.record_call(entry(61_500, 4_096)), CallRecord::Overrun);
        assert_eq!(grant.calls_made(), 1);
        assert_eq!(grant.overruns(), 1);
        assert_eq!(
            grant.total_context_bytes(),
            4_096,
            "a ledger must not under-report egress that actually happened"
        );
    }

    #[test]
    fn no_caller_can_grant_itself_more_than_the_hard_ceiling() {
        let grant = grant(u32::MAX);
        assert_eq!(grant.calls_remaining(), MAX_TURN_MODEL_CALLS);
    }

    #[test]
    fn the_ledger_totals_every_call_without_deduplication() {
        let mut grant = grant(4);
        grant.record_call(entry(1_100, 1_000));
        grant.record_call(entry(1_200, 2_500));

        // The transcript is re-sent each iteration, so the honest total is the
        // sum, not the distinct content.
        assert_eq!(grant.total_context_bytes(), 3_500);
        assert_eq!(grant.entries().len(), 2);
        assert_eq!(grant.entries()[0].context_bytes, 1_000);
    }

    #[test]
    fn a_call_predating_the_grant_is_refused_before_it_is_sent() {
        // Symmetry with expiry: the gate rejects a nonsensical clock up front,
        // which is where a bad timestamp can still be stopped for free.
        let grant = grant(4);
        assert_eq!(
            grant.authorize_call(UnixMillis(900)),
            Err(EgressError::TurnGrantExpired)
        );
    }

    #[test]
    fn a_grant_with_no_window_or_no_calls_is_refused() {
        let turn = ContractId::new("turn_02").expect("turn id");
        assert_eq!(
            TurnEgressGrant::open(turn.clone(), UnixMillis(1_000), UnixMillis(1_000), 4).err(),
            Some(EgressError::InvalidTurnGrant)
        );
        assert_eq!(
            TurnEgressGrant::open(turn.clone(), UnixMillis(2_000), UnixMillis(1_000), 4).err(),
            Some(EgressError::InvalidTurnGrant)
        );
        assert_eq!(
            TurnEgressGrant::open(turn, UnixMillis(1_000), UnixMillis(2_000), 0).err(),
            Some(EgressError::InvalidTurnGrant)
        );
    }
}
