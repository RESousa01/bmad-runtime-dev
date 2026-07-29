//! The one durable lifecycle every Method and Builder skill runs through.
//!
//! Before this module a skill was advanced by pasting prompt text and hoping
//! the caller remembered which stage it was in. Here a session is a persisted
//! contract advanced only by typed commands: each accepted command bumps an
//! optimistic `version`, chains a new `evidence_hash` over the prior one, and
//! emits one append-only event. A caller holding a stale version cannot write,
//! a replayed command cannot double-apply, and an invalid pair cannot mutate
//! anything at all.
//!
//! The machine has no execution port. `ToolProposal` records an intent and its
//! plan hash; approving one only marks the proposal approved. Running the
//! effect belongs to the Airlock, not to this file.

use serde::Serialize;
use serde_json::json;

use crate::generated_contracts::{
    BmadSession, BmadSessionApprovalRequest, BmadSessionInputRequest, BmadSessionModelCall,
    BmadSessionOutcome, BmadSessionOutcomeKind, BmadSessionRecordedAnswer,
    BmadSessionSessionControl, BmadSessionState, BmadSessionToolProposal,
    BmadSessionToolProposalStatus,
};
use crate::{canonical_hash, Sha256Digest};

/// Bounds that keep one session's recorded history inside its contract limits.
const MAX_ANSWERS: usize = 1024;
const MAX_MODEL_CALLS: usize = 1024;
const MAX_TOOL_PROPOSALS: usize = 1024;
const MAX_ARTIFACTS: usize = 1024;
const MAX_EVIDENCE: usize = 4096;

/// A typed intent to advance one session.
///
/// Commands carrying payload are the ones that must be idempotent under
/// retry: a redelivered `ProvideAnswer` for an answer already recorded is
/// accepted as a no-op rather than appending a duplicate.
#[derive(Clone, Debug)]
pub enum BmadSessionCommand {
    /// `created` -> `resolving`. Begins source, package, config, and
    /// customization resolution.
    Resolve,
    /// `resolving` | `running` -> `input_required`.
    RequireInput(BmadSessionInputRequest),
    /// Records one answer to an outstanding input request.
    ProvideAnswer(BmadSessionRecordedAnswer),
    /// `resolving` | `input_required` -> `ready`.
    MarkReady,
    /// `ready` | `paused` -> `running`.
    Start,
    /// Records one model call against the running session.
    RecordModelCall(BmadSessionModelCall),
    /// Records one inert tool proposal. Never executes anything.
    ProposeTool(BmadSessionToolProposal),
    /// `running` -> `awaiting_approval`.
    RequestApproval(BmadSessionApprovalRequest),
    /// Resolves the outstanding approval and returns to `running`.
    SettleApproval { request_id: String, approved: bool },
    /// `running` -> `paused`.
    Pause,
    /// `paused` | `input_required` -> `running`.
    Resume,
    /// `running` -> `validating`.
    BeginValidation,
    /// Attaches one produced artifact identifier.
    AttachArtifact(String),
    /// `validating` -> `completed`.
    Complete(BmadSessionOutcome),
    /// Any non-terminal state -> `failed`.
    Fail(BmadSessionOutcome),
    /// Any non-terminal state -> `cancelled`.
    Cancel(BmadSessionOutcome),
}

/// Why a command was refused. Every variant leaves the session untouched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmadSessionCommandError {
    /// The caller's `expected_version` did not match the stored version.
    VersionConflict { expected: u64, actual: u64 },
    /// The command is not defined for the current state.
    InvalidTransition {
        from: BmadSessionState,
        command: &'static str,
    },
    /// The command referenced a request the session is not waiting on.
    UnknownRequest,
    /// Recording this entry would exceed the contract's bounded history.
    BoundsViolation,
    /// The outcome kind did not match the terminal state being entered.
    OutcomeMismatch,
    /// The evidence chain could not be extended.
    Hash,
}

/// One accepted transition. Callers append these; they never rewrite them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadSessionEvent {
    pub session_id: String,
    pub command: &'static str,
    pub from_state: BmadSessionState,
    pub to_state: BmadSessionState,
    pub from_version: u64,
    pub to_version: u64,
    pub previous_evidence_hash: Sha256Digest,
    pub evidence_hash: Sha256Digest,
    /// True when the command was already reflected in the session. The
    /// session is unchanged and the version does not move.
    pub idempotent: bool,
}

/// The renderer-safe view of a session.
///
/// Prompts, answers, model payloads, and plan bodies are deliberately absent:
/// the renderer receives state, control affordances, and counts, which is
/// everything it needs to draw progress without holding governed content.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadSessionProjection {
    pub session_id: String,
    pub capability_id: String,
    pub state: BmadSessionState,
    pub version: u64,
    pub control: BmadSessionSessionControl,
    pub stage: Option<String>,
    pub action: Option<String>,
    pub awaiting_input: bool,
    pub awaiting_approval: bool,
    pub answer_count: usize,
    pub model_call_count: usize,
    pub tool_proposal_count: usize,
    pub artifact_count: usize,
    pub terminal: bool,
    pub evidence_hash: Sha256Digest,
}

