#![allow(clippy::expect_used)]

use desktop_store::{
    EffectJournalUpsert, EvidenceAppend, ExecutionCheckpointAppend, ExecutionResultAppend,
    KeyProtector, LocalStore, StoreError,
};

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

fn open_store(dir: &tempfile::TempDir) -> LocalStore {
    LocalStore::open(dir.path(), &TestProtector).expect("store opens")
}

fn digest(seed: &str) -> String {
    desktop_runtime::sha256_bytes(seed.as_bytes()).to_string()
}

fn checkpoint_append(checkpoint_id: &str) -> ExecutionCheckpointAppend {
    ExecutionCheckpointAppend {
        checkpoint_id: checkpoint_id.to_owned(),
        workspace_target_hash: digest("target"),
        candidate_hash: digest("candidate"),
        manifest_hash: digest(&format!("manifest:{checkpoint_id}")),
        entry_count: 2,
        checkpoint_json: br#"{"schemaVersion":"sapphirus.local-checkpoint.v1","entries":[]}"#
            .to_vec(),
    }
}

fn journal_upsert(journal_id: &str, checkpoint_id: &str) -> EffectJournalUpsert {
    EffectJournalUpsert {
        journal_id: journal_id.to_owned(),
        execution_id: format!("execution_{journal_id}"),
        checkpoint_id: checkpoint_id.to_owned(),
        candidate_hash: digest("candidate"),
        spec_hash: digest(&format!("spec:{journal_id}")),
        consumption_hash: digest(&format!("consumption:{journal_id}")),
        workspace_id: "workspace_1".to_owned(),
        workspace_grant_epoch: 2,
        state: "prepared".to_owned(),
        journal_json: r#"{"state":"prepared"}"#.to_owned(),
    }
}

fn evidence(stream: &str, event_type: &str, payload: &str) -> EvidenceAppend {
    EvidenceAppend {
        stream_id: stream.to_owned(),
        event_type: event_type.to_owned(),
        payload_hash: digest(payload),
        payload_ref: None,
        correlation_id: "request_1".to_owned(),
        causation_id: None,
        redaction_level: "none".to_owned(),
        retention_class: "standard".to_owned(),
    }
}

fn result_append(journal: &EffectJournalUpsert) -> ExecutionResultAppend {
    ExecutionResultAppend {
        execution_id: journal.execution_id.clone(),
        journal_id: journal.journal_id.clone(),
        checkpoint_id: journal.checkpoint_id.clone(),
        candidate_hash: journal.candidate_hash.clone(),
        spec_hash: journal.spec_hash.clone(),
        consumption_hash: journal.consumption_hash.clone(),
        result_hash: digest(&format!("result:{}", journal.journal_id)),
        result_json: r#"{"files":[]}"#.to_owned(),
        file_count: 2,
        journal_json: r#"{"state":"result_recorded"}"#.to_owned(),
    }
}

fn advance_to_postimages(store: &LocalStore, journal_id: &str) {
    for state in [
        "checkpoint_durable",
        "preconditions_verified",
        "applying",
        "effects_applied",
        "postimages_verified",
    ] {
        store
            .update_effect_journal(
                journal_id,
                state,
                &format!(r#"{{"state":"{state}"}}"#),
                None,
            )
            .expect("transition persists");
    }
}

#[test]
fn records_a_full_execution_lifecycle_durably() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(&dir);

    store
        .persist_execution_checkpoint(&checkpoint_append("checkpoint_1"))
        .expect("checkpoint persists");
    let journal = journal_upsert("journal_1", "checkpoint_1");
    store
        .create_effect_journal(
            &journal,
            &evidence(
                "execution:journal_1",
                "execution.journal-created",
                "created",
            ),
        )
        .expect("journal persists");
    advance_to_postimages(&store, "journal_1");

    store
        .record_execution_result(
            &result_append(&journal),
            &evidence("execution:journal_1", "execution.result-recorded", "result"),
        )
        .expect("result persists");
    store
        .update_effect_journal("journal_1", "completed", r#"{"state":"completed"}"#, None)
        .expect("completion persists");

    let loaded = store
        .load_effect_journal("journal_1")
        .expect("journal loads")
        .expect("journal exists");
    assert_eq!(loaded.state, "completed");
    assert_eq!(loaded.workspace_grant_epoch, 2);

    let result = store
        .load_execution_result(&journal.execution_id)
        .expect("result loads")
        .expect("result exists");
    assert_eq!(result.file_count, 2);

    let (checkpoint_row, checkpoint_bytes) = store
        .load_execution_checkpoint("checkpoint_1")
        .expect("checkpoint loads")
        .expect("checkpoint exists");
    assert_eq!(checkpoint_row.entry_count, 2);
    assert!(checkpoint_bytes.starts_with(b"{\"schemaVersion\""));

    assert!(store.list_open_effect_journals().expect("list").is_empty());
    let recent = store.list_recent_execution_results(10).expect("recent");
    assert_eq!(recent.len(), 1);
    store.verify_integrity().expect("integrity holds");
}

#[test]
fn rejects_result_recording_before_postimages_are_verified() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(&dir);
    store
        .persist_execution_checkpoint(&checkpoint_append("checkpoint_1"))
        .expect("checkpoint persists");
    let journal = journal_upsert("journal_1", "checkpoint_1");
    store
        .create_effect_journal(
            &journal,
            &evidence(
                "execution:journal_1",
                "execution.journal-created",
                "created",
            ),
        )
        .expect("journal persists");

    let premature = store.record_execution_result(
        &result_append(&journal),
        &evidence("execution:journal_1", "execution.result-recorded", "result"),
    );
    assert!(matches!(premature, Err(StoreError::StateConflict)));
    assert!(store
        .load_execution_result(&journal.execution_id)
        .expect("result query")
        .is_none());
}

