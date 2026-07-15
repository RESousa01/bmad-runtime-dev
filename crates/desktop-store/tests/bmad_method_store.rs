#![allow(clippy::expect_used)]

use desktop_runtime::{
    sha256_bytes, AuthorityRef, BmadCapabilityKey, ContractId, CreateMethodSession,
    MethodAdvanceDisposition, MethodAdvanceRequest, MethodAdvanceResult, MethodArtifactExpectation,
    MethodContextDecision, MethodEvidenceClass, MethodExactBinding, MethodExecutionProfile,
    MethodExecutionProfileData, MethodInvocationModes, MethodModelBinding, MethodModelBindingData,
    MethodPersistenceEvent, MethodResourcePolicy, MethodSession, MethodSessionRepository,
    MethodSessionService, MethodStepTable, UnixMillis,
};
use desktop_store::{KeyProtector, LocalStore, StoreError};
use std::sync::{Arc, Barrier};

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

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid id")
}

fn method_binding(
    artifact_expectations: Vec<MethodArtifactExpectation>,
) -> Result<MethodExactBinding, Box<dyn std::error::Error>> {
    let digest = |label: &str| sha256_bytes(label.as_bytes());
    let execution_profile = MethodExecutionProfile::from_source(
        MethodExecutionProfileData {
            entrypoint_kind: "direct".to_owned(),
            invocation_modes: MethodInvocationModes {
                interactive: true,
                headless: false,
                actions: vec!["create".to_owned()],
            },
            required_runtimes: Vec::new(),
            resource_policy: MethodResourcePolicy {
                entrypoint_timing: "invocation_start".to_owned(),
                resource_timing: "all_declared_at_start".to_owned(),
                declared_resource_paths: Vec::new(),
            },
            declared_tool_intents: Vec::new(),
            state_hints: Vec::new(),
            completion_evidence: Vec::new(),
            customization_profile: "none".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
        },
        digest("execution"),
    )?;
    let model_binding = MethodModelBinding::from_source(
        MethodModelBindingData {
            binding_kind: "method_model".to_owned(),
            provider_id: "test-provider".to_owned(),
            model_id: "test-model".to_owned(),
            deployment_id: "test-deployment".to_owned(),
            model_profile_hash: digest("model-profile"),
            model_capability_hash: digest("model-capability"),
            context_window_profile_hash: digest("context-window"),
            egress_profile_hash: digest("egress"),
            request_schema_hash: digest("request-schema"),
            response_schema_hash: digest("response-schema"),
        },
        digest("model"),
    )?;
    Ok(MethodExactBinding {
        capability_key: BmadCapabilityKey {
            package_version_id: id("pkgver_01J00000000000000000000000"),
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            normalized_action: Some("create".to_owned()),
        },
        package_descriptor_hash: digest("descriptor"),
        package_source_hash: digest("source"),
        instruction_projection_hash: digest("instructions"),
        capability_catalog_hash: digest("catalog"),
        agent_roster_hash: None,
        agent_binding_hash: None,
        agent_binding: None,
        distribution_profile: "sapphirus_package".to_owned(),
        install_profile: "SapphirusManagedV1".to_owned(),
        entrypoint_kind: "direct".to_owned(),
        execution_profile_hash: execution_profile.profile_hash,
        execution_profile,
        validation_profile: "MethodStepWorkflowV6".to_owned(),
        validation_profile_hash: digest("validation"),
        config_graph_hash: digest("config-graph"),
        config_resolution_hash: digest("config"),
        customization_hash: digest("customization"),
        resource_set_hash: digest("resources"),
        model_binding_hash: model_binding.binding_hash,
        model_binding,
        method_schema_hash: digest("schema"),
        egress_profile_hash: digest("egress"),
        artifact_expectations,
    })
}

fn ready_session(
    store: &LocalStore,
) -> Result<(MethodSession, MethodExactBinding, MethodContextDecision), Box<dyn std::error::Error>>
{
    ready_session_with_expectations(store, Vec::new())
}

