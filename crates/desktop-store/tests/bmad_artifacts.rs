#![allow(clippy::expect_used)]

//! Restart recovery and optimistic durability for the BMAD artifact lifecycle.

use desktop_runtime::generated_contracts::{BmadArtifact, BmadArtifactState};
use desktop_runtime::{BmadArtifactIntent, BmadDomainRuntime, BmadRuntimePort};
use desktop_store::{BmadArtifactEvent, KeyProtector, LocalStore, StoreError};
use serde_json::json;

const WORKSPACE: &str = "workspace_01J00000000000000000000000";
const ARTIFACT: &str = "artifact_01J00000000000000000000000";
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

fn artifact() -> BmadArtifact {
    serde_json::from_value(json!({
        "artifactId": ARTIFACT,
        "contentHash": format!("sha256:{}", "1".repeat(64)),
        "evidenceHash": format!("sha256:{}", "2".repeat(64)),
        "kind": "document",
        "lineage": {
            "sourceId": "source_01J00000000000000000000000",
            "templateHash": format!("sha256:{}", "3".repeat(64)),
            "inputsHash": format!("sha256:{}", "4".repeat(64)),
            "consumedArtifactIds": ["artifact_01J99999999999999999999999"]
        },
        "revision": 1,
        "schemaVersion": "sapphirus.bmad-artifact.v1",
        "sessionId": SESSION,
        "state": "draft",
        "validation": {
            "declaredChecks": ["schema_ok", "links_resolve"],
            "passedChecks": [],
            "status": "not_run"
        }
    }))
    .expect("generated artifact")
}

/// Applies one runtime-accepted transition and commits it under the store's
/// own transition sequence (independent of the contract's content
/// `revision`; see `bmad_artifacts.rs`'s module doc for why).
#[allow(clippy::too_many_arguments)]
fn advance(
    store: &LocalStore,
    runtime: &BmadDomainRuntime,
    value: &mut BmadArtifact,
    sequence: &mut u64,
    intent: BmadArtifactIntent,
    command: &str,
    now_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let from_state = value.state.to_string();
    let from_sequence = *sequence;
    runtime
        .apply_artifact_intent(value, intent)
        .map_err(|error| format!("{error:?}"))?;
    *sequence += 1;
    store.commit_bmad_artifact_transition(
        value,
        &BmadArtifactEvent {
            artifact_id: value.artifact_id.to_string(),
            to_sequence: *sequence,
            from_sequence,
            command: command.to_owned(),
            from_state,
            to_state: value.state.to_string(),
        },
        now_ms,
    )?;
    Ok(())
}

#[test]
fn bmad_artifacts_restart_recovery_loses_no_accepted_state_or_lineage(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: an artifact's validation, lineage, and completion-hook
    // history lived only in memory, so a resumed skill lost the provenance a
    // downstream skill needed to trust its output.
    let directory = tempfile::tempdir()?;
    let runtime = BmadDomainRuntime;
    let mut value = artifact();
    let mut sequence = 1_u64;

    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.create_bmad_artifact(&value, WORKSPACE, 1_700_000_000_000)?;

        advance(
            &store,
            &runtime,
            &mut value,
            &mut sequence,
            BmadArtifactIntent::Validate {
                passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
            },
            "validate",
            1_700_000_000_001,
        )?;
        advance(
            &store,
            &runtime,
            &mut value,
            &mut sequence,
            BmadArtifactIntent::Approve,
            "approve",
            1_700_000_000_002,
        )?;
        advance(
            &store,
            &runtime,
            &mut value,
            &mut sequence,
            BmadArtifactIntent::Write,
            "write",
            1_700_000_000_003,
        )?;
        advance(
            &store,
            &runtime,
            &mut value,
            &mut sequence,
            BmadArtifactIntent::RecordCompletionHook {
                hook: desktop_runtime::generated_contracts::BmadArtifactCompletionHookRunHook::EmitReceipt,
                status: desktop_runtime::generated_contracts::BmadArtifactCompletionHookRunStatus::Succeeded,
                receipt_hash: Some(desktop_runtime::Sha256Digest::parse(&format!(
                    "sha256:{}",
                    "5".repeat(64)
                ))?),
            },
            "record_completion_hook",
            1_700_000_000_004,
        )?;

        assert_eq!(value.state, BmadArtifactState::Written);
    }

    // Reopen: a different process, a cold store, no in-memory carry-over.
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let recovered = store
        .load_bmad_artifact(ARTIFACT)?
        .expect("artifact survives restart");

    assert_eq!(recovered.artifact.state, BmadArtifactState::Written);
    assert_eq!(recovered.sequence, sequence);
    assert_eq!(recovered.artifact.revision.get(), value.revision.get());
    assert_eq!(recovered.workspace_id, WORKSPACE);
    assert_eq!(
        serde_json::to_value(&recovered.artifact)?,
        serde_json::to_value(&value)?
    );

    // Lineage survives byte-for-byte.
    assert_eq!(
        recovered.artifact.lineage.source_id.as_str(),
        "source_01J00000000000000000000000"
    );
    assert_eq!(recovered.artifact.lineage.consumed_artifact_ids.len(), 1);

    // Completion hooks survive, and there is exactly one — not a duplicate.
    assert_eq!(recovered.artifact.completion_hooks.len(), 1);
    assert_eq!(
        recovered.artifact.completion_hooks[0].status,
        desktop_runtime::generated_contracts::BmadArtifactCompletionHookRunStatus::Succeeded
    );

    // The event log explains every accepted move, in order.
    let log = store.bmad_artifact_event_log(ARTIFACT)?;
    assert_eq!(log.len(), 4);
    assert_eq!(
        log.iter()
            .map(|row| row.command.as_str())
            .collect::<Vec<_>>(),
        vec!["validate", "approve", "write", "record_completion_hook"]
    );
    for (index, row) in log.iter().enumerate() {
        assert_eq!(row.from_sequence, index as i64 + 1);
        assert_eq!(row.to_sequence, index as i64 + 2);
    }
    Ok(())
}

