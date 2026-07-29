#![allow(clippy::expect_used)]

//! Restart recovery and optimistic durability for the BMAD skill lifecycle.

use desktop_runtime::generated_contracts::{BmadSession, BmadSessionState};
use desktop_runtime::{apply_bmad_session_command as apply, BmadSessionCommand};
use desktop_store::{KeyProtector, LocalStore, StoreError};
use serde_json::json;

const WORKSPACE: &str = "workspace_01J00000000000000000000000";
const SESSION: &str = "session_01J00000000000000000000000";

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

fn session() -> BmadSession {
    serde_json::from_value(json!({
        "schemaVersion": "sapphirus.bmad-session.v1",
        "sessionId": SESSION,
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
            "workspaceId": WORKSPACE,
            "policyHash": format!("sha256:{}", "2".repeat(64))
        },
        "evidenceHash": format!("sha256:{}", "3".repeat(64)),
        "control": { "pauseAllowed": false, "resumeAllowed": false, "cancelAllowed": true },
        "artifacts": [],
        "evidence": []
    }))
    .expect("generated session")
}

#[test]
fn bmad_sessions_restart_recovery_loses_no_accepted_state() -> Result<(), Box<dyn std::error::Error>>
{
    // Break caught: a lifecycle held only in memory silently restarts a
    // half-finished skill from `created` after a crash.
    let directory = tempfile::tempdir()?;
    let mut value = session();

    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.create_bmad_session(&value, WORKSPACE, 1_700_000_000_000)?;

        let mut version = value.version.get();
        for (index, command) in [
            BmadSessionCommand::Resolve,
            BmadSessionCommand::MarkReady,
            BmadSessionCommand::Start,
            BmadSessionCommand::Pause,
        ]
        .into_iter()
        .enumerate()
        {
            let event = apply(&mut value, version, &command).expect("transition");
            store.commit_bmad_session_transition(
                &value,
                &event,
                1_700_000_000_001 + index as u64,
            )?;
            version = event.to_version;
        }
        assert_eq!(value.state, BmadSessionState::Paused);
    }

    // Reopen: a different process, a cold store, no in-memory carry-over.
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let recovered = store
        .load_bmad_session(SESSION)?
        .expect("session survives restart");

    assert_eq!(recovered.session.state, BmadSessionState::Paused);
    assert_eq!(recovered.session.version.get(), value.version.get());
    assert_eq!(recovered.session.evidence_hash, value.evidence_hash);
    assert_eq!(recovered.workspace_id, WORKSPACE);
    assert_eq!(
        serde_json::to_value(&recovered.session)?,
        serde_json::to_value(&value)?
    );

    // The identity bindings a resumed run must honour are all durable.
    let identities = &recovered.session.identities;
    assert_eq!(identities.workspace_id.as_str(), WORKSPACE);
    assert_eq!(
        identities.config_hash.as_str(),
        format!("sha256:{}", "0".repeat(64))
    );
    assert_eq!(
        identities.customization_hash.as_str(),
        format!("sha256:{}", "1".repeat(64))
    );
    assert_eq!(
        identities.policy_hash.as_str(),
        format!("sha256:{}", "2".repeat(64))
    );

    // The projection is rebuilt from the recovered row, so a renderer
    // reconnecting after restart sees the same affordances.
    assert!(recovered.projection.control.resume_allowed);
    assert!(!recovered.projection.terminal);
    assert_eq!(recovered.projection.state, BmadSessionState::Paused);

    // The event log explains every accepted move, in order, with a chain
    // whose links join end to end.
    let log = store.bmad_session_event_log(SESSION)?;
    assert_eq!(log.len(), 4);
    assert_eq!(
        log.iter()
            .map(|row| row.command.as_str())
            .collect::<Vec<_>>(),
        vec!["resolve", "mark_ready", "start", "pause"]
    );
    for (index, row) in log.iter().enumerate() {
        assert_eq!(row.from_version, index as i64 + 1);
        assert_eq!(row.to_version, index as i64 + 2);
        if index > 0 {
            assert_eq!(row.previous_evidence_hash, log[index - 1].evidence_hash);
        }
    }
    assert_eq!(
        log.last().expect("last event").evidence_hash,
        recovered.session.evidence_hash.as_str()
    );
    Ok(())
}

#[test]
fn bmad_sessions_reject_stale_writers_and_replayed_versions(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: two concurrent advances of the same session both commit,
    // producing a lifecycle whose event log cannot explain its own state.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = session();
    store.create_bmad_session(&value, WORKSPACE, 1_700_000_000_000)?;

    let event = apply(&mut value, 1, &BmadSessionCommand::Resolve).expect("resolve");
    store.commit_bmad_session_transition(&value, &event, 1_700_000_000_001)?;

    // A second writer that still believed the session was at version 1
    // produced a competing transition. The row has already moved, so the
    // compare-and-swap finds nothing to update.
    let mut competing = session();
    let competing_event =
        apply(&mut competing, 1, &BmadSessionCommand::Resolve).expect("competing resolve");
    let refused = store
        .commit_bmad_session_transition(&competing, &competing_event, 1_700_000_000_002)
        .expect_err("a stale writer must not commit");
    assert!(matches!(refused, StoreError::Inconsistent));

    // The stored session is untouched by the refused write.
    let stored = store.load_bmad_session(SESSION)?.expect("session");
    assert_eq!(stored.session.version.get(), 2);
    assert_eq!(stored.session.state, BmadSessionState::Resolving);
    assert_eq!(store.bmad_session_event_log(SESSION)?.len(), 1);

    // A duplicate session identifier cannot be created over an existing one.
    let duplicate = store
        .create_bmad_session(&session(), WORKSPACE, 1_700_000_000_003)
        .expect_err("session identifiers are unique");
    assert!(matches!(duplicate, StoreError::AlreadyConsumed));
    Ok(())
}