fn ready_session_with_expectations(
    store: &LocalStore,
    artifact_expectations: Vec<MethodArtifactExpectation>,
) -> Result<(MethodSession, MethodExactBinding, MethodContextDecision), Box<dyn std::error::Error>>
{
    let mut session = MethodSession::create(CreateMethodSession {
        session_id: id("session_01J00000000000000000000000"),
        owner_scope_ref: id("ownerscope_01J00000000000000000000000"),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id("run_01J00000000000000000000000"),
        authority_ref: AuthorityRef {
            authority_kind: "desktop_local_store".to_owned(),
            authority_id: id("authority_01J00000000000000000000000"),
            installation_id: id("install_01J00000000000000000000000"),
            local_store_id: id("store_01J00000000000000000000000"),
            authority_epoch: 1,
        },
        created_at: UnixMillis(1_000),
    })
    .expect("create");
    store.create_method_session(&session)?;
    let digest = |label: &str| sha256_bytes(label.as_bytes());
    let binding = method_binding(artifact_expectations)?;
    session
        .bind_capability(
            1,
            binding.clone(),
            MethodStepTable::new("respond", [("respond", None)]).expect("steps"),
        )
        .expect("bind");
    store.persist_method_transition(&session, 1, MethodPersistenceEvent::CapabilityBound)?;
    session.request_context_review(2).expect("review request");
    store.persist_method_transition(&session, 2, MethodPersistenceEvent::ContextReviewRequested)?;
    let review = MethodContextDecision {
        decision_id: id("decision_01J00000000000000000000000"),
        manifest_hash: digest("manifest"),
        consent_hash: digest("consent"),
        context_digest: digest("context"),
        binding_hash: binding.binding_hash().expect("hash"),
        reviewed_at: UnixMillis(2_000),
    };
    session
        .record_context_review(3, review.clone())
        .expect("review");
    store.persist_method_transition(&session, 3, MethodPersistenceEvent::ContextReviewAccepted)?;
    Ok((session, binding, review))
}

#[test]
fn method_repository_atomically_consumes_one_decision_and_recovers_after_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;

    let request = MethodAdvanceRequest {
        invocation_id: id("invoke_01J00000000000000000000000"),
        idempotency_key: "method-advance-1".to_owned(),
        decision_id: review.decision_id.clone(),
        expected_version: session.version(),
    };
    let receipt = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        request.clone(),
    )?;
    assert_eq!(
        store.begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &binding,
            request.clone(),
        )?,
        receipt
    );
    let mut changed_version = request.clone();
    changed_version.expected_version = 99;
    assert!(store
        .begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &binding,
            changed_version,
        )
        .is_err());

    let mut wrong_authority = session.scope();
    wrong_authority.authority_ref.authority_id = id("authority_01J99999999999999999999999");
    assert!(store
        .begin_method_advance(&wrong_authority, &session.session_id(), &binding, request,)
        .is_err());

    let second = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        MethodAdvanceRequest {
            invocation_id: id("invoke_01J11111111111111111111111"),
            idempotency_key: "method-advance-2".to_owned(),
            decision_id: review.decision_id,
            expected_version: receipt.aggregate_version,
        },
    );
    assert!(second.is_err());
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("stored session");
    assert_eq!(restored.version(), receipt.aggregate_version);
    assert_eq!(restored.state(), desktop_runtime::MethodState::Advancing);
    Ok(())
}

#[test]
fn fresh_store_reaches_compiled_v9_schema() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(store.schema_version()?, 9);
    let tables = store.schema_table_names()?;
    assert!(tables.contains(&"bmad_method_decision_consumptions".to_owned()));
    assert!(tables.contains(&"bmad_method_sessions".to_owned()));
    Ok(())
}

#[test]
fn fresh_and_v4_upgraded_stores_have_identical_v9_catalogs(
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh_directory = tempfile::tempdir()?;
    let fresh = LocalStore::open(fresh_directory.path(), &TestProtector)?;
    let expected = fresh.schema_catalog()?;

    let upgraded_directory = tempfile::tempdir()?;
    let upgraded = LocalStore::open(upgraded_directory.path(), &TestProtector)?;
    let database_path = upgraded.database_path();
    drop(upgraded);
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         PRAGMA user_version = 4;",
    )?;
    drop(connection);

    let reopened = LocalStore::open(upgraded_directory.path(), &TestProtector)?;
    assert_eq!(reopened.schema_version()?, 9);
    assert_eq!(reopened.schema_catalog()?, expected);
    Ok(())
}