#[test]
fn bmad_artifacts_reject_stale_writers_and_duplicate_identifiers(
) -> Result<(), Box<dyn std::error::Error>> {
    // Break caught: two concurrent advances of the same artifact both commit,
    // producing a lifecycle whose event log cannot explain its own state.
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let runtime = BmadDomainRuntime;
    let mut value = artifact();
    let mut sequence = 1_u64;
    store.create_bmad_artifact(&value, WORKSPACE, 1_700_000_000_000)?;

    advance(
        &store,
        &runtime,
        &mut value,
        &mut sequence,
        BmadArtifactIntent::Validate {
            passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
        },
        "validate",
        1_700_000_000_001,
    )?;

    // A second writer that still believed the artifact was at sequence 1
    // produced a competing transition. The row has already moved, so the
    // compare-and-swap finds nothing to update.
    let mut competing = artifact();
    runtime
        .apply_artifact_intent(
            &mut competing,
            BmadArtifactIntent::Validate {
                passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
            },
        )
        .expect("competing validate");
    let refused = store
        .commit_bmad_artifact_transition(
            &competing,
            &BmadArtifactEvent {
                artifact_id: competing.artifact_id.to_string(),
                to_sequence: 2,
                from_sequence: 1,
                command: "validate".to_owned(),
                from_state: "draft".to_owned(),
                to_state: "validated".to_owned(),
            },
            1_700_000_000_002,
        )
        .expect_err("a stale writer must not commit");
    assert!(matches!(refused, StoreError::Inconsistent));

    let stored = store.load_bmad_artifact(ARTIFACT)?.expect("artifact");
    assert_eq!(stored.sequence, 2);
    assert_eq!(stored.artifact.state, BmadArtifactState::Validated);
    assert_eq!(store.bmad_artifact_event_log(ARTIFACT)?.len(), 1);

    let duplicate = store
        .create_bmad_artifact(&artifact(), WORKSPACE, 1_700_000_000_003)
        .expect_err("artifact identifiers are unique");
    assert!(matches!(duplicate, StoreError::AlreadyConsumed));
    Ok(())
}

#[test]
fn bmad_artifacts_reject_non_initial_creation_revision() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let runtime = BmadDomainRuntime;

    let mut advanced = artifact();
    runtime
        .apply_artifact_intent(
            &mut advanced,
            BmadArtifactIntent::Revise {
                raw_output_hash: desktop_runtime::Sha256Digest::parse(&format!(
                    "sha256:{}",
                    "6".repeat(64)
                ))?,
                content_hash: desktop_runtime::Sha256Digest::parse(&format!(
                    "sha256:{}",
                    "7".repeat(64)
                ))?,
            },
        )
        .expect("revise bumps content revision to 2");
    assert_eq!(advanced.revision.get(), 2);

    let non_initial = store
        .create_bmad_artifact(&advanced, WORKSPACE, 1)
        .expect_err("creation must start at content revision 1");
    assert!(matches!(non_initial, StoreError::Inconsistent));

    assert!(store.load_bmad_artifact(ARTIFACT)?.is_none());
    Ok(())
}
