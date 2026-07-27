use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use desktop_runtime::{canonical_hash, ContractId, Sha256Digest, UnixMillis};
use serde::Serialize;

use crate::{IpcValidationError, ValidatedCommandEnvelope};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    New,
    Replay,
}

#[derive(Clone, Copy, Debug)]
pub struct AdmissionPolicy {
    pub max_requests_per_window: usize,
    pub window_ms: u64,
    pub max_tracked_sessions: usize,
    pub max_tracked_mutations: usize,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            max_requests_per_window: 120,
            window_ms: 60_000,
            max_tracked_sessions: 32,
            max_tracked_mutations: 2_048,
        }
    }
}

#[derive(Debug)]
struct GateState {
    session_requests: HashMap<ContractId, VecDeque<u64>>,
    session_order: VecDeque<ContractId>,
    mutation_identities: HashMap<ContractId, MutationIdentity>,
    mutation_order: VecDeque<ContractId>,
}

#[derive(Debug, Eq, PartialEq)]
enum MutationIdentity {
    DurableHelpRun,
    OneShot,
    Fingerprint(Sha256Digest),
}

/// In-memory abuse and short-retry guard. The runtime/store remains responsible
/// for durable request-id receipts across eviction and process restart.
pub struct RequestGate {
    policy: AdmissionPolicy,
    state: Mutex<GateState>,
}

impl RequestGate {
    #[must_use]
    pub fn new(policy: AdmissionPolicy) -> Self {
        Self {
            policy,
            state: Mutex::new(GateState {
                session_requests: HashMap::new(),
                session_order: VecDeque::new(),
                mutation_identities: HashMap::new(),
                mutation_order: VecDeque::new(),
            }),
        }
    }

