//! Adversarial coverage for the generic sealed capability run (Task 5).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use desktop_runtime::{
    BmadBuilderDraftFile, BmadBuilderDraftKind, BmadCandidateChange, BmadCapabilityOutput,
    BmadCapabilityRun, BmadCapabilityRunError, BmadCapabilityRunParams, BmadClosureCapabilityId,
    BmadDocumentArtifact, BmadDocumentSection, BmadGovernedChangeSet, BmadInactiveBuilderDraft,
    BMAD_DOCUMENT_ARTIFACT_SCHEMA, BMAD_GOVERNED_CHANGE_SET_SCHEMA,
};
use desktop_runtime::{ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn digest() -> Sha256Digest {
    Sha256Digest::parse(&format!("sha256:{}", "a".repeat(64))).expect("test digest")
}

fn document() -> BmadDocumentArtifact {
    BmadDocumentArtifact::new(
        "Product brief".to_owned(),
        vec![BmadDocumentSection {
            heading: "Problem".to_owned(),
            body: "A bounded problem statement.".to_owned(),
        }],
        vec![id("evidence_01ARZ3NDEKTSV4RRFFQ69G5FAV")],
        vec!["Which launch market?".to_owned()],
        None,
    )
    .expect("valid document artifact")
}

fn run_params(output_schema_id: &str) -> BmadCapabilityRunParams {
    BmadCapabilityRunParams {
        run_id: id("run_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        capability_id: BmadClosureCapabilityId::new("bmm:bmad-product-brief")
            .expect("capability id"),
        workspace_id: id("workspace_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        instruction_hash: digest(),
        context_manifest_hash: digest(),
        output_schema_id: output_schema_id.to_owned(),
        consent_evidence_id: id("consent_01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        created_at: UnixMillis(10_000),
    }
}

fn open_run(output_schema_id: &str) -> BmadCapabilityRun {
    BmadCapabilityRun::open(run_params(output_schema_id)).expect("open run")
}

#[test]
fn capability_ids_admit_only_the_closure_ledger_shape() {
    for valid in ["bmm:bmad-product-brief", "builder:agent.create_rebuild"] {
        assert!(BmadClosureCapabilityId::new(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "bmm:",
        "bmm:AB",
        "shell:rm",
        "bmm:UPPER",
        "builder:with space",
        "bmm:a",
        &format!("bmm:{}", "a".repeat(82)),
        "bmad-product-brief",
    ] {
        assert_eq!(
            BmadClosureCapabilityId::new(invalid).unwrap_err(),
            BmadCapabilityRunError::InvalidCapabilityId,
            "{invalid}"
        );
    }
}

#[test]
fn a_run_declares_exactly_one_reviewed_output_schema() {
    assert_eq!(
        BmadCapabilityRun::open(run_params("sapphirus.some-other-schema.v1")).unwrap_err(),
        BmadCapabilityRunError::UnknownOutputSchema
    );
}

#[test]
fn a_result_cannot_substitute_another_archetype_or_repeat() {
    let mut run = open_run(BMAD_GOVERNED_CHANGE_SET_SCHEMA);
    assert_eq!(
        run.record_result(BmadCapabilityOutput::DocumentArtifact(document()))
            .unwrap_err(),
        BmadCapabilityRunError::ResultArchetypeMismatch
    );

    let change_set = BmadGovernedChangeSet::new(
        "Implements the story.".to_owned(),
        vec![BmadCandidateChange::Create {
            path: RelativeWorkspacePath::new("src/feature.rs").expect("relative path"),
            content: "// body".to_owned(),
        }],
    )
    .expect("valid change set");
    run.record_result(BmadCapabilityOutput::GovernedChangeSet(change_set.clone()))
        .expect("first result");
    assert_eq!(
        run.record_result(BmadCapabilityOutput::GovernedChangeSet(change_set))
            .unwrap_err(),
        BmadCapabilityRunError::ResultAlreadyRecorded
    );
}

#[test]
fn document_artifacts_enforce_reviewed_bounds() {
    assert_eq!(
        BmadDocumentArtifact::new(String::new(), Vec::new(), Vec::new(), Vec::new(), None)
            .unwrap_err(),
        BmadCapabilityRunError::BoundsViolation
    );
    assert_eq!(
        BmadDocumentArtifact::new(
            "t".repeat(201),
            vec![BmadDocumentSection {
                heading: "h".to_owned(),
                body: "b".to_owned(),
            }],
            Vec::new(),
            Vec::new(),
            None,
        )
        .unwrap_err(),
        BmadCapabilityRunError::BoundsViolation
    );
    assert_eq!(
        BmadDocumentArtifact::new(
            "Title".to_owned(),
            vec![BmadDocumentSection {
                heading: "h".to_owned(),
                body: "b".repeat(32_769),
            }],
            Vec::new(),
            Vec::new(),
            None,
        )
        .unwrap_err(),
        BmadCapabilityRunError::BoundsViolation
    );
}

#[test]
fn change_sets_reject_duplicates_and_oversized_content() {
    let path = RelativeWorkspacePath::new("src/a.rs").expect("relative path");
    assert_eq!(
        BmadGovernedChangeSet::new(
            "s".to_owned(),
            vec![
                BmadCandidateChange::Create {
                    path: path.clone(),
                    content: "one".to_owned(),
                },
                BmadCandidateChange::Delete {
                    path,
                    preimage_sha256: digest(),
                },
            ],
        )
        .unwrap_err(),
        BmadCapabilityRunError::DuplicatePath
    );
    assert_eq!(
        BmadGovernedChangeSet::new(
            "s".to_owned(),
            vec![BmadCandidateChange::Create {
                path: RelativeWorkspacePath::new("src/big.rs").expect("relative path"),
                content: "x".repeat(262_145),
            }],
        )
        .unwrap_err(),
        BmadCapabilityRunError::BoundsViolation
    );
    // Absolute and escaping paths cannot even be constructed.
    assert!(RelativeWorkspacePath::new("C:/Windows/evil.dll").is_err());
    assert!(RelativeWorkspacePath::new("../outside.txt").is_err());
}

#[test]
fn builder_drafts_stay_bounded_and_deduplicated() {
    let file = BmadBuilderDraftFile {
        path: RelativeWorkspacePath::new("agent.instructions.md").expect("relative path"),
        content: "# Draft".to_owned(),
    };
    assert!(BmadInactiveBuilderDraft::new(
        BmadBuilderDraftKind::Agent,
        "Draft agent".to_owned(),
        "First draft.".to_owned(),
        vec![file.clone()],
    )
    .is_ok());
    assert_eq!(
        BmadInactiveBuilderDraft::new(
            BmadBuilderDraftKind::Workflow,
            "Draft".to_owned(),
            "note".to_owned(),
            vec![file.clone(), file],
        )
        .unwrap_err(),
        BmadCapabilityRunError::DuplicatePath
    );
}

#[test]
fn serialized_runs_mirror_the_wire_contract() {
    let mut run = open_run(BMAD_DOCUMENT_ARTIFACT_SCHEMA);
    run.record_result(BmadCapabilityOutput::DocumentArtifact(document()))
        .expect("record result");
    let value = serde_json::to_value(&run).expect("serializable run");
    assert_eq!(value["schemaVersion"], "sapphirus.bmad-capability-run.v1");
    assert_eq!(value["capabilityId"], "bmm:bmad-product-brief");
    assert_eq!(value["result"]["resultKind"], "document_artifact");
    assert_eq!(
        value["result"]["documentArtifact"]["schemaVersion"],
        "sapphirus.bmad-document-artifact.v1"
    );
    let serialized = value.to_string();
    for forbidden in [
        "command",
        "toolCall",
        "approval",
        "absolutePath",
        "authority",
    ] {
        assert!(!serialized.contains(forbidden), "{forbidden}");
    }
}
