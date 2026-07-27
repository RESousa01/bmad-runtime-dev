//! Generic reviewed model-capability lifecycle (readiness Task 6).
//!
//! One coordinator drives the D2 lifecycle — prepare, one reviewed
//! decision, single-use consumption, transport dispatch, output
//! verification, durable capability-run persistence — for every ADR-0005
//! capability. Every stage is bound to one closure-ledger capability
//! identity: a manifest, decision, or output minted for capability A can
//! never be replayed against capability B, and a result whose archetype
//! differs from the run's declared output schema terminates the flow.

use std::collections::BTreeMap;

use desktop_cloud::{
    AuthorizedModelRequest, DispatchedModelRequest, ProviderFailure, RawModelOutput,
};
use desktop_egress::{
    ApproveDecisionInput, CancelDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextEgressManifest, EgressError, MemoryDecisionLedger, ModelInvocationBinding,
    PendingContextDecision,
};
use desktop_runtime::{
    canonical_hash, BmadCapabilityOutput, BmadCapabilityRun, BmadCapabilityRunParams,
    BmadClosureCapabilityId, ContractId, Sha256Digest, UnixMillis,
};
use desktop_store::LocalStore;
use serde::Serialize;

use super::context::derived_contract_id;

/// Maximum decision validity after approval, mirroring the Help lifecycle.
const DECISION_WINDOW_MS: u64 = 5 * 60 * 1_000;

/// How many workspaces may hold a capability slot at once.
///
/// Slots are keyed by workspace and a workspace whose caller never finalises
/// its run keeps its slot for the life of the process, so without a ceiling the
/// map would grow with every workspace ever opened. Live slots are never
/// evicted — that would admit a second concurrent run where the lifecycle
/// refuses one — so at capacity the only reclaimable slots are the inert
/// `Terminal` ones, and a workspace with no slot is refused outright when even
/// those are gone. Far above any plausible number of workspaces a person has
/// open at once.
pub(crate) const MAX_TRACKED_CAPABILITY_WORKSPACES: usize = 32;

/// Transport seam shared by deterministic, offline, and (later) production
/// capability dispatch. Mirrors the Help transport so Task 9 can supply the
/// deployed round trip once.
///
/// The failure type is [`ProviderFailure`], not `CloudError`, and that is the
/// whole point of this seam now. `CloudError::TransportFailed` is one value for
/// every way a provider can refuse — a rate limit, a dead key, a spent quota, a
/// 500 — so a caller holding one could only ever say something generic, and what
/// it said was *"Connect a model provider in Settings"* to users whose provider
/// was connected and working. A `ProviderFailure` carries the class alongside
/// the exact `CloudError` this lane already read, so `CloudError::from(failure)`
/// still answers what it always answered and nothing downstream loses a fact.
pub(crate) trait BmadCapabilityTransport: Send + Sync {
    /// # Errors
    ///
    /// Returns the classified [`ProviderFailure`]. It carries no provider text
    /// — see `desktop_cloud::provider_failure` — so it is safe to log whole.
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), ProviderFailure>;
}

/// Emits the one line a failed send now leaves behind.
///
/// `RUST_LOG=debug` produces nothing from this binary because there is no
/// logging facility in it at all — no `tracing`, no `log`, no `env_logger`. A
/// failed turn therefore left no trace anywhere: Activity showed *"No activity
/// yet"*, stdout and stderr were empty, and the only evidence anyone had was the
/// sentence on screen, which was the same sentence whatever had happened.
///
/// Three fields, all of them host-authored or a number off the wire: the stable
/// diagnostic token, the HTTP status, and the clamped retry delay. The
/// provider's body, the request, and every header are deliberately absent — an
/// error body routinely echoes the request, which for this app is reviewed
/// workspace content, and that is exactly what must not reach a log file. This
/// is `eprintln!` rather than a logging crate on purpose: one line at one call
/// site does not justify a dependency, and a dependency would invite the
/// formatting seam this deliberately does not have.
pub(crate) fn note_provider_failure(lane: &str, failure: &ProviderFailure) {
    let status = failure
        .status()
        .map_or_else(|| "none".to_owned(), |status| status.to_string());
    let retry_after_ms = failure
        .retry_after()
        .map_or_else(|| "none".to_owned(), |delay| delay.as_millis().to_string());
    eprintln!(
        "forge {lane}: send failed code={} status={status} retry_after_ms={retry_after_ms}",
        failure.diagnostic_code()
    );
}