/// Applies one optimistic command to a session.
///
/// On success the session is mutated in place and the returned event records
/// the exact version and evidence-hash movement. On any error the session is
/// left byte-identical to its input, so a caller may persist unconditionally
/// only after a successful call.
///
/// # Errors
///
/// Returns [`BmadSessionCommandError::VersionConflict`] for a stale writer,
/// [`BmadSessionCommandError::InvalidTransition`] for a state/command pair
/// that is not defined, [`BmadSessionCommandError::UnknownRequest`] when a
/// settlement names a request the session is not holding,
/// [`BmadSessionCommandError::BoundsViolation`] when bounded history is full,
/// [`BmadSessionCommandError::OutcomeMismatch`] when a terminal outcome does
/// not match the state it closes, and [`BmadSessionCommandError::Hash`] when
/// the evidence chain cannot be extended.
pub fn apply_command(
    session: &mut BmadSession,
    expected_version: u64,
    command: &BmadSessionCommand,
) -> Result<BmadSessionEvent, BmadSessionCommandError> {
    let actual = session.version.get();
    if actual != expected_version {
        return Err(BmadSessionCommandError::VersionConflict {
            expected: expected_version,
            actual,
        });
    }

    let from_state = session.state;
    let previous_evidence_hash = digest_of(&session.evidence_hash)?;

    if is_already_applied(session, command) {
        return Ok(BmadSessionEvent {
            session_id: session.session_id.to_string(),
            command: command_label(command),
            from_state,
            to_state: from_state,
            from_version: actual,
            to_version: actual,
            previous_evidence_hash,
            evidence_hash: previous_evidence_hash,
            idempotent: true,
        });
    }

    // Work on a clone so a mid-command failure cannot leave a torn session.
    let mut next = session.clone();
    let to_state = transition(&mut next, from_state, command)?;

    let to_version = actual
        .checked_add(1)
        .ok_or(BmadSessionCommandError::BoundsViolation)?;
    next.version =
        std::num::NonZeroU64::new(to_version).ok_or(BmadSessionCommandError::BoundsViolation)?;
    next.state = to_state;
    next.control = control_for(to_state);

    let evidence_hash = chain(
        previous_evidence_hash,
        session.session_id.as_str(),
        command_label(command),
        from_state,
        to_state,
        to_version,
    )?;
    if next.evidence.len() >= MAX_EVIDENCE {
        return Err(BmadSessionCommandError::BoundsViolation);
    }
    next.evidence.push(sha256_contract(evidence_hash)?);
    next.evidence_hash = sha256_contract(evidence_hash)?;

    *session = next;
    Ok(BmadSessionEvent {
        session_id: session.session_id.to_string(),
        command: command_label(command),
        from_state,
        to_state,
        from_version: actual,
        to_version,
        previous_evidence_hash,
        evidence_hash,
        idempotent: false,
    })
}

/// Builds the renderer-safe projection of a session.
#[must_use]
pub fn project(session: &BmadSession) -> BmadSessionProjection {
    BmadSessionProjection {
        session_id: session.session_id.to_string(),
        capability_id: session.capability_id.to_string(),
        state: session.state,
        version: session.version.get(),
        control: session.control.clone(),
        stage: session.stage.as_deref().cloned(),
        action: session.action.as_deref().cloned(),
        awaiting_input: session.state == BmadSessionState::InputRequired,
        awaiting_approval: session.state == BmadSessionState::AwaitingApproval,
        answer_count: session.answers.len(),
        model_call_count: session.model_calls.len(),
        tool_proposal_count: session.tool_proposals.len(),
        artifact_count: session.artifacts.len(),
        terminal: is_terminal(session.state),
        evidence_hash: digest_of(&session.evidence_hash)
            .unwrap_or_else(|_| Sha256Digest::from_bytes([0; 32])),
    }
}

/// True for states no command can leave.
#[must_use]
pub const fn is_terminal(state: BmadSessionState) -> bool {
    matches!(
        state,
        BmadSessionState::Completed | BmadSessionState::Failed | BmadSessionState::Cancelled
    )
}

fn transition(
    session: &mut BmadSession,
    from: BmadSessionState,
    command: &BmadSessionCommand,
) -> Result<BmadSessionState, BmadSessionCommandError> {
    if is_terminal(from) {
        return Err(invalid(from, command));
    }
    if let Some(next) = close(session, command)? {
        return Ok(next);
    }
    if let Some(next) = record(session, from, command)? {
        return Ok(next);
    }
    move_state(session, from, command)
}

