#![allow(clippy::expect_used, clippy::too_many_lines)]

//! The lifecycle every primary Method and Builder skill shares.

use desktop_runtime::generated_contracts::{
    BmadSession, BmadSessionOutcomeKind, BmadSessionState, BmadSessionToolProposalStatus,
};
use desktop_runtime::{
    apply_bmad_session_command as apply, is_bmad_session_terminal as is_terminal,
    project_bmad_session as project, BmadSessionCommand, BmadSessionCommandError,
};
use serde_json::json;

fn session() -> BmadSession {
    serde_json::from_value(json!({
        "schemaVersion": "sapphirus.bmad-session.v1",
        "sessionId": "session_01J00000000000000000000000",
        "capabilityId": "capability_01J00000000000000000000000",
        "mode": "interactive",
        "agentProfile": "stateless",
        "state": "created",
        "version": 1,
        "identities": {
            "sourceId": "source_01J00000000000000000000000",
            "packageId": "package_01J00000000000000000000000",
            "configHash": format!("sha256:{}", "0".repeat(64)),
            "customizationHash": format!("sha256:{}", "1".repeat(64)),
            "modelId": "model_01J00000000000000000000000",
            "taskId": "task_01J00000000000000000000000",
            "workspaceId": "workspace_01J00000000000000000000000",
            "policyHash": format!("sha256:{}", "2".repeat(64))
        },
        "evidenceHash": format!("sha256:{}", "3".repeat(64)),
        "control": { "pauseAllowed": false, "resumeAllowed": false, "cancelAllowed": true },
        "artifacts": [],
        "evidence": []
    }))
    .expect("generated session")
}

fn input_request() -> desktop_runtime::generated_contracts::BmadSessionInputRequest {
    serde_json::from_value(json!({
        "requestId": "request_01J00000000000000000000000",
        "prompt": "Name the output consumer.",
        "responseSchemaHash": format!("sha256:{}", "4".repeat(64))
    }))
    .expect("input request")
}

fn answer() -> desktop_runtime::generated_contracts::BmadSessionRecordedAnswer {
    serde_json::from_value(json!({
        "requestId": "request_01J00000000000000000000000",
        "answerHash": format!("sha256:{}", "5".repeat(64))
    }))
    .expect("answer")
}

fn proposal(id: &str, plan: &str) -> desktop_runtime::generated_contracts::BmadSessionToolProposal {
    serde_json::from_value(json!({
        "proposalId": id,
        "planHash": format!("sha256:{}", plan.repeat(64)),
        "status": "proposed"
    }))
    .expect("tool proposal")
}

fn approval(plan: &str) -> desktop_runtime::generated_contracts::BmadSessionApprovalRequest {
    serde_json::from_value(json!({
        "requestId": "request_01J00000000000000000000001",
        "planHash": format!("sha256:{}", plan.repeat(64)),
        "effectClass": "filesystem"
    }))
    .expect("approval request")
}

fn outcome(kind: &str, code: &str) -> desktop_runtime::generated_contracts::BmadSessionOutcome {
    serde_json::from_value(json!({ "kind": kind, "code": code })).expect("outcome")
}

/// Drives one session to `running` and returns it with its current version.
fn running() -> (BmadSession, u64) {
    let mut value = session();
    let mut version = 1;
    for command in [
        BmadSessionCommand::Resolve,
        BmadSessionCommand::MarkReady,
        BmadSessionCommand::Start,
    ] {
        let event = apply(&mut value, version, &command).expect("transition");
        version = event.to_version;
    }
    assert_eq!(value.state, BmadSessionState::Running);
    (value, version)
}

#[test]
fn bmad_session_state_machine_runs_the_full_eleven_state_lifecycle() {
    let mut value = session();
    assert_eq!(value.state, BmadSessionState::Created);
    let mut version = value.version.get();
    let mut seen = vec![value.state];

    for command in [
        BmadSessionCommand::Resolve,
        BmadSessionCommand::RequireInput(input_request()),
        BmadSessionCommand::ProvideAnswer(answer()),
        BmadSessionCommand::MarkReady,
        BmadSessionCommand::Start,
        BmadSessionCommand::RequestApproval(approval("6")),
        BmadSessionCommand::SettleApproval {
            request_id: "request_01J00000000000000000000001".to_owned(),
            approved: true,
        },
        BmadSessionCommand::Pause,
        BmadSessionCommand::Start,
        BmadSessionCommand::BeginValidation,
        BmadSessionCommand::Complete(outcome("completed", "skill_complete")),
    ] {
        let event = apply(&mut value, version, &command).expect("lifecycle transition");
        assert_eq!(event.from_version, version);
        assert_eq!(event.to_version, version + 1);
        assert!(!event.idempotent);
        // Every accepted move extends the chain rather than restating it.
        assert_ne!(event.evidence_hash, event.previous_evidence_hash);
        version = event.to_version;
        seen.push(value.state);
    }

    // All eleven contract states are reachable through this one machine.
    for state in [
        BmadSessionState::Created,
        BmadSessionState::Resolving,
        BmadSessionState::InputRequired,
        BmadSessionState::Ready,
        BmadSessionState::Running,
        BmadSessionState::AwaitingApproval,
        BmadSessionState::Paused,
        BmadSessionState::Validating,
        BmadSessionState::Completed,
    ] {
        assert!(seen.contains(&state), "unreached state {state:?}");
    }
    assert!(is_terminal(value.state));
    assert_eq!(
        u64::try_from(value.evidence.len()).expect("evidence count"),
        version - 1
    );
    assert!(!value.control.cancel_allowed);

    // Failure and cancellation are reachable from a non-terminal state.
    let (mut failing, failing_version) = running();
    apply(
        &mut failing,
        failing_version,
        &BmadSessionCommand::Fail(outcome("failed", "model_unavailable")),
    )
    .expect("fail");
    assert_eq!(failing.state, BmadSessionState::Failed);

    let (mut cancelling, cancelling_version) = running();
    apply(
        &mut cancelling,
        cancelling_version,
        &BmadSessionCommand::Cancel(outcome("cancelled", "user_cancelled")),
    )
    .expect("cancel");
    assert_eq!(cancelling.state, BmadSessionState::Cancelled);
}