/// Verifies raw model output into exactly one sealed capability output.
/// Implementations must treat the raw output as untrusted data.
pub(crate) trait BmadCapabilityOutputVerifier {
    fn verify(
        &self,
        capability_id: &BmadClosureCapabilityId,
        output: &RawModelOutput,
    ) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BmadCapabilityCoordinatorError {
    Unauthorized,
    Conflict,
    CapabilityBindingMismatch,
    ConsentBindingMismatch,
    ConsentExpired,
    ConsentAlreadyConsumed,
    OutputRejected,
    ResultArchetypeMismatch,
    Transport,
    Store,
    Integrity,
}

/// The egress-safe purpose label for one capability: `bmm:x` -> `bmm.x`.
pub(crate) fn capability_purpose(capability_id: &BmadClosureCapabilityId) -> String {
    capability_id.as_str().replacen(':', ".", 1)
}

pub(crate) struct PrepareCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub run_id: ContractId,
    pub instruction_hash: Sha256Digest,
    pub output_schema_id: String,
    pub manifest: ContextEgressManifest,
    pub invocation_binding: ModelInvocationBinding,
    pub deterministic_fixture: String,
}

pub(crate) struct ApproveCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub approved_at: UnixMillis,
}

pub(crate) struct CancelCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub cancelled_at: UnixMillis,
}