#[test]
fn accepted_result_persists_checkpoint_state_evidence_and_outbox_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let request = MethodAdvanceRequest {
        invocation_id: id("invoke_01J00000000000000000000000"),
        idempotency_key: "checkpoint".to_owned(),
        decision_id: review.decision_id,
        expected_version: 4,
    };
    let receipt =
        store.begin_method_advance(&session.scope(), &session.session_id(), &binding, request)?;
    let mut advancing = store
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("advancing session");
    advancing.accept_result(
        5,
        &receipt.invocation_id,
        desktop_runtime::MethodAdvanceResult {
            disposition: desktop_runtime::MethodAdvanceDisposition::Completed,
            current_step_key: "respond".to_owned(),
            next_step_key: None,
            working_artifact_refs: Vec::new(),
        },
        UnixMillis(3_000),
    )?;
    store.persist_method_transition(&advancing, 5, MethodPersistenceEvent::ResultAccepted)?;
    store.verify_integrity()?;
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("completed session");
    assert_eq!(restored.state(), desktop_runtime::MethodState::Completed);
    assert_eq!(restored.resume().map(|value| value.turn_ordinal), Some(1));
    Ok(())
}

#[test]
fn concurrent_distinct_invocations_cannot_share_one_decision(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let primary = Arc::new(LocalStore::open(directory.path(), &TestProtector)?);
    let (session, binding, review) = ready_session(primary.as_ref())?;
    let secondary = Arc::new(LocalStore::open(directory.path(), &TestProtector)?);
    let stores = [Arc::clone(&primary), secondary];
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for (ordinal, store) in stores.into_iter().enumerate() {
        let barrier = Arc::clone(&barrier);
        let scope = session.scope();
        let session_id = session.session_id();
        let decision_id = review.decision_id.clone();
        let binding = binding.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            store.begin_method_advance(
                &scope,
                &session_id,
                &binding,
                MethodAdvanceRequest {
                    invocation_id: id(if ordinal == 0 {
                        "invoke_01J00000000000000000000000"
                    } else {
                        "invoke_01J11111111111111111111111"
                    }),
                    idempotency_key: format!("race-{ordinal}"),
                    decision_id,
                    expected_version: 4,
                },
            )
        }));
    }
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker did not panic"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    primary.verify_integrity()?;
    let restored = primary
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("stored session");
    assert_eq!(restored.version(), 5);
    Ok(())
}

#[test]
fn method_repository_isolates_owner_project_run_and_authority_scope(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, _, _) = ready_session(&store)?;
    let mut wrong_owner = session.scope();
    wrong_owner.owner_scope_ref = id("ownerscope_01J99999999999999999999999");
    let mut wrong_project = session.scope();
    wrong_project.project_id = id("project_01J99999999999999999999999");
    let mut wrong_run = session.scope();
    wrong_run.run_id = id("run_01J99999999999999999999999");
    let mut wrong_authority = session.scope();
    wrong_authority.authority_ref.authority_id = id("authority_01J99999999999999999999999");
    for scope in [wrong_owner, wrong_project, wrong_run, wrong_authority] {
        assert!(store
            .load_method_session(&scope, &session.session_id())?
            .is_none());
    }
    assert!(store
        .load_method_session(&session.scope(), &session.session_id())?
        .is_some());
    Ok(())
}