#[test]
fn bmad_sessions_persist_answers_proposals_and_artifacts_across_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: recorded answers and inert tool proposals lived only in
    // the running process, so a resumed skill re-asked questions the user had
    // already answered and lost its pending approvals.
    let directory = tempfile::tempdir()?;
    let mut value = session();

    let input: desktop_runtime::generated_contracts::BmadSessionInputRequest =
        serde_json::from_value(json!({
            "requestId": "request_01J00000000000000000000000",
            "prompt": "Name the output consumer.",
            "responseSchemaHash": format!("sha256:{}", "4".repeat(64))
        }))?;
    let answer: desktop_runtime::generated_contracts::BmadSessionRecordedAnswer =
        serde_json::from_value(json!({
            "requestId": "request_01J00000000000000000000000",
            "answerHash": format!("sha256:{}", "5".repeat(64))
        }))?;
    let proposal: desktop_runtime::generated_contracts::BmadSessionToolProposal =
        serde_json::from_value(json!({
            "proposalId": "proposal_01J00000000000000000000000",
            "planHash": format!("sha256:{}", "6".repeat(64)),
            "status": "proposed"
        }))?;

    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.create_bmad_session(&value, WORKSPACE, 1_700_000_000_000)?;
        let mut version = value.version.get();
        for (index, command) in [
            BmadSessionCommand::Resolve,
            BmadSessionCommand::RequireInput(input),
            BmadSessionCommand::ProvideAnswer(answer),
            BmadSessionCommand::MarkReady,
            BmadSessionCommand::Start,
            BmadSessionCommand::ProposeTool(proposal),
            BmadSessionCommand::AttachArtifact("artifact_01J00000000000000000000000".to_owned()),
        ]
        .into_iter()
        .enumerate()
        {
            let event = apply(&mut value, version, &command).expect("transition");
            store.commit_bmad_session_transition(
                &value,
                &event,
                1_700_000_000_001 + index as u64,
            )?;
            version = event.to_version;
        }
    }

    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let recovered = store.load_bmad_session(SESSION)?.expect("session");
    assert_eq!(recovered.session.state, BmadSessionState::Running);
    assert_eq!(recovered.session.answers.len(), 1);
    assert_eq!(recovered.session.tool_proposals.len(), 1);
    assert_eq!(recovered.session.artifacts.len(), 1);
    // The answered request is not re-asked on resume.
    assert!(recovered.session.input_request.is_none());
    assert_eq!(recovered.projection.answer_count, 1);
    assert_eq!(recovered.projection.tool_proposal_count, 1);
    assert_eq!(recovered.projection.artifact_count, 1);
    Ok(())
}

#[test]
fn bmad_sessions_treat_redelivered_commands_as_no_ops() -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: a retried command after an ambiguous crash appended a
    // second identical proposal and desynchronized the version chain.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = session();
    store.create_bmad_session(&value, WORKSPACE, 1_700_000_000_000)?;

    let mut version = value.version.get();
    for command in [
        BmadSessionCommand::Resolve,
        BmadSessionCommand::MarkReady,
        BmadSessionCommand::Start,
    ] {
        let event = apply(&mut value, version, &command).expect("transition");
        store.commit_bmad_session_transition(&value, &event, 1_700_000_000_001)?;
        version = event.to_version;
    }

    let artifact =
        BmadSessionCommand::AttachArtifact("artifact_01J00000000000000000000000".to_owned());
    let first = apply(&mut value, version, &artifact).expect("attach");
    store.commit_bmad_session_transition(&value, &first, 1_700_000_000_002)?;

    // The redelivery is recognised as already applied, so committing it
    // writes nothing at all rather than erroring or duplicating.
    let repeat = apply(&mut value, first.to_version, &artifact).expect("redelivered attach");
    assert!(repeat.idempotent);
    store.commit_bmad_session_transition(&value, &repeat, 1_700_000_000_003)?;

    let recovered = store.load_bmad_session(SESSION)?.expect("session");
    assert_eq!(recovered.session.artifacts.len(), 1);
    assert_eq!(recovered.session.version.get(), first.to_version);
    assert_eq!(
        store.bmad_session_event_log(SESSION)?.len(),
        first.to_version as usize - 1
    );
    Ok(())
}

#[test]
fn bmad_sessions_reject_mismatched_workspace_and_non_initial_versions(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: a session created under one workspace identity but
    // indexed under another escapes that workspace's retention and erasure.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    let mismatched = store
        .create_bmad_session(&session(), "workspace_01J0000000000000000000000Z", 1)
        .expect_err("workspace identity must match the bound identity");
    assert!(matches!(mismatched, StoreError::Inconsistent));

    let mut advanced = session();
    apply(&mut advanced, 1, &BmadSessionCommand::Resolve).expect("resolve");
    let non_initial = store
        .create_bmad_session(&advanced, WORKSPACE, 1)
        .expect_err("creation must start at version 1");
    assert!(matches!(non_initial, StoreError::Inconsistent));

    assert!(store.load_bmad_session(SESSION)?.is_none());
    Ok(())
}