pub(crate) struct SubmitCapabilityRunInput {
    pub capability_id: BmadClosureCapabilityId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub submitted_at: UnixMillis,
    /// Whether this run's artifact is the doc workspace's whole state or one
    /// contribution to it, read from the sealed `entrypointKind` by the caller.
    ///
    /// Descriptive only: it shapes what is recorded in the memory log and takes
    /// no part in consent, identity, or any canonical hash.
    pub run_scope: crate::bmad_memlog::RunScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityReviewProjection {
    pub capability_id: String,
    pub run_id: ContractId,
    pub manifest_hash: Sha256Digest,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityApprovedProjection {
    pub capability_id: String,
    pub manifest_hash: Sha256Digest,
    pub decision_id: ContractId,
    pub expires_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityCompletedProjection {
    pub capability_id: String,
    pub run_id: ContractId,
    pub result_kind: String,
}

struct PreparedCapabilityRun {
    capability_id: BmadClosureCapabilityId,
    workspace_id: ContractId,
    workspace_grant_epoch: u64,
    workspace_context_read_epoch: u64,
    run_id: ContractId,
    instruction_hash: Sha256Digest,
    output_schema_id: String,
    manifest: ContextEgressManifest,
    invocation_binding: ModelInvocationBinding,
    deterministic_fixture: String,
}

struct ApprovedCapabilityRun {
    prepared: PreparedCapabilityRun,
    decision: PendingContextDecision,
    issued_at: UnixMillis,
}

/// The finalize-time record for a run whose decision has already been
/// consumed and whose authorized request has been handed to the caller for
/// transport dispatch. The coordinator mutex is *not* held while the model
/// round trip runs, so this is the durable metadata the coordinator needs to
/// record the result once the caller returns.
struct DispatchingCapabilityRun {
    /// Identifies this exact dispatch so a stale `finalize_dispatch` /
    /// `abort_dispatch` (whose transport was still in flight while `invalidate`
    /// then a fresh run replaced the slot) cannot act on a newer dispatch.
    dispatch_id: u64,
    capability_id: BmadClosureCapabilityId,
    run_id: ContractId,
    workspace_id: ContractId,
    instruction_hash: Sha256Digest,
    context_manifest_hash: Sha256Digest,
    output_schema_id: String,
    consent_evidence_id: ContractId,
    submitted_at: UnixMillis,
    run_scope: crate::bmad_memlog::RunScope,
}

/// Everything the caller needs to run one transport round trip *without*
/// holding the coordinator mutex. Returned by [`begin_dispatch`]; the caller
/// sends it and hands the raw output back to [`finalize_dispatch`], quoting
/// [`Self::dispatch_id`] so the coordinator can reject a superseded dispatch.
pub(crate) struct CapabilityDispatchRequest {
    pub dispatch_id: u64,
    /// The workspace whose slot this dispatch occupies. Carried here so the
    /// caller needs nothing but this request to name the run it is finalizing:
    /// `finalize_dispatch` and `abort_dispatch` are the only lifecycle entry
    /// points not already handed a workspace by their input.
    pub workspace_id: ContractId,
    pub authorized_request: AuthorizedModelRequest,
    pub deterministic_fixture: String,
    pub submitted_at: UnixMillis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityTerminalReason {
    Cancelled,
    ConsentExpired,
    ConsentConsumed,
    OutputRejected,
    Invalidated,
}

enum ActiveCapabilityRun {
    ReviewRequired(Box<PreparedCapabilityRun>),
    Approved(Box<ApprovedCapabilityRun>),
    /// The decision is consumed and the authorized request is in flight at the
    /// caller; the coordinator mutex is released. A concurrent `invalidate`
    /// (sign-out / workspace revocation) can still overwrite this with
    /// `Terminal(Invalidated)`, which makes `finalize_dispatch` drop the result.
    Dispatching(Box<DispatchingCapabilityRun>),
    Terminal(CapabilityTerminalReason),
}

const fn terminal_error(reason: CapabilityTerminalReason) -> BmadCapabilityCoordinatorError {
    match reason {
        CapabilityTerminalReason::ConsentExpired => BmadCapabilityCoordinatorError::ConsentExpired,
        CapabilityTerminalReason::ConsentConsumed => {
            BmadCapabilityCoordinatorError::ConsentAlreadyConsumed
        }
        CapabilityTerminalReason::OutputRejected => BmadCapabilityCoordinatorError::OutputRejected,
        CapabilityTerminalReason::Cancelled | CapabilityTerminalReason::Invalidated => {
            BmadCapabilityCoordinatorError::Unauthorized
        }
    }
}

/// Appends the completed artifact's entries to its doc workspace's log, so the
/// *next* run against the same folder starts from what this one decided.
///
/// Infallible by design. The run's result is already durably stored and about
/// to be returned; nothing about recording memory afterwards may retroactively
/// fail it. A capability with no doc workspace keeps no log, a full log stops
/// growing, and a store fault loses continuity for this run only — in every
/// case the completed run stands. Continuity is an addition to the durable
/// result, never a precondition for it.
fn append_completion_memlog(
    store: &LocalStore,
    capability_id: &str,
    workspace_id: &str,
    artifact: &desktop_runtime::BmadDocumentArtifact,
    now: UnixMillis,
    scope: crate::bmad_memlog::RunScope,
) {
    let Some(doc_workspace) = crate::bmad_memlog::doc_workspace_for(capability_id) else {
        return;
    };
    let Ok(prior) = store.memlog_entries(workspace_id, &doc_workspace) else {
        return;
    };
    for entry in crate::bmad_memlog::completion_entries(capability_id, artifact, &prior, scope) {
        let entry_id = format!("memlog_{}", ulid::Ulid::new());
        let outcome = store.append_memlog_entry(desktop_store::MemlogAppend {
            entry_id: &entry_id,
            workspace_id,
            doc_workspace: &doc_workspace,
            kind: entry.kind,
            author: None,
            subject: entry.subject.as_deref(),
            text: &entry.text,
            recorded_at_ms: now.0,
        });
        // A partial log is still a usable chronology; abandoning the rest of
        // this run's entries is better than looping against a full or faulted
        // table once per section.
        if !matches!(outcome, Ok(desktop_store::MemlogAppendOutcome::Appended(_))) {
            return;
        }
    }
}

#[derive(Serialize)]
struct CapabilityDecisionIdentity<'a> {
    schema_version: &'static str,
    capability_id: &'a str,
    run_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    issued_at: UnixMillis,
}

#[derive(Serialize)]
struct CapabilityInvocationIdentity<'a> {
    schema_version: &'static str,
    capability_id: &'a str,
    decision_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    consumed_at: UnixMillis,
}

pub(crate) struct BmadCapabilityCoordinator {
    ledger: MemoryDecisionLedger,
    /// The governed lifecycle slot of every workspace that has one.
    ///
    /// A map rather than one process-global slot: the lifecycle is bound to a
    /// workspace at every stage — `prepare`, `approve`, `cancel`, and
    /// `begin_dispatch` are each handed the workspace they act on, and each
    /// already re-checks that binding — so a single slot meant one workspace's
    /// review blocked a capability run in every other. The workspace is the
    /// only identity present at every entry point (`run_id` exists only at
    /// `prepare`, `dispatch_id` only from `begin_dispatch` onward), which is
    /// why it is the key.
    ///
    /// Per-workspace exclusivity is unchanged and now structural: one key, one
    /// slot, so a second concurrent run in the *same* workspace still fails
    /// closed exactly as it did.
    active: BTreeMap<ContractId, ActiveCapabilityRun>,
    /// Monotonic id stamped on each dispatch so a stale finalize/abort cannot
    /// act on a dispatch that replaced it while the lock was released. Minted
    /// from one counter across all workspaces, so an id is never ambiguous
    /// between them.
    dispatch_seq: u64,
}

impl BmadCapabilityCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            active: BTreeMap::new(),
            dispatch_seq: 0,
        }
    }

    /// Withdraws every in-flight capability review, for sign-out and
    /// workspace-revocation paths (ADR-0002).
    ///
    /// Sign-out and revocation are process-wide, so this stays process-wide:
    /// each workspace's slot becomes `Terminal(Invalidated)` exactly as the one
    /// global slot did, and a workspace holding no slot still gains none.
    pub(crate) fn invalidate(&mut self) {
        for slot in self.active.values_mut() {
            *slot = ActiveCapabilityRun::Terminal(CapabilityTerminalReason::Invalidated);
        }
    }

    /// Gated on the same feature as its only callers: the whole capability
    /// coordinator test module is `deterministic-help`-only, so a bare
    /// `#[cfg(test)]` left this dead in a default-feature test build.
    #[cfg(all(test, feature = "deterministic-help"))]
    pub(crate) fn terminal_reason_for_test(
        &self,
        workspace_id: &ContractId,
    ) -> Option<CapabilityTerminalReason> {
        match self.active.get(workspace_id) {
            Some(ActiveCapabilityRun::Terminal(reason)) => Some(*reason),
            _ => None,
        }
    }

    /// Makes room for `workspace_id` to hold a slot, keeping the map bounded.
    ///
    /// A workspace that already holds one needs no room. Otherwise, at
    /// capacity, the inert `Terminal` slots are dropped: they hold no decision
    /// and no in-flight dispatch, and exist only to tell their own workspace
    /// why its last flow ended. Losing that explanation downgrades a later
    /// `approve`/`cancel`/`submit` on *that* workspace from its specific
    /// terminal error to `Unauthorized` — still refused, never admitted. Live
    /// slots are never touched, because evicting one would let a second
    /// concurrent run start where the lifecycle refuses one today.
    fn reserve_slot(
        &mut self,
        workspace_id: &ContractId,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        if self.active.contains_key(workspace_id)
            || self.active.len() < MAX_TRACKED_CAPABILITY_WORKSPACES
        {
            return Ok(());
        }
        self.active
            .retain(|_, slot| !matches!(slot, ActiveCapabilityRun::Terminal(_)));
        if self.active.len() >= MAX_TRACKED_CAPABILITY_WORKSPACES {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        }
        Ok(())
    }

    fn validate_workspace_binding(
        prepared: &PreparedCapabilityRun,
        workspace_id: &ContractId,
        workspace_grant_epoch: u64,
        workspace_context_read_epoch: u64,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        if prepared.workspace_id != *workspace_id
            || prepared.workspace_grant_epoch != workspace_grant_epoch
            || prepared.workspace_context_read_epoch != workspace_context_read_epoch
        {
            return Err(BmadCapabilityCoordinatorError::Unauthorized);
        }
        Ok(())
    }

    /// Opens one reviewed capability flow. The manifest and invocation
    /// binding must have been minted for exactly this capability: their
    /// purpose label is the capability identity, so cross-capability
    /// substitution fails closed here.
    pub(crate) fn prepare(
        &mut self,
        input: PrepareCapabilityRunInput,
    ) -> Result<CapabilityReviewProjection, BmadCapabilityCoordinatorError> {
        // Only this workspace's own slot can conflict; a run in another
        // workspace is none of this one's business. A `Terminal` slot is
        // overwritten here exactly as before, which is what lets a workspace
        // start a fresh flow after a cancelled, expired, or invalidated one.
        if matches!(
            self.active.get(&input.workspace_id),
            Some(
                ActiveCapabilityRun::ReviewRequired(_)
                    | ActiveCapabilityRun::Approved(_)
                    | ActiveCapabilityRun::Dispatching(_)
            )
        ) {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        }
        self.reserve_slot(&input.workspace_id)?;
        let expected_purpose = capability_purpose(&input.capability_id);
        if input.manifest.draft.purpose != expected_purpose
            || input.invocation_binding.draft.purpose != expected_purpose
        {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.invocation_binding.draft.manifest_hash != input.manifest.manifest_hash {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        if input
            .invocation_binding
            .draft
            .canonical_output_schema_id
            .as_str()
            != input.output_schema_id
        {
            return Err(BmadCapabilityCoordinatorError::ResultArchetypeMismatch);
        }
        let projection = CapabilityReviewProjection {
            capability_id: input.capability_id.as_str().to_owned(),
            run_id: input.run_id.clone(),
            manifest_hash: input.manifest.manifest_hash,
            expires_at: input.manifest.draft.expires_at,
        };
        let workspace_id = input.workspace_id.clone();
        let prepared = PreparedCapabilityRun {
            capability_id: input.capability_id,
            workspace_id: input.workspace_id,
            workspace_grant_epoch: input.workspace_grant_epoch,
            workspace_context_read_epoch: input.workspace_context_read_epoch,
            run_id: input.run_id,
            instruction_hash: input.instruction_hash,
            output_schema_id: input.output_schema_id,
            manifest: input.manifest,
            invocation_binding: input.invocation_binding,
            deterministic_fixture: input.deterministic_fixture,
        };
        self.active.insert(
            workspace_id,
            ActiveCapabilityRun::ReviewRequired(Box::new(prepared)),
        );
        Ok(projection)
    }

    /// Approves the pending review, minting one single-use decision bound
    /// to this capability, manifest, and binding.
    pub(crate) fn approve(
        &mut self,
        input: &ApproveCapabilityRunInput,
    ) -> Result<CapabilityApprovedProjection, BmadCapabilityCoordinatorError> {
        // Naming a workspace other than the one holding the review now finds no
        // slot at all, which is the same `Unauthorized` that
        // `validate_workspace_binding` returned for it before this lane became
        // per-workspace. The epoch halves of that check still run below.
        let prepared = match self.active.get(&input.workspace_id) {
            Some(ActiveCapabilityRun::ReviewRequired(prepared)) => prepared,
            Some(ActiveCapabilityRun::Approved(_) | ActiveCapabilityRun::Dispatching(_)) => {
                return Err(BmadCapabilityCoordinatorError::Conflict);
            }
            Some(ActiveCapabilityRun::Terminal(reason)) => {
                return Err(terminal_error(*reason));
            }
            None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        Self::validate_workspace_binding(
            prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            input.workspace_context_read_epoch,
        )?;
        if input.capability_id != prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != prepared.manifest.manifest_hash {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        if input.approved_at < prepared.manifest.draft.created_at
            || input.approved_at >= prepared.manifest.draft.expires_at
        {
            self.active.insert(
                input.workspace_id.clone(),
                ActiveCapabilityRun::Terminal(CapabilityTerminalReason::ConsentExpired),
            );
            return Err(BmadCapabilityCoordinatorError::ConsentExpired);
        }
        let expires_at = UnixMillis(
            input
                .approved_at
                .0
                .checked_add(DECISION_WINDOW_MS)
                .map_or(prepared.manifest.draft.expires_at.0, |maximum| {
                    maximum.min(prepared.manifest.draft.expires_at.0)
                }),
        );
        let decision_digest = canonical_hash(
            "bmad-capability-context-decision-id",
            1,
            &CapabilityDecisionIdentity {
                schema_version: "sapphirus.bmad-capability-decision.v1",
                capability_id: prepared.capability_id.as_str(),
                run_id: &prepared.run_id,
                manifest_hash: prepared.manifest.manifest_hash,
                binding_hash: prepared.invocation_binding.binding_hash,
                issued_at: input.approved_at,
            },
        )
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let decision_id = derived_contract_id("decision", decision_digest)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        // The slot leaves the map here, so every exit below must put something
        // back: the review on failure, the approval on success. Dropping it
        // would strand the workspace with a silently vanished review.
        let Some(ActiveCapabilityRun::ReviewRequired(prepared)) =
            self.active.remove(&input.workspace_id)
        else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        let decision = match ConsentService::new(&self.ledger).approve(ApproveDecisionInput {
            manifest: &prepared.manifest,
            binding: &prepared.invocation_binding,
            decision_id: decision_id.clone(),
            issued_at: input.approved_at,
            expires_at,
        }) {
            Ok(decision) => decision,
            Err(error) => {
                self.active.insert(
                    input.workspace_id.clone(),
                    ActiveCapabilityRun::ReviewRequired(prepared),
                );
                return Err(map_egress_error(error));
            }
        };
        let projection = CapabilityApprovedProjection {
            capability_id: prepared.capability_id.as_str().to_owned(),
            manifest_hash: input.manifest_hash,
            decision_id,
            expires_at,
        };
        self.active.insert(
            input.workspace_id.clone(),
            ActiveCapabilityRun::Approved(Box::new(ApprovedCapabilityRun {
                prepared: *prepared,
                decision,
                issued_at: input.approved_at,
            })),
        );
        Ok(projection)
    }

    /// Cancels the approved decision without consuming it.
    pub(crate) fn cancel(
        &mut self,
        input: &CancelCapabilityRunInput,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        let approved = match self.active.get(&input.workspace_id) {
            Some(ActiveCapabilityRun::Approved(approved)) => approved,
            Some(ActiveCapabilityRun::ReviewRequired(_)) => {
                return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
            }
            Some(ActiveCapabilityRun::Dispatching(_)) => {
                return Err(BmadCapabilityCoordinatorError::Conflict);
            }
            Some(ActiveCapabilityRun::Terminal(reason)) => {
                return Err(terminal_error(*reason));
            }
            None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        Self::validate_workspace_binding(
            &approved.prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            input.workspace_context_read_epoch,
        )?;
        if input.capability_id != approved.prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        let Some(ActiveCapabilityRun::Approved(approved)) = self.active.remove(&input.workspace_id)
        else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
            decision: &approved.decision,
            cancelled_at: input.cancelled_at,
        });
        self.active.insert(
            input.workspace_id.clone(),
            ActiveCapabilityRun::Terminal(CapabilityTerminalReason::Cancelled),
        );
        Ok(())
    }

    /// Begins one capability dispatch **under the coordinator lock**: it
    /// consumes the single-use decision, builds the authorized model request,
    /// and parks the run in `Dispatching`. The caller then runs the transport
    /// round trip with the mutex released and hands the raw output back to
    /// [`Self::finalize_dispatch`]. On any transport failure the caller must
    /// call [`Self::abort_dispatch`].
    pub(crate) fn begin_dispatch(
        &mut self,
        input: &SubmitCapabilityRunInput,
    ) -> Result<CapabilityDispatchRequest, BmadCapabilityCoordinatorError> {
        self.validate_approved_binding(input)?;
        let Some(ActiveCapabilityRun::Approved(approved)) = self.active.remove(&input.workspace_id)
        else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        if input.submitted_at < approved.issued_at
            || input.submitted_at > approved.decision.expires_at()
        {
            let _ = ConsentService::new(&self.ledger).cancel(CancelDecisionInput {
                decision: &approved.decision,
                cancelled_at: input.submitted_at,
            });
            self.active.insert(
                input.workspace_id.clone(),
                ActiveCapabilityRun::Terminal(CapabilityTerminalReason::ConsentExpired),
            );
            return Err(BmadCapabilityCoordinatorError::ConsentExpired);
        }

        // The slot is out of the map from here. Each exit below decides what
        // refills it: `Terminal(ConsentConsumed)` once the decision has been
        // spent, `Dispatching` on success. The two `Integrity` exits deliberately
        // leave the workspace with *no* slot — the decision was not consumed on
        // that path, so the workspace is simply free to prepare a fresh run,
        // which is what it did when this lane held one global slot. Empty is
        // recoverable; the state to avoid is a slot nothing can clear.
        let invocation_digest = canonical_hash(
            "bmad-capability-invocation-id",
            1,
            &CapabilityInvocationIdentity {
                schema_version: "sapphirus.bmad-capability-invocation.v1",
                capability_id: approved.prepared.capability_id.as_str(),
                decision_id: approved.decision.decision_id(),
                manifest_hash: approved.prepared.manifest.manifest_hash,
                consumed_at: input.submitted_at,
            },
        )
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let invocation_id = derived_contract_id("invoke", invocation_digest)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let consumption = match ConsentService::new(&self.ledger).consume(ConsumeDecisionInput {
            decision: &approved.decision,
            binding: &approved.prepared.invocation_binding,
            invocation_id,
            consumed_at: input.submitted_at,
        }) {
            Ok(consumption) => consumption,
            Err(error) => {
                self.active.insert(
                    input.workspace_id.clone(),
                    ActiveCapabilityRun::Terminal(CapabilityTerminalReason::ConsentConsumed),
                );
                return Err(map_egress_error(error));
            }
        };
        let consent_evidence_id = consumption.decision_id().clone();
        let Ok(authorized_request) = AuthorizedModelRequest::new(
            &approved.prepared.manifest,
            &approved.prepared.invocation_binding,
            consumption,
        ) else {
            self.active.insert(
                input.workspace_id.clone(),
                ActiveCapabilityRun::Terminal(CapabilityTerminalReason::ConsentConsumed),
            );
            return Err(BmadCapabilityCoordinatorError::Integrity);
        };

        self.dispatch_seq = self.dispatch_seq.wrapping_add(1);
        let dispatch_id = self.dispatch_seq;
        let approved = *approved;
        let ApprovedCapabilityRun { prepared, .. } = approved;
        let dispatch = CapabilityDispatchRequest {
            dispatch_id,
            workspace_id: prepared.workspace_id.clone(),
            authorized_request,
            deterministic_fixture: prepared.deterministic_fixture,
            submitted_at: input.submitted_at,
        };
        self.active.insert(
            input.workspace_id.clone(),
            ActiveCapabilityRun::Dispatching(Box::new(DispatchingCapabilityRun {
                dispatch_id,
                capability_id: prepared.capability_id,
                run_id: prepared.run_id,
                workspace_id: prepared.workspace_id,
                instruction_hash: prepared.instruction_hash,
                context_manifest_hash: prepared.manifest.manifest_hash,
                output_schema_id: prepared.output_schema_id,
                consent_evidence_id,
                submitted_at: input.submitted_at,
                run_scope: input.run_scope,
            })),
        );
        Ok(dispatch)
    }

    /// Finalizes an in-flight dispatch **under the coordinator lock**: verifies
    /// the raw output, records the run and its result in one durable step, and
    /// clears that workspace's slot. If the run left `Dispatching` while the
    /// round trip ran — e.g. a concurrent `invalidate` from sign-out or
    /// workspace revocation — the result is dropped and the terminal reason
    /// preserved. Only the named workspace's slot is ever read or written, so a
    /// dispatch can neither observe nor disturb a run in another workspace.
    pub(crate) fn finalize_dispatch(
        &mut self,
        workspace_id: &ContractId,
        dispatch_id: u64,
        raw_output: &RawModelOutput,
        verifier: &dyn BmadCapabilityOutputVerifier,
        store: &LocalStore,
    ) -> Result<CapabilityCompletedProjection, BmadCapabilityCoordinatorError> {
        // Inspected before it is taken, so a stale or superseded finalize
        // leaves the slot exactly as it found it and the map is only disturbed
        // once this call has committed to finishing the run it names.
        match self.active.get(workspace_id) {
            Some(ActiveCapabilityRun::Dispatching(dispatching))
                if dispatching.dispatch_id == dispatch_id => {}
            Some(ActiveCapabilityRun::Terminal(reason)) => {
                return Err(terminal_error(*reason));
            }
            // A newer dispatch replaced this one while its transport was in
            // flight, or the slot is no longer dispatching at all; leave what is
            // there in place and drop this stale result.
            Some(
                ActiveCapabilityRun::Dispatching(_)
                | ActiveCapabilityRun::ReviewRequired(_)
                | ActiveCapabilityRun::Approved(_),
            )
            | None => {
                return Err(BmadCapabilityCoordinatorError::Conflict);
            }
        }
        // From here the slot is out of the map. A rejected output refills it
        // with its terminal reason; the durable-write failures below
        // deliberately leave it empty, exactly as the single global slot was
        // left empty by the same failures, which frees the workspace to prepare
        // another run rather than wedging it.
        let Some(ActiveCapabilityRun::Dispatching(dispatching)) = self.active.remove(workspace_id)
        else {
            return Err(BmadCapabilityCoordinatorError::Conflict);
        };
        let Ok(output) = verifier.verify(&dispatching.capability_id, raw_output) else {
            self.active.insert(
                workspace_id.clone(),
                ActiveCapabilityRun::Terminal(CapabilityTerminalReason::OutputRejected),
            );
            return Err(BmadCapabilityCoordinatorError::OutputRejected);
        };
        if output.schema_id() != dispatching.output_schema_id {
            self.active.insert(
                workspace_id.clone(),
                ActiveCapabilityRun::Terminal(CapabilityTerminalReason::OutputRejected),
            );
            return Err(BmadCapabilityCoordinatorError::ResultArchetypeMismatch);
        }

        let run = BmadCapabilityRun::open(BmadCapabilityRunParams {
            run_id: dispatching.run_id.clone(),
            capability_id: dispatching.capability_id.clone(),
            workspace_id: dispatching.workspace_id.clone(),
            instruction_hash: dispatching.instruction_hash,
            context_manifest_hash: dispatching.context_manifest_hash,
            output_schema_id: dispatching.output_schema_id.clone(),
            consent_evidence_id: dispatching.consent_evidence_id.clone(),
            created_at: dispatching.submitted_at,
        })
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        store
            .create_bmad_capability_run(&run)
            .map_err(|_| BmadCapabilityCoordinatorError::Store)?;
        let result_kind = match &output {
            BmadCapabilityOutput::DocumentArtifact(_) => "document_artifact",
            BmadCapabilityOutput::GovernedChangeSet(_) => "governed_change_set",
            BmadCapabilityOutput::InactiveBuilderDraft(_) => "inactive_builder_draft",
        };
        store
            .record_bmad_capability_result(&run.run_id, &output)
            .map_err(|_| BmadCapabilityCoordinatorError::Store)?;
        if let BmadCapabilityOutput::DocumentArtifact(artifact) = &output {
            append_completion_memlog(
                store,
                dispatching.capability_id.as_str(),
                run.workspace_id.as_str(),
                artifact,
                dispatching.submitted_at,
                dispatching.run_scope,
            );
        }

        // The slot was already taken above; a completed run leaves none behind.
        Ok(CapabilityCompletedProjection {
            capability_id: dispatching.capability_id.as_str().to_owned(),
            run_id: dispatching.run_id,
            result_kind: result_kind.to_owned(),
        })
    }

    /// Marks *this* in-flight dispatch terminal after a transport failure at the
    /// caller. A no-op if the run already left `Dispatching` or was superseded by
    /// a newer dispatch (e.g. it was invalidated then replaced concurrently), and
    /// a no-op on every workspace but the one named.
    pub(crate) fn abort_dispatch(
        &mut self,
        workspace_id: &ContractId,
        dispatch_id: u64,
        reason: CapabilityTerminalReason,
    ) {
        if matches!(
            self.active.get(workspace_id),
            Some(ActiveCapabilityRun::Dispatching(dispatching))
                if dispatching.dispatch_id == dispatch_id
        ) {
            self.active
                .insert(workspace_id.clone(), ActiveCapabilityRun::Terminal(reason));
        }
    }

    /// One-shot dispatch that holds the coordinator across the transport round
    /// trip. Production command handlers use
    /// [`Self::begin_dispatch`]/[`Self::finalize_dispatch`] so the mutex is
    /// released during the network wait; this wrapper preserves the original
    /// single-call contract for the in-process coordinator tests.
    #[cfg(all(test, feature = "deterministic-help"))]
    pub(crate) fn submit(
        &mut self,
        input: &SubmitCapabilityRunInput,
        transport: &dyn BmadCapabilityTransport,
        verifier: &dyn BmadCapabilityOutputVerifier,
        store: &LocalStore,
    ) -> Result<CapabilityCompletedProjection, BmadCapabilityCoordinatorError> {
        let dispatch = self.begin_dispatch(input)?;
        let dispatch_id = dispatch.dispatch_id;
        let workspace_id = dispatch.workspace_id.clone();
        // A `let`-`else` here — which is what this was — discards the failure
        // before anything can read it, so no amount of classification upstream
        // could have reached a caller. Bound and read instead.
        let sent = transport.send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.submitted_at,
        );
        let (_dispatched, raw_output) = match sent {
            Ok(sent) => sent,
            Err(failure) => {
                note_provider_failure("capability", &failure);
                self.abort_dispatch(
                    &workspace_id,
                    dispatch_id,
                    CapabilityTerminalReason::ConsentConsumed,
                );
                return Err(BmadCapabilityCoordinatorError::Transport);
            }
        };
        self.finalize_dispatch(&workspace_id, dispatch_id, &raw_output, verifier, store)
    }
}

impl BmadCapabilityCoordinator {
    fn validate_approved_binding(
        &self,
        input: &SubmitCapabilityRunInput,
    ) -> Result<(), BmadCapabilityCoordinatorError> {
        let approved = match self.active.get(&input.workspace_id) {
            Some(ActiveCapabilityRun::Approved(approved)) => approved,
            Some(ActiveCapabilityRun::ReviewRequired(_)) => {
                return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
            }
            Some(ActiveCapabilityRun::Dispatching(_)) => {
                return Err(BmadCapabilityCoordinatorError::Conflict);
            }
            Some(ActiveCapabilityRun::Terminal(reason)) => {
                return Err(terminal_error(*reason));
            }
            None => {
                return Err(BmadCapabilityCoordinatorError::Unauthorized);
            }
        };
        Self::validate_workspace_binding(
            &approved.prepared,
            &input.workspace_id,
            input.workspace_grant_epoch,
            input.workspace_context_read_epoch,
        )?;
        if input.capability_id != approved.prepared.capability_id {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        if input.manifest_hash != approved.prepared.manifest.manifest_hash
            || input.decision_id != *approved.decision.decision_id()
        {
            return Err(BmadCapabilityCoordinatorError::ConsentBindingMismatch);
        }
        Ok(())
    }
}

const fn map_egress_error(error: EgressError) -> BmadCapabilityCoordinatorError {
    match error {
        EgressError::DecisionExpired => BmadCapabilityCoordinatorError::ConsentExpired,
        EgressError::DecisionAlreadyConsumed | EgressError::DecisionCancelled => {
            BmadCapabilityCoordinatorError::ConsentAlreadyConsumed
        }
        EgressError::DecisionBindingMismatch | EgressError::DecisionAlreadyExists => {
            BmadCapabilityCoordinatorError::ConsentBindingMismatch
        }
        _ => BmadCapabilityCoordinatorError::Integrity,
    }
}
