//! Durable capability-run persistence coverage (readiness Task 5).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use desktop_runtime::{
    BmadCandidateChange, BmadCapabilityOutput, BmadCapabilityRun, BmadCapabilityRunParams,
    BmadClosureCapabilityId, BmadDocumentArtifact, BmadDocumentSection, BmadGovernedChangeSet,
    ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis, BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    BMAD_GOVERNED_CHANGE_SET_SCHEMA,
};
use desktop_store::{KeyProtector, LocalStore, StoreError};

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
    ContractId::new(value).expect("valid contract id")
}

fn digest() -> Sha256Digest {
    Sha256Digest::parse(&format!("sha256:{}", "a".repeat(64))).expect("test digest")
}

fn run(run_id: &str, consent_id: &str, output_schema_id: &str) -> BmadCapabilityRun {
    BmadCapabilityRun::open(BmadCapabilityRunParams {
        run_id: id(run_id),
        capability_id: BmadClosureCapabilityId::new("bmm:bmad-product-brief")
            .expect("capability id"),
        workspace_id: id("workspace_01J00000000000000000000000"),
        instruction_hash: digest(),
        context_manifest_hash: digest(),
        output_schema_id: output_schema_id.to_owned(),
        consent_evidence_id: id(consent_id),
        created_at: UnixMillis(10_000),
    })
    .expect("open run")
}

fn document_output() -> BmadCapabilityOutput {
    BmadCapabilityOutput::DocumentArtifact(
        BmadDocumentArtifact::new(
            "Product brief".to_owned(),
            vec![BmadDocumentSection {
                heading: "Problem".to_owned(),
                body: "A bounded problem statement.".to_owned(),
            }],
            vec![id("evidence_01J00000000000000000000000")],
            vec!["Which launch market?".to_owned()],
            None,
        )
        .expect("valid artifact"),
    )
}

fn change_output() -> BmadCapabilityOutput {
    BmadCapabilityOutput::GovernedChangeSet(
        BmadGovernedChangeSet::new(
            "Implements the story.".to_owned(),
            vec![BmadCandidateChange::Create {
                path: RelativeWorkspacePath::new("src/feature.rs").expect("relative path"),
                content: "// body".to_owned(),
            }],
        )
        .expect("valid change set"),
    )
}

#[test]
fn capability_runs_persist_and_replay_across_restart() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let opened = run(
        "run_01J00000000000000000000001",
        "consent_01J00000000000000000000001",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    );
    store.create_bmad_capability_run(&opened)?;
    store.record_bmad_capability_result(&opened.run_id, &document_output())?;
    drop(store);

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let record = reopened
        .bmad_capability_run(&opened.run_id)?
        .expect("stored run");
    assert_eq!(record.capability_id, "bmm:bmad-product-brief");
    assert_eq!(record.output_schema_id, BMAD_DOCUMENT_ARTIFACT_SCHEMA);
    assert_eq!(record.created_at_ms, 10_000);
    assert_eq!(record.result_kind.as_deref(), Some("document_artifact"));
    let result_json = record.result_json.expect("decrypted result");
    assert!(result_json.contains("\"resultKind\":\"document_artifact\""));
    assert!(result_json.contains("sapphirus.bmad-document-artifact.v1"));
    Ok(())
}

#[test]
fn duplicate_runs_results_and_consents_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let opened = run(
        "run_01J00000000000000000000001",
        "consent_01J00000000000000000000001",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    );
    store.create_bmad_capability_run(&opened)?;
    assert!(matches!(
        store.create_bmad_capability_run(&opened),
        Err(StoreError::AlreadyConsumed)
    ));
    // A different run cannot reuse the consumed consent evidence.
    let reused_consent = run(
        "run_01J00000000000000000000002",
        "consent_01J00000000000000000000001",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    );
    assert!(matches!(
        store.create_bmad_capability_run(&reused_consent),
        Err(StoreError::AlreadyConsumed)
    ));

    store.record_bmad_capability_result(&opened.run_id, &document_output())?;
    assert!(matches!(
        store.record_bmad_capability_result(&opened.run_id, &document_output()),
        Err(StoreError::AlreadyConsumed)
    ));
    Ok(())
}

#[test]
fn archetype_substitution_and_unknown_runs_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let opened = run(
        "run_01J00000000000000000000001",
        "consent_01J00000000000000000000001",
        BMAD_GOVERNED_CHANGE_SET_SCHEMA,
    );
    store.create_bmad_capability_run(&opened)?;
    // A document artifact cannot land in a change-set run.
    assert!(matches!(
        store.record_bmad_capability_result(&opened.run_id, &document_output()),
        Err(StoreError::Inconsistent)
    ));
    // Unknown run identifiers fail closed.
    assert!(matches!(
        store
            .record_bmad_capability_result(&id("run_01J00000000000000000000009"), &change_output()),
        Err(StoreError::Inconsistent)
    ));
    // The declared archetype still works after the rejected attempts.
    store.record_bmad_capability_result(&opened.run_id, &change_output())?;
    let record = store
        .bmad_capability_run(&opened.run_id)?
        .expect("stored run");
    assert_eq!(record.result_kind.as_deref(), Some("governed_change_set"));
    Ok(())
}

#[test]
fn runs_carrying_results_cannot_be_created_directly() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut opened = run(
        "run_01J00000000000000000000001",
        "consent_01J00000000000000000000001",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    );
    opened.record_result(document_output()).expect("record");
    assert!(matches!(
        store.create_bmad_capability_run(&opened),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}