#[test]
fn bmad_session_state_machine_fails_closed_on_replay_and_invalid_transitions() {
    let (mut value, version) = running();
    let before = serde_json::to_value(&value).expect("session json");

    // A stale writer loses. The session is untouched.
    let stale = apply(&mut value, version - 1, &BmadSessionCommand::Pause)
        .expect_err("stale version must be refused");
    assert_eq!(
        stale,
        BmadSessionCommandError::VersionConflict {
            expected: version - 1,
            actual: version
        }
    );
    assert_eq!(serde_json::to_value(&value).expect("session json"), before);

    // An undefined pair is refused without mutating anything.
    let invalid = apply(&mut value, version, &BmadSessionCommand::Resolve)
        .expect_err("resolve is not defined for running");
    assert!(matches!(
        invalid,
        BmadSessionCommandError::InvalidTransition {
            from: BmadSessionState::Running,
            command: "resolve"
        }
    ));
    assert_eq!(serde_json::to_value(&value).expect("session json"), before);

    // A settlement naming an approval the session is not holding is refused.
    let unknown = apply(
        &mut value,
        version,
        &BmadSessionCommand::SettleApproval {
            request_id: "request_01J0000000000000000000000Z".to_owned(),
            approved: true,
        },
    )
    .expect_err("no outstanding approval");
    assert!(matches!(
        unknown,
        BmadSessionCommandError::InvalidTransition { .. }
    ));

    // A terminal outcome must match the state it closes.
    let mismatched = apply(
        &mut value,
        version,
        &BmadSessionCommand::Fail(outcome("completed", "skill_complete")),
    )
    .expect_err("outcome kind must match");
    assert_eq!(mismatched, BmadSessionCommandError::OutcomeMismatch);

    // Terminal states accept nothing at all.
    let mut terminal = value.clone();
    let terminal_version = apply(
        &mut terminal,
        version,
        &BmadSessionCommand::Cancel(outcome("cancelled", "user_cancelled")),
    )
    .expect("cancel")
    .to_version;
    for command in [
        BmadSessionCommand::Resume,
        BmadSessionCommand::Start,
        BmadSessionCommand::Pause,
        BmadSessionCommand::Fail(outcome("failed", "late_failure")),
    ] {
        let refused = apply(&mut terminal, terminal_version, &command)
            .expect_err("terminal sessions are closed");
        assert!(matches!(
            refused,
            BmadSessionCommandError::InvalidTransition { .. }
        ));
    }
    assert_eq!(terminal.state, BmadSessionState::Cancelled);
}

#[test]
fn bmad_session_state_machine_redelivers_append_commands_idempotently() {
    let (mut value, version) = running();

    let first = apply(
        &mut value,
        version,
        &BmadSessionCommand::ProposeTool(proposal("proposal_01J00000000000000000000000", "7")),
    )
    .expect("first proposal");
    assert!(!first.idempotent);
    assert_eq!(value.tool_proposals.len(), 1);

    // The same command redelivered at the new version is a no-op: no second
    // proposal, no version movement, no new evidence link.
    let repeat = apply(
        &mut value,
        first.to_version,
        &BmadSessionCommand::ProposeTool(proposal("proposal_01J00000000000000000000000", "7")),
    )
    .expect("redelivered proposal");
    assert!(repeat.idempotent);
    assert_eq!(repeat.to_version, first.to_version);
    assert_eq!(repeat.evidence_hash, repeat.previous_evidence_hash);
    assert_eq!(value.tool_proposals.len(), 1);
    assert_eq!(value.version.get(), first.to_version);

    // A distinct proposal still appends.
    let second = apply(
        &mut value,
        first.to_version,
        &BmadSessionCommand::ProposeTool(proposal("proposal_01J00000000000000000000001", "8")),
    )
    .expect("second proposal");
    assert!(!second.idempotent);
    assert_eq!(value.tool_proposals.len(), 2);
}