#[test]
fn drift_is_checked_before_atomic_decision_consumption_and_can_be_rebound(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let mut rebound = binding.clone();
    rebound.package_source_hash = sha256_bytes(b"updated package source");
    let stale_request = MethodAdvanceRequest {
        invocation_id: id("invoke_01J66666666666666666666666"),
        idempotency_key: "drifted-binding".to_owned(),
        decision_id: review.decision_id,
        expected_version: 4,
    };
    assert!(store
        .begin_method_advance(
            &session.scope(),
            &session.session_id(),
            &rebound,
            stale_request,
        )
        .is_err());
    assert_eq!(
        store
            .load_method_session(&session.scope(), &session.session_id())?
            .expect("ready session")
            .state(),
        desktop_runtime::MethodState::Ready
    );

    let service = MethodSessionService::new(store);
    let rebound_session = service.rebind_invocation(
        &session.scope(),
        &session.session_id(),
        4,
        rebound.clone(),
        MethodStepTable::new("respond", [("respond", None)])?,
    )?;
    assert_eq!(rebound_session.version(), 5);
    let review_required =
        service.request_context_review(&session.scope(), &session.session_id(), 5)?;
    assert_eq!(review_required.version(), 6);
    let fresh_review = MethodContextDecision {
        decision_id: id("decision_01J66666666666666666666666"),
        manifest_hash: sha256_bytes(b"new manifest"),
        consent_hash: sha256_bytes(b"new consent"),
        context_digest: sha256_bytes(b"new context"),
        binding_hash: rebound.binding_hash()?,
        reviewed_at: UnixMillis(3_000),
    };
    let ready = service.record_context_review(
        &session.scope(),
        &session.session_id(),
        6,
        fresh_review.clone(),
    )?;
    assert_eq!(ready.version(), 7);
    let (_, receipt) = service.begin_advance(
        &session.scope(),
        &session.session_id(),
        &rebound,
        MethodAdvanceRequest {
            invocation_id: id("invoke_01J77777777777777777777777"),
            idempotency_key: "fresh-rebound-review".to_owned(),
            decision_id: fresh_review.decision_id,
            expected_version: 7,
        },
    )?;
    assert_eq!(receipt.aggregate_version, 8);
    Ok(())
}

#[test]
fn coordinator_authenticates_artifacts_and_enforces_required_expectations(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expectation = MethodArtifactExpectation::from_source(MethodArtifactExpectation {
        expectation_kind: "method_artifact".to_owned(),
        expectation_id: id("expectation_01J00000000000000000000000"),
        artifact_kind: "architecture_document".to_owned(),
        required: true,
        storage_scope: "app_local".to_owned(),
        expected_media_type: "application/json".to_owned(),
        expected_content_schema_hash: Some(sha256_bytes(b"architecture schema")),
        source_output_hint: Some("architecture.md".to_owned()),
        completion_evidence_class: MethodEvidenceClass::Authoritative,
        expectation_hash: sha256_bytes(b"source expectation hash"),
    })?;
    let (session, binding, review) =
        ready_session_with_expectations(&store, vec![expectation.clone()])?;
    let service = MethodSessionService::new(store);
    let (advancing, receipt) = service.begin_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        MethodAdvanceRequest {
            invocation_id: id("invoke_01J88888888888888888888888"),
            idempotency_key: "artifact-evidence".to_owned(),
            decision_id: review.decision_id,
            expected_version: 4,
        },
    )?;
    let missing = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        &receipt.invocation_id,
        MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::Completed,
            current_step_key: "respond".to_owned(),
            next_step_key: None,
            working_artifact_refs: vec![format!("cas://sha256/{}", "a".repeat(64))],
        },
        UnixMillis(4_000),
    );
    assert!(missing.is_err(), "invented CAS references must fail");

    let provenance = advancing.artifact_provenance_for(&receipt.invocation_id)?;
    let artifact_ref = service.repository().put_method_artifact(
        &provenance,
        &expectation,
        br#"{"kind":"architecture"}"#,
    )?;
    let mut cross_session = provenance.clone();
    cross_session.session_id = id("session_01J99999999999999999999999");
    assert!(service
        .repository()
        .validate_method_artifact_refs(
            &cross_session,
            &binding,
            MethodAdvanceDisposition::Completed,
            std::slice::from_ref(&artifact_ref),
        )
        .is_err());
    let completed = service.accept_result(
        &session.scope(),
        &session.session_id(),
        5,
        &receipt.invocation_id,
        MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::Completed,
            current_step_key: "respond".to_owned(),
            next_step_key: None,
            working_artifact_refs: vec![artifact_ref],
        },
        UnixMillis(4_000),
    )?;
    assert_eq!(completed.state(), desktop_runtime::MethodState::Completed);
    Ok(())
}