    /// Applies rate limiting and short-lived mutation admission checks.
    /// `run.create` records only command-family occupancy: its same-family
    /// replay and conflict authority remains the durable local store, including
    /// after renderer rebind or process restart.
    ///
    /// # Errors
    ///
    /// Returns [`IpcValidationError`] when policy or state is unavailable, the
    /// session exceeds its rate limit, the request identifier conflicts with a
    /// prior mutation, or the command cannot be fingerprinted.
    pub fn admit(
        &self,
        envelope: &ValidatedCommandEnvelope,
        now: UnixMillis,
    ) -> Result<Admission, IpcValidationError> {
        if self.policy.max_requests_per_window == 0
            || self.policy.window_ms == 0
            || self.policy.max_tracked_sessions == 0
            || self.policy.max_tracked_mutations == 0
        {
            return Err(IpcValidationError::AdmissionUnavailable);
        }

        let mutation_identity = if !envelope.command.requires_request_tracking() {
            None
        } else if matches!(
            &envelope.command,
            desktop_runtime::LocalCommand::CreateBmadHelpRun { .. }
        ) {
            Some(MutationIdentity::DurableHelpRun)
        } else if matches!(
            &envelope.command,
            desktop_runtime::LocalCommand::ModelAuthSignIn { .. }
                | desktop_runtime::LocalCommand::ModelAuthSignOut
                | desktop_runtime::LocalCommand::PrepareBmadHelpReview { .. }
                | desktop_runtime::LocalCommand::ApproveBmadHelpReview { .. }
                | desktop_runtime::LocalCommand::CancelBmadHelpReview { .. }
                | desktop_runtime::LocalCommand::SubmitBmadHelpReview { .. }
                | desktop_runtime::LocalCommand::DecideChangesRecovery { .. }
        ) {
            Some(MutationIdentity::OneShot)
        } else {
            Some(MutationIdentity::Fingerprint(command_fingerprint(
                envelope,
            )?))
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| IpcValidationError::AdmissionUnavailable)?;
        let cutoff = now.0.saturating_sub(self.policy.window_ms);
        if !state
            .session_requests
            .contains_key(&envelope.renderer_session_id)
        {
            while state.session_requests.len() >= self.policy.max_tracked_sessions {
                let Some(evicted) = state.session_order.pop_front() else {
                    return Err(IpcValidationError::AdmissionUnavailable);
                };
                state.session_requests.remove(&evicted);
            }
            state
                .session_order
                .push_back(envelope.renderer_session_id.clone());
        }
        let requests = state
            .session_requests
            .entry(envelope.renderer_session_id.clone())
            .or_default();
        // Eviction below is front-only, which is correct exactly while the
        // recorded samples are non-decreasing — and `now` is the host's wall
        // clock (`SystemTime`), which an NTP correction, a manual change, or a
        // resume from sleep can step backwards by an arbitrary amount. A
        // backwards step used to append a smaller sample behind a larger one,
        // after which the large front value was never below the cutoff, nothing
        // was ever evicted, the window filled, and the session stayed
        // rate-limited until the real clock caught up — for a step of days or
        // years, until the process restarted.
        //
        // Rebasing rather than dropping: these requests really were made, so a
        // clock correction must not buy a fresh window. They stay counted for
        // one more full window, measured from the corrected clock, and the
        // ordering the eviction below depends on is restored.
        if requests.back().is_some_and(|latest| *latest > now.0) {
            for issued in &mut *requests {
                *issued = now.0;
            }
        }
        while requests.front().is_some_and(|issued| *issued < cutoff) {
            requests.pop_front();
        }
        if requests.len() >= self.policy.max_requests_per_window {
            return Err(IpcValidationError::RateLimited);
        }
        requests.push_back(now.0);

        let Some(mutation_identity) = mutation_identity else {
            return Ok(Admission::New);
        };

        if let Some(previous) = state.mutation_identities.get(&envelope.request_id) {
            return match (previous, &mutation_identity) {
                (MutationIdentity::DurableHelpRun, MutationIdentity::DurableHelpRun) => {
                    Ok(Admission::New)
                }
                (MutationIdentity::Fingerprint(previous), MutationIdentity::Fingerprint(next))
                    if previous == next =>
                {
                    Ok(Admission::Replay)
                }
                _ => Err(IpcValidationError::IdempotencyConflict),
            };
        }

        while state.mutation_order.len() >= self.policy.max_tracked_mutations {
            if let Some(evicted) = state.mutation_order.pop_front() {
                state.mutation_identities.remove(&evicted);
            }
        }
        state
            .mutation_identities
            .insert(envelope.request_id.clone(), mutation_identity);
        state.mutation_order.push_back(envelope.request_id.clone());
        Ok(Admission::New)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandFingerprint<'a> {
    renderer_session_id: &'a ContractId,
    installation_id: &'a ContractId,
    issued_at: UnixMillis,
    command: &'a desktop_runtime::LocalCommand,
}

fn command_fingerprint(
    envelope: &ValidatedCommandEnvelope,
) -> Result<Sha256Digest, IpcValidationError> {
    canonical_hash(
        "desktop-ipc-command",
        1,
        &CommandFingerprint {
            renderer_session_id: &envelope.renderer_session_id,
            installation_id: &envelope.installation_id,
            issued_at: envelope.issued_at,
            command: &envelope.command,
        },
    )
    .map_err(|_| IpcValidationError::InvalidPayload)
}

#[cfg(test)]
mod tests {
    use desktop_runtime::{BmadHelpIntent, ContractId, LocalCommand, UnixMillis};

    use super::{Admission, AdmissionPolicy, RequestGate};
    use crate::{IpcValidationError, ValidatedCommandEnvelope};

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn envelope(
        command: LocalCommand,
    ) -> Result<ValidatedCommandEnvelope, Box<dyn std::error::Error>> {
        Ok(ValidatedCommandEnvelope {
            request_id: id("req_001")?,
            window_label: "main".to_owned(),
            renderer_session_id: id("rs_test")?,
            installation_id: id("install_test")?,
            issued_at: UnixMillis(1_000),
            command,
        })
    }

    #[test]
    fn identical_mutation_is_an_idempotent_replay() -> Result<(), Box<dyn std::error::Error>> {
        let gate = RequestGate::new(AdmissionPolicy::default());
        let request = envelope(LocalCommand::SelectWorkspace)?;
        assert_eq!(gate.admit(&request, UnixMillis(1_000))?, Admission::New);
        assert_eq!(gate.admit(&request, UnixMillis(1_001))?, Admission::Replay);
        Ok(())
    }

    #[test]
    fn changed_mutation_under_same_request_id_is_rejected() -> Result<(), Box<dyn std::error::Error>>
    {
        let gate = RequestGate::new(AdmissionPolicy::default());
        let first = envelope(LocalCommand::SelectWorkspace)?;
        let second = envelope(LocalCommand::RevokeWorkspace {
            workspace_id: id("workspace_1")?,
        })?;
        assert_eq!(gate.admit(&first, UnixMillis(1_000))?, Admission::New);
        assert!(gate.admit(&second, UnixMillis(1_001)).is_err());
        Ok(())
    }