#[test]
fn bmad_session_state_machine_settles_approvals_without_executing_anything() {
    let (mut value, version) = running();
    let event = apply(
        &mut value,
        version,
        &BmadSessionCommand::ProposeTool(proposal("proposal_01J00000000000000000000000", "9")),
    )
    .expect("proposal");
    let event = apply(
        &mut value,
        event.to_version,
        &BmadSessionCommand::RequestApproval(approval("9")),
    )
    .expect("approval requested");
    assert_eq!(value.state, BmadSessionState::AwaitingApproval);
    assert!(value.approval_request.is_some());

    let event = apply(
        &mut value,
        event.to_version,
        &BmadSessionCommand::SettleApproval {
            request_id: "request_01J00000000000000000000001".to_owned(),
            approved: true,
        },
    )
    .expect("approval settled");
    assert_eq!(value.state, BmadSessionState::Running);
    assert!(value.approval_request.is_none());
    assert_eq!(
        value.tool_proposals[0].status,
        BmadSessionToolProposalStatus::Approved
    );

    // Approval marks the proposal approved and nothing else. The session
    // records no execution result, because this machine cannot execute.
    let wire = serde_json::to_value(&value).expect("wire contract");
    assert!(wire.get("executedEffects").is_none());
    assert!(wire.get("processHandles").is_none());
    assert_eq!(wire["state"], "running");

    // Rejection is equally inert.
    let (mut rejecting, rejecting_version) = running();
    let e = apply(
        &mut rejecting,
        rejecting_version,
        &BmadSessionCommand::ProposeTool(proposal("proposal_01J00000000000000000000002", "a")),
    )
    .expect("proposal");
    let e = apply(
        &mut rejecting,
        e.to_version,
        &BmadSessionCommand::RequestApproval(approval("a")),
    )
    .expect("approval requested");
    apply(
        &mut rejecting,
        e.to_version,
        &BmadSessionCommand::SettleApproval {
            request_id: "request_01J00000000000000000000001".to_owned(),
            approved: false,
        },
    )
    .expect("approval rejected");
    assert_eq!(
        rejecting.tool_proposals[0].status,
        BmadSessionToolProposalStatus::Rejected
    );
    let _ = event;
}

#[test]
fn bmad_session_state_machine_projects_without_leaking_governed_content() {
    let (mut value, version) = running();
    apply(
        &mut value,
        version,
        &BmadSessionCommand::RequireInput(input_request()),
    )
    .expect("input required");

    let projection = project(&value);
    assert!(projection.awaiting_input);
    assert!(!projection.awaiting_approval);
    assert!(!projection.terminal);
    assert_eq!(projection.state, BmadSessionState::InputRequired);
    assert_eq!(projection.session_id, "session_01J00000000000000000000000");
    assert!(projection.control.resume_allowed);
    assert!(!projection.control.pause_allowed);

    // The renderer receives counts and affordances, never the prompt text,
    // the answers, the model payloads, or the identity bindings.
    let wire = serde_json::to_value(&projection).expect("projection json");
    let rendered = serde_json::to_string(&wire).expect("projection string");
    assert!(!rendered.contains("Name the output consumer"));
    assert!(wire.get("inputRequest").is_none());
    assert!(wire.get("answers").is_none());
    assert!(wire.get("modelCalls").is_none());
    assert!(wire.get("identities").is_none());
    assert_eq!(wire["answerCount"], 0);
}

#[test]
fn bmad_session_state_machine_chains_evidence_over_the_prior_hash() {
    let mut value = session();
    let mut version = value.version.get();
    let mut chain = Vec::new();

    for command in [
        BmadSessionCommand::Resolve,
        BmadSessionCommand::MarkReady,
        BmadSessionCommand::Start,
        BmadSessionCommand::Pause,
    ] {
        let event = apply(&mut value, version, &command).expect("transition");
        // Each link names its predecessor exactly.
        if let Some(previous) = chain.last() {
            assert_eq!(&event.previous_evidence_hash, previous);
        }
        chain.push(event.evidence_hash);
        version = event.to_version;
    }

    // The chain is strictly growing and free of repeats, and the session's
    // head hash is the last link.
    let mut sorted = chain.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), chain.len());
    assert_eq!(
        value.evidence_hash.as_str(),
        chain.last().expect("chain").to_string()
    );
    assert_eq!(value.evidence.len(), chain.len());

    // The same command sequence on a fresh session reproduces the same chain:
    // transitions are deterministic in identity, not in wall-clock time.
    let mut replay = session();
    let mut replay_version = replay.version.get();
    let mut replay_chain = Vec::new();
    for command in [
        BmadSessionCommand::Resolve,
        BmadSessionCommand::MarkReady,
        BmadSessionCommand::Start,
        BmadSessionCommand::Pause,
    ] {
        let event = apply(&mut replay, replay_version, &command).expect("transition");
        replay_chain.push(event.evidence_hash);
        replay_version = event.to_version;
    }
    assert_eq!(replay_chain, chain);
    let _ = BmadSessionOutcomeKind::Completed;
}