#[test]
fn rejects_invalid_and_terminal_journal_transitions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(&dir);
    store
        .persist_execution_checkpoint(&checkpoint_append("checkpoint_1"))
        .expect("checkpoint persists");
    let journal = journal_upsert("journal_1", "checkpoint_1");
    store
        .create_effect_journal(
            &journal,
            &evidence(
                "execution:journal_1",
                "execution.journal-created",
                "created",
            ),
        )
        .expect("journal persists");

    let skip =
        store.update_effect_journal("journal_1", "applying", r#"{"state":"applying"}"#, None);
    assert!(matches!(skip, Err(StoreError::StateConflict)));

    store
        .update_effect_journal(
            "journal_1",
            "recovery_required",
            r#"{"state":"recovery_required"}"#,
            Some(&evidence(
                "execution:journal_1",
                "execution.recovery-required",
                "recovery",
            )),
        )
        .expect("recovery marker persists");
    store
        .update_effect_journal(
            "journal_1",
            "manual_review",
            r#"{"state":"manual_review"}"#,
            None,
        )
        .expect("manual review persists");

    let after_terminal =
        store.update_effect_journal("journal_1", "recovered", r#"{"state":"recovered"}"#, None);
    assert!(matches!(after_terminal, Err(StoreError::StateConflict)));

    let open = store.list_open_effect_journals().expect("list");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].state, "manual_review");
}

#[test]
fn enforces_single_use_result_and_checkpoint_uniqueness() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(&dir);
    store
        .persist_execution_checkpoint(&checkpoint_append("checkpoint_1"))
        .expect("checkpoint persists");
    let duplicate_checkpoint =
        store.persist_execution_checkpoint(&checkpoint_append("checkpoint_1"));
    assert!(matches!(
        duplicate_checkpoint,
        Err(StoreError::StateConflict)
    ));

    let journal = journal_upsert("journal_1", "checkpoint_1");
    store
        .create_effect_journal(
            &journal,
            &evidence(
                "execution:journal_1",
                "execution.journal-created",
                "created",
            ),
        )
        .expect("journal persists");
    let duplicate_journal = store.create_effect_journal(
        &journal,
        &evidence(
            "execution:journal_1",
            "execution.journal-created",
            "created",
        ),
    );
    assert!(matches!(duplicate_journal, Err(StoreError::StateConflict)));

    advance_to_postimages(&store, "journal_1");
    store
        .record_execution_result(
            &result_append(&journal),
            &evidence("execution:journal_1", "execution.result-recorded", "result"),
        )
        .expect("result persists");
    let duplicate_result = store.record_execution_result(
        &result_append(&journal),
        &evidence("execution:journal_1", "execution.result-recorded", "result"),
    );
    assert!(matches!(
        duplicate_result,
        Err(StoreError::StateConflict | StoreError::AlreadyConsumed)
    ));
}

#[test]
fn open_journals_survive_reopen_for_boot_reconciliation() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let store = open_store(&dir);
        store
            .persist_execution_checkpoint(&checkpoint_append("checkpoint_1"))
            .expect("checkpoint persists");
        let journal = journal_upsert("journal_1", "checkpoint_1");
        store
            .create_effect_journal(
                &journal,
                &evidence(
                    "execution:journal_1",
                    "execution.journal-created",
                    "created",
                ),
            )
            .expect("journal persists");
        store
            .update_effect_journal(
                "journal_1",
                "checkpoint_durable",
                r#"{"state":"checkpoint_durable"}"#,
                None,
            )
            .expect("transition persists");
    }

    let reopened = open_store(&dir);
    let open = reopened.list_open_effect_journals().expect("list");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].journal_id, "journal_1");
    assert_eq!(open[0].state, "checkpoint_durable");
    reopened
        .update_effect_journal("journal_1", "recovered", r#"{"state":"recovered"}"#, None)
        .expect("no-effect reconciliation persists");
    assert!(reopened
        .list_open_effect_journals()
        .expect("list")
        .is_empty());
    reopened.verify_integrity().expect("integrity holds");
}