    #[test]
    fn help_run_creation_is_rate_limited_but_never_short_circuits_durable_store_admission(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gate = RequestGate::new(AdmissionPolicy::default());
        let first = envelope(LocalCommand::CreateBmadHelpRun {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 1,
            current_intent: BmadHelpIntent::new("find the next Method step")?,
        })?;
        let mut rebound = envelope(LocalCommand::CreateBmadHelpRun {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 1,
            current_intent: BmadHelpIntent::new("changed intent reaches durable conflict logic")?,
        })?;
        rebound.renderer_session_id = id("rs_rebound")?;
        rebound.issued_at = UnixMillis(2_000);

        assert_eq!(gate.admit(&first, UnixMillis(1_000))?, Admission::New);
        assert_eq!(gate.admit(&first, UnixMillis(1_001))?, Admission::New);
        assert_eq!(gate.admit(&rebound, UnixMillis(2_000))?, Admission::New);
        Ok(())
    }

    #[test]
    fn help_run_request_ids_cannot_alias_another_mutation_family(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let help = envelope(LocalCommand::CreateBmadHelpRun {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 1,
            current_intent: BmadHelpIntent::new("find the next Method step")?,
        })?;
        let revoke = envelope(LocalCommand::RevokeWorkspace {
            workspace_id: id("workspace_1")?,
        })?;

        let help_first = RequestGate::new(AdmissionPolicy::default());
        assert_eq!(help_first.admit(&help, UnixMillis(1_000))?, Admission::New);
        assert!(help_first.admit(&revoke, UnixMillis(1_001)).is_err());

        let revoke_first = RequestGate::new(AdmissionPolicy::default());
        assert_eq!(
            revoke_first.admit(&revoke, UnixMillis(1_000))?,
            Admission::New
        );
        assert!(revoke_first.admit(&help, UnixMillis(1_001)).is_err());
        Ok(())
    }

    #[test]
    fn recovery_prepare_is_fingerprinted_even_though_it_has_no_file_effect(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gate = RequestGate::new(AdmissionPolicy::default());
        let first = envelope(LocalCommand::PrepareChangesRecovery {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 7,
            journal_id: id("journal_1")?,
        })?;
        let changed = envelope(LocalCommand::PrepareChangesRecovery {
            workspace_id: id("workspace_1")?,
            workspace_grant_epoch: 8,
            journal_id: id("journal_1")?,
        })?;

        assert_eq!(gate.admit(&first, UnixMillis(1_000))?, Admission::New);
        assert_eq!(gate.admit(&first, UnixMillis(1_001))?, Admission::Replay);
        assert!(gate.admit(&changed, UnixMillis(1_002)).is_err());
        Ok(())
    }

    /// The rate limit, asserted rather than assumed.
    ///
    /// `IpcValidationError::RateLimited` was not named by a single test in the
    /// repository, `max_requests_per_window` was never overridden, and the
    /// whole limiting block could be deleted with the suite green — including
    /// the one test whose name claims this property.
    ///
    /// A read-only command is used deliberately:
    /// `requires_request_tracking` is false for it, so the rate limit is the
    /// only thing this can be refused by.
    #[test]
    fn a_session_is_refused_once_it_fills_its_window() -> Result<(), Box<dyn std::error::Error>> {
        let gate = RequestGate::new(AdmissionPolicy {
            max_requests_per_window: 3,
            window_ms: 1_000,
            ..AdmissionPolicy::default()
        });
        let request = envelope(LocalCommand::GetBootState)?;

        for tick in 0..3 {
            assert_eq!(
                gate.admit(&request, UnixMillis(1_000 + tick))?,
                Admission::New,
            );
        }
        assert!(matches!(
            gate.admit(&request, UnixMillis(1_003)),
            Err(IpcValidationError::RateLimited),
        ));

        // A window, not a quota: it reopens once it has passed.
        assert_eq!(gate.admit(&request, UnixMillis(2_100))?, Admission::New);
        Ok(())
    }