#[test]
fn failed_result_transaction_leaves_state_and_evidence_unadvanced(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, binding, review) = ready_session(&store)?;
    let receipt = store.begin_method_advance(
        &session.scope(),
        &session.session_id(),
        &binding,
        MethodAdvanceRequest {
            invocation_id: id("invoke_01J99999999999999999999999"),
            idempotency_key: "injected-failure".to_owned(),
            decision_id: review.decision_id,
            expected_version: 4,
        },
    )?;
    drop(store);
    let connection = rusqlite::Connection::open(directory.path().join("authority.sqlite3"))?;
    connection.execute_batch(
        "CREATE TRIGGER reject_method_result_evidence
         BEFORE INSERT ON evidence_events
         WHEN NEW.event_type = 'bmad.method.result_accepted'
         BEGIN SELECT RAISE(ABORT, 'injected result evidence failure'); END;",
    )?;
    drop(connection);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let mut advancing = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("advancing state");
    advancing.accept_result(
        5,
        &receipt.invocation_id,
        MethodAdvanceResult {
            disposition: MethodAdvanceDisposition::Completed,
            current_step_key: "respond".to_owned(),
            next_step_key: None,
            working_artifact_refs: Vec::new(),
        },
        UnixMillis(5_000),
    )?;
    assert!(reopened
        .persist_method_transition(&advancing, 5, MethodPersistenceEvent::ResultAccepted)
        .is_err());
    let retained = reopened
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("retained advancing state");
    assert_eq!(retained.version(), 5);
    assert_eq!(retained.state(), desktop_runtime::MethodState::Advancing);
    assert!(retained.checkpoints().is_empty());
    Ok(())
}

#[test]
fn migration_interruptions_roll_back_and_reopen_to_complete_v9(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let expected = store.schema_catalog()?;
    let database_path = store.database_path();
    drop(store);
    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         DROP TABLE outbox;
         DROP TABLE evidence_events;
         DROP TABLE aggregates;
         DROP TABLE spec_consumptions;
         DROP TABLE payloads;
         DROP TABLE store_meta;
         PRAGMA user_version = 0;",
    )?;
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE store_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;",
    )?;
    drop(connection);
    let after_base_interrupt = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(after_base_interrupt.schema_catalog()?, expected);
    drop(after_base_interrupt);

    let connection = rusqlite::Connection::open(&database_path)?;
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE bmad_help_run_creations;
         DROP TABLE bmad_builder_analysis_decisions;
         DROP TABLE bmad_builder_analyses;
         DROP TABLE bmad_builder_revisions;
         DROP TABLE bmad_builder_drafts;
         DROP TABLE bmad_method_artifacts;
         DROP TABLE bmad_method_decision_consumptions;
         DROP TABLE bmad_method_checkpoints;
         DROP TABLE bmad_method_sessions;
         PRAGMA user_version = 4;",
    )?;
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE bmad_method_artifacts (content_hash TEXT PRIMARY KEY) STRICT;",
    )?;
    drop(connection);
    let after_v5_interrupt = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(after_v5_interrupt.schema_catalog()?, expected);
    Ok(())
}

#[test]
fn migration_failure_can_open_retained_method_history_read_only(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let (session, _, _) = ready_session(&store)?;
    let database_path = store.database_path();
    drop(store);
    let connection = rusqlite::Connection::open(database_path)?;
    connection.execute_batch("DROP TABLE bmad_method_checkpoints;")?;
    drop(connection);
    assert!(LocalStore::open(directory.path(), &TestProtector).is_err());

    let recovery = LocalStore::open_read_only_recovery(directory.path(), &TestProtector)?;
    assert_eq!(recovery.schema_version()?, 9);
    assert!(!recovery
        .schema_table_names()?
        .contains(&"bmad_method_checkpoints".to_owned()));
    let retained = recovery
        .load_method_session(&session.scope(), &session.session_id())?
        .expect("retained session remains readable");
    assert_eq!(retained.state(), desktop_runtime::MethodState::Ready);
    Ok(())
}