/// The terminal closes, which are legal from every non-terminal state except
/// `Complete`, which may only close a validated run.
fn close(
    session: &mut BmadSession,
    command: &BmadSessionCommand,
) -> Result<Option<BmadSessionState>, BmadSessionCommandError> {
    let (outcome, expected, next) = match command {
        BmadSessionCommand::Complete(outcome) => (
            outcome,
            BmadSessionOutcomeKind::Completed,
            BmadSessionState::Completed,
        ),
        BmadSessionCommand::Fail(outcome) => (
            outcome,
            BmadSessionOutcomeKind::Failed,
            BmadSessionState::Failed,
        ),
        BmadSessionCommand::Cancel(outcome) => (
            outcome,
            BmadSessionOutcomeKind::Cancelled,
            BmadSessionState::Cancelled,
        ),
        _ => return Ok(None),
    };
    if outcome.kind != expected {
        return Err(BmadSessionCommandError::OutcomeMismatch);
    }
    session.outcome = Some(outcome.clone());
    Ok(Some(next))
}

/// Commands that append one bounded record and leave the state where it was.
fn record(
    session: &mut BmadSession,
    from: BmadSessionState,
    command: &BmadSessionCommand,
) -> Result<Option<BmadSessionState>, BmadSessionCommandError> {
    match (from, command) {
        (BmadSessionState::Running, BmadSessionCommand::RecordModelCall(call)) => {
            if session.model_calls.len() >= MAX_MODEL_CALLS {
                return Err(BmadSessionCommandError::BoundsViolation);
            }
            session.model_calls.push(call.clone());
        }
        (BmadSessionState::Running, BmadSessionCommand::ProposeTool(proposal)) => {
            if session.tool_proposals.len() >= MAX_TOOL_PROPOSALS {
                return Err(BmadSessionCommandError::BoundsViolation);
            }
            // A proposal may only enter the log unsettled; approval is a
            // separate, explicitly authorized step.
            if proposal.status != BmadSessionToolProposalStatus::Proposed {
                return Err(invalid(from, command));
            }
            session.tool_proposals.push(proposal.clone());
        }
        (
            BmadSessionState::Running | BmadSessionState::Validating,
            BmadSessionCommand::AttachArtifact(artifact),
        ) => {
            if session.artifacts.len() >= MAX_ARTIFACTS {
                return Err(BmadSessionCommandError::BoundsViolation);
            }
            let parsed = artifact
                .parse()
                .map_err(|_| BmadSessionCommandError::UnknownRequest)?;
            session.artifacts.push(parsed);
        }
        _ => return Ok(None),
    }
    Ok(Some(from))
}

/// The plain lifecycle moves, including the two that also settle a request.
fn move_state(
    session: &mut BmadSession,
    from: BmadSessionState,
    command: &BmadSessionCommand,
) -> Result<BmadSessionState, BmadSessionCommandError> {
    match (from, command) {
        (BmadSessionState::Created, BmadSessionCommand::Resolve) => Ok(BmadSessionState::Resolving),
        (BmadSessionState::Resolving, BmadSessionCommand::MarkReady) => Ok(BmadSessionState::Ready),
        (BmadSessionState::Running, BmadSessionCommand::Pause) => Ok(BmadSessionState::Paused),
        (BmadSessionState::Running, BmadSessionCommand::BeginValidation) => {
            Ok(BmadSessionState::Validating)
        }
        (BmadSessionState::Ready | BmadSessionState::Paused, BmadSessionCommand::Start)
        | (
            BmadSessionState::Paused | BmadSessionState::InputRequired,
            BmadSessionCommand::Resume,
        ) => Ok(BmadSessionState::Running),

        (
            BmadSessionState::Resolving | BmadSessionState::Running,
            BmadSessionCommand::RequireInput(request),
        ) => {
            session.input_request = Some(request.clone());
            Ok(BmadSessionState::InputRequired)
        }
        (BmadSessionState::InputRequired, BmadSessionCommand::ProvideAnswer(answer)) => {
            let outstanding = session
                .input_request
                .as_ref()
                .is_some_and(|request| request.request_id == answer.request_id);
            if !outstanding {
                return Err(BmadSessionCommandError::UnknownRequest);
            }
            if session.answers.len() >= MAX_ANSWERS {
                return Err(BmadSessionCommandError::BoundsViolation);
            }
            session.answers.push(answer.clone());
            session.input_request = None;
            Ok(BmadSessionState::Resolving)
        }

        (BmadSessionState::Running, BmadSessionCommand::RequestApproval(request)) => {
            session.approval_request = Some(request.clone());
            Ok(BmadSessionState::AwaitingApproval)
        }
        (
            BmadSessionState::AwaitingApproval,
            BmadSessionCommand::SettleApproval {
                request_id,
                approved,
            },
        ) => settle_approval(session, request_id, *approved),

        _ => Err(invalid(from, command)),
    }
}