    /// The window is measured on whatever clock the caller samples. The host
    /// samples `SystemTime` (`desktop_app::state::now`), which an NTP
    /// correction, a manual change, or a resume from sleep can step backwards
    /// by an arbitrary amount.
    ///
    /// Eviction is front-only. Before the rebase in `admit`, a backwards step
    /// appended a smaller sample behind a larger one; from then on the large
    /// front value was never below the cutoff, so nothing was ever evicted, the
    /// window filled, and every later request from that session was refused —
    /// for a step of days or years, until the process restarted. The last
    /// assertion is the one that fails on that code.
    #[test]
    fn a_backwards_clock_step_does_not_permanently_rate_limit_a_session(
    ) -> Result<(), Box<dyn std::error::Error>> {
        /// Far enough ahead of the corrected clock below that "wait for real
        /// time to catch up" is not a recovery any user would ever see.
        const CLOCK_FAR_AHEAD: u64 = 1_700_000_000_000;

        let gate = RequestGate::new(AdmissionPolicy {
            max_requests_per_window: 3,
            window_ms: 60_000,
            ..AdmissionPolicy::default()
        });
        let request = envelope(LocalCommand::GetBootState)?;

        // One request while the clock is a year fast, then the correction.
        assert_eq!(
            gate.admit(&request, UnixMillis(CLOCK_FAR_AHEAD))?,
            Admission::New
        );
        for tick in 0..2 {
            assert_eq!(
                gate.admit(&request, UnixMillis(1_000 + tick))?,
                Admission::New,
            );
        }

        // Still enforced across the correction: three requests really were
        // made, so the fourth inside the window is still refused. A clock
        // correction is not a way to buy a fresh window.
        assert!(matches!(
            gate.admit(&request, UnixMillis(1_002)),
            Err(IpcValidationError::RateLimited),
        ));

        // And the session recovers after one window rather than after a year.
        assert_eq!(gate.admit(&request, UnixMillis(61_001))?, Admission::New);
        Ok(())
    }

    /// The session table is bounded, and the bound is kept by evicting the
    /// oldest session. Neither of the gate's two eviction branches was ever
    /// executed by a test, so both were free to be wrong.
    #[test]
    fn evicting_the_oldest_session_forgets_its_window() -> Result<(), Box<dyn std::error::Error>> {
        let gate = RequestGate::new(AdmissionPolicy {
            max_requests_per_window: 1,
            max_tracked_sessions: 2,
            ..AdmissionPolicy::default()
        });
        let mut first = envelope(LocalCommand::GetBootState)?;
        first.renderer_session_id = id("rs_first")?;

        assert_eq!(gate.admit(&first, UnixMillis(1_000))?, Admission::New);
        assert!(matches!(
            gate.admit(&first, UnixMillis(1_001)),
            Err(IpcValidationError::RateLimited),
        ));

        // Two further sessions push the first one out of the table.
        for session in ["rs_second", "rs_third"] {
            let mut other = envelope(LocalCommand::GetBootState)?;
            other.renderer_session_id = id(session)?;
            assert_eq!(gate.admit(&other, UnixMillis(1_002))?, Admission::New);
        }

        // Its window is forgotten, which is what being evicted means. This
        // table is an in-memory abuse guard; the durable store remains the
        // authority for anything that has to survive eviction or a restart.
        assert_eq!(gate.admit(&first, UnixMillis(1_003))?, Admission::New);
        Ok(())
    }

    /// The mutation table is bounded the same way, and evicting an identity is
    /// exactly what makes its request id admissible again.
    #[test]
    fn evicting_the_oldest_mutation_frees_its_request_id() -> Result<(), Box<dyn std::error::Error>>
    {
        let gate = RequestGate::new(AdmissionPolicy {
            max_tracked_mutations: 2,
            ..AdmissionPolicy::default()
        });
        let mut first = envelope(LocalCommand::SelectWorkspace)?;
        first.request_id = id("req_first")?;
        let mut conflicting = envelope(LocalCommand::RevokeWorkspace {
            workspace_id: id("workspace_1")?,
        })?;
        conflicting.request_id = id("req_first")?;

        assert_eq!(gate.admit(&first, UnixMillis(1_000))?, Admission::New);
        // A different command under the same id conflicts while the identity is
        // still tracked.
        assert!(matches!(
            gate.admit(&conflicting, UnixMillis(1_001)),
            Err(IpcValidationError::IdempotencyConflict),
        ));

        for request_id in ["req_second", "req_third"] {
            let mut other = envelope(LocalCommand::SelectWorkspace)?;
            other.request_id = id(request_id)?;
            assert_eq!(gate.admit(&other, UnixMillis(1_002))?, Admission::New);
        }

        // The identity is gone, so the id reads as new rather than as a
        // conflict. Same caveat as above: replay authority across eviction is
        // the durable store's, not this table's.
        assert_eq!(gate.admit(&conflicting, UnixMillis(1_003))?, Admission::New);
        Ok(())
    }
}