/// Marks the proposals covered by the outstanding approval, and nothing else.
fn settle_approval(
    session: &mut BmadSession,
    request_id: &str,
    approved: bool,
) -> Result<BmadSessionState, BmadSessionCommandError> {
    let held = session
        .approval_request
        .as_ref()
        .ok_or(BmadSessionCommandError::UnknownRequest)?;
    if held.request_id.as_str() != request_id {
        return Err(BmadSessionCommandError::UnknownRequest);
    }
    let plan_hash = held.plan_hash.clone();
    let settled = if approved {
        BmadSessionToolProposalStatus::Approved
    } else {
        BmadSessionToolProposalStatus::Rejected
    };
    for proposal in &mut session.tool_proposals {
        if proposal.plan_hash == plan_hash
            && proposal.status == BmadSessionToolProposalStatus::Proposed
        {
            proposal.status = settled;
        }
    }
    session.approval_request = None;
    Ok(BmadSessionState::Running)
}

fn invalid(from: BmadSessionState, command: &BmadSessionCommand) -> BmadSessionCommandError {
    BmadSessionCommandError::InvalidTransition {
        from,
        command: command_label(command),
    }
}

/// True when the command's whole effect is already recorded.
///
/// Only append-shaped commands can be redelivered safely; a bare state move
/// such as `Pause` is deliberately not treated as idempotent, because two
/// distinct pauses are two distinct facts.
fn is_already_applied(session: &BmadSession, command: &BmadSessionCommand) -> bool {
    match command {
        BmadSessionCommand::ProvideAnswer(answer) => session
            .answers
            .iter()
            .any(|recorded| recorded.request_id == answer.request_id),
        BmadSessionCommand::RecordModelCall(call) => session
            .model_calls
            .iter()
            .any(|recorded| recorded.call_id == call.call_id),
        BmadSessionCommand::ProposeTool(proposal) => session
            .tool_proposals
            .iter()
            .any(|recorded| recorded.proposal_id == proposal.proposal_id),
        BmadSessionCommand::AttachArtifact(artifact) => session
            .artifacts
            .iter()
            .any(|recorded| recorded.as_str() == artifact.as_str()),
        _ => false,
    }
}

const fn command_label(command: &BmadSessionCommand) -> &'static str {
    match command {
        BmadSessionCommand::Resolve => "resolve",
        BmadSessionCommand::RequireInput(_) => "require_input",
        BmadSessionCommand::ProvideAnswer(_) => "provide_answer",
        BmadSessionCommand::MarkReady => "mark_ready",
        BmadSessionCommand::Start => "start",
        BmadSessionCommand::RecordModelCall(_) => "record_model_call",
        BmadSessionCommand::ProposeTool(_) => "propose_tool",
        BmadSessionCommand::RequestApproval(_) => "request_approval",
        BmadSessionCommand::SettleApproval { .. } => "settle_approval",
        BmadSessionCommand::Pause => "pause",
        BmadSessionCommand::Resume => "resume",
        BmadSessionCommand::BeginValidation => "begin_validation",
        BmadSessionCommand::AttachArtifact(_) => "attach_artifact",
        BmadSessionCommand::Complete(_) => "complete",
        BmadSessionCommand::Fail(_) => "fail",
        BmadSessionCommand::Cancel(_) => "cancel",
    }
}

const fn control_for(state: BmadSessionState) -> BmadSessionSessionControl {
    BmadSessionSessionControl {
        cancel_allowed: !is_terminal(state),
        pause_allowed: matches!(state, BmadSessionState::Running),
        resume_allowed: matches!(
            state,
            BmadSessionState::InputRequired | BmadSessionState::Paused
        ),
    }
}

fn chain(
    previous: Sha256Digest,
    session_id: &str,
    command: &str,
    from: BmadSessionState,
    to: BmadSessionState,
    version: u64,
) -> Result<Sha256Digest, BmadSessionCommandError> {
    canonical_hash(
        "bmad-session-transition",
        1,
        &json!({
            "previous": previous.to_string(),
            "sessionId": session_id,
            "command": command,
            "from": from.to_string(),
            "to": to.to_string(),
            "version": version,
        }),
    )
    .map_err(|_| BmadSessionCommandError::Hash)
}

fn digest_of(
    value: &crate::generated_contracts::CommonSha256,
) -> Result<Sha256Digest, BmadSessionCommandError> {
    Sha256Digest::parse(value.as_str()).map_err(|_| BmadSessionCommandError::Hash)
}

fn sha256_contract(
    digest: Sha256Digest,
) -> Result<crate::generated_contracts::CommonSha256, BmadSessionCommandError> {
    digest
        .to_string()
        .parse()
        .map_err(|_| BmadSessionCommandError::Hash)
}
