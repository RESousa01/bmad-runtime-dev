//! Parity and substitution coverage for the generic capability lifecycle
//! (readiness Task 6): the same reviewed D2 flow drives two different
//! capabilities, and every cross-capability substitution fails closed.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(feature = "deterministic-help")]

use desktop_cloud::{
    AuthorizedModelRequest, CloudError, DeterministicModelTransport, DispatchedModelRequest,
    RawModelOutput,
};
use desktop_egress::{
    ContextCandidate, ContextClassification, ContextEgressManifest, ContextPreparer, EgressLimits,
    ModelInvocationBinding, ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput,
    RetentionMode,
};
use desktop_runtime::{
    sha256_bytes, BmadCandidateChange, BmadCapabilityOutput, BmadClosureCapabilityId,
    BmadDocumentArtifact, BmadDocumentSection, BmadGovernedChangeSet, ContractId,
    RelativeWorkspacePath, Sha256Digest, UnixMillis, BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    BMAD_GOVERNED_CHANGE_SET_SCHEMA,
};
use desktop_store::{KeyProtector, LocalStore, StoreError};

use super::capability_coordinator::{
    capability_purpose, ApproveCapabilityRunInput, BmadCapabilityCoordinator,
    BmadCapabilityCoordinatorError, BmadCapabilityOutputVerifier, BmadCapabilityTransport,
    CancelCapabilityRunInput, CapabilityTerminalReason, PrepareCapabilityRunInput,
    SubmitCapabilityRunInput,
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

struct FixtureTransport;

impl BmadCapabilityTransport for FixtureTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        DeterministicModelTransport.send_fixture(request, deterministic_fixture.to_owned(), now)
    }
}

struct CannedVerifier {
    output: BmadCapabilityOutput,
    expected_capability: &'static str,
}

impl BmadCapabilityOutputVerifier for CannedVerifier {
    fn verify(
        &self,
        capability_id: &BmadClosureCapabilityId,
        _output: &RawModelOutput,
    ) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
        if capability_id.as_str() != self.expected_capability {
            return Err(BmadCapabilityCoordinatorError::CapabilityBindingMismatch);
        }
        Ok(self.output.clone())
    }
}

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("contract id")
}

fn digest(seed: u8) -> Sha256Digest {
    sha256_bytes(&[seed])
}

fn document_output() -> BmadCapabilityOutput {
    BmadCapabilityOutput::DocumentArtifact(
        BmadDocumentArtifact::new(
            "Product brief".to_owned(),
            vec![BmadDocumentSection {
                heading: "Problem".to_owned(),
                body: "A bounded problem statement.".to_owned(),
            }],
            vec![],
            vec![],
            None,
        )
        .expect("document artifact"),
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
        .expect("change set"),
    )
}

fn manifest_and_binding(
    capability: &BmadClosureCapabilityId,
    output_schema_id: &str,
    created_at: UnixMillis,
) -> (ContextEgressManifest, ModelInvocationBinding) {
    let purpose = capability_purpose(capability);
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_01J00000000000000000000000"),
            project_ref: id("project_01J00000000000000000000000"),
            run_ref: id("run_01J00000000000000000000000"),
            purpose: purpose.clone(),
            model_role: "planning".to_owned(),
            canonical_output_schema_id: id(output_schema_id),
            canonical_output_schema_hash: digest(1),
            provider_profile_hash: digest(2),
            model_profile_hash: digest(3),
            deployment_hash: digest(4),
            policy_hash: digest(5),
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at,
            expires_at: UnixMillis(created_at.0 + 10 * 60 * 1_000),
            limits: EgressLimits {
                maximum_context_items: 10,
                maximum_context_bytes: 65_536,
                maximum_token_estimate: 65_536,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_01J00000000000000000000000"),
                relative_label: RelativeWorkspacePath::new("docs/context.md")
                    .expect("relative path"),
                semantic_role: "primary_context".to_owned(),
                language: Some("markdown".to_owned()),
                classification: ContextClassification::Internal,
                content: "Reviewed context body.".to_owned(),
            }],
            exclusions: vec![],
        })
        .expect("sealed manifest");
    let binding = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("modelreq_01J00000000000000000000000"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("install_01J00000000000000000000000"),
        session_authority_hash: digest(6),
        manifest_hash: manifest.manifest_hash,
        purpose,
        model_role: manifest.draft.model_role.clone(),
        canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
        canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
        provider_profile_hash: manifest.draft.provider_profile_hash,
        model_profile_hash: manifest.draft.model_profile_hash,
        deployment_hash: manifest.draft.deployment_hash,
        policy_hash: manifest.draft.policy_hash,
        region: manifest.draft.region.clone(),
        retention_mode: manifest.draft.retention_mode,
        consent_disclosure_hash: digest(7),
    }
    .seal()
    .expect("sealed binding");
    (manifest, binding)
}

fn prepare_input(
    capability: &BmadClosureCapabilityId,
    output_schema_id: &str,
    run_suffix: &str,
    created_at: UnixMillis,
) -> PrepareCapabilityRunInput {
    let (manifest, binding) = manifest_and_binding(capability, output_schema_id, created_at);
    PrepareCapabilityRunInput {
        capability_id: capability.clone(),
        workspace_id: id("workspace_01J00000000000000000000000"),
        workspace_grant_epoch: 1,
        workspace_context_read_epoch: 1,
        run_id: id(&format!("caprun_01J0000000000000000000{run_suffix}")),
        instruction_hash: digest(8),
        output_schema_id: output_schema_id.to_owned(),
        manifest,
        invocation_binding: binding,
        deterministic_fixture: "{\"fixture\":\"capability\"}".to_owned(),
        created_at,
    }
}

fn capability(value: &str) -> BmadClosureCapabilityId {
    BmadClosureCapabilityId::new(value).expect("capability id")
}

struct Lifecycle {
    coordinator: BmadCapabilityCoordinator,
    capability: BmadClosureCapabilityId,
    manifest_hash: Sha256Digest,
    decision_id: Option<ContractId>,
    run_id: ContractId,
}

fn prepared_lifecycle(capability_id: &str, output_schema_id: &str, run_suffix: &str) -> Lifecycle {
    let capability = capability(capability_id);
    let mut coordinator = BmadCapabilityCoordinator::new();
    let input = prepare_input(
        &capability,
        output_schema_id,
        run_suffix,
        UnixMillis(10_000),
    );
    let run_id = input.run_id.clone();
    let review = coordinator.prepare(input).expect("prepared review");
    assert_eq!(review.capability_id, capability_id);
    Lifecycle {
        coordinator,
        capability,
        manifest_hash: review.manifest_hash,
        decision_id: None,
        run_id,
    }
}

fn approve(lifecycle: &mut Lifecycle) {
    let approved = lifecycle
        .coordinator
        .approve(&ApproveCapabilityRunInput {
            capability_id: lifecycle.capability.clone(),
            manifest_hash: lifecycle.manifest_hash,
            approved_at: UnixMillis(20_000),
        })
        .expect("approved decision");
    lifecycle.decision_id = Some(approved.decision_id);
}

#[test]
fn the_same_lifecycle_drives_two_capabilities_to_their_archetypes(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    for (capability_id, schema, output, expected_kind, run_suffix) in [
        (
            "bmm:bmad-product-brief",
            BMAD_DOCUMENT_ARTIFACT_SCHEMA,
            document_output(),
            "document_artifact",
            "01",
        ),
        (
            "bmm:bmad-dev-story",
            BMAD_GOVERNED_CHANGE_SET_SCHEMA,
            change_output(),
            "governed_change_set",
            "02",
        ),
    ] {
        let mut lifecycle = prepared_lifecycle(capability_id, schema, run_suffix);
        approve(&mut lifecycle);
        let completed = lifecycle
            .coordinator
            .submit(
                &SubmitCapabilityRunInput {
                    capability_id: lifecycle.capability.clone(),
                    manifest_hash: lifecycle.manifest_hash,
                    decision_id: lifecycle.decision_id.clone().expect("decision"),
                    submitted_at: UnixMillis(30_000),
                },
                &FixtureTransport,
                &CannedVerifier {
                    output,
                    expected_capability: capability_id,
                },
                &store,
            )
            .expect("completed run");
        assert_eq!(completed.result_kind, expected_kind);
        let record = store
            .bmad_capability_run(&lifecycle.run_id)?
            .expect("durable run");
        assert_eq!(record.capability_id, capability_id);
        assert_eq!(record.result_kind.as_deref(), Some(expected_kind));
    }
    Ok(())
}

#[test]
fn prepare_rejects_manifests_minted_for_another_capability() {
    let brief = capability("bmm:bmad-product-brief");
    let story = capability("bmm:bmad-dev-story");
    let mut input = prepare_input(
        &story,
        BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        "01",
        UnixMillis(10_000),
    );
    // The manifest and binding carry the story capability's purpose.
    input.capability_id = brief;
    let mut coordinator = BmadCapabilityCoordinator::new();
    assert_eq!(
        coordinator.prepare(input).unwrap_err(),
        BmadCapabilityCoordinatorError::CapabilityBindingMismatch
    );
}

#[test]
fn prepare_rejects_archetype_substitution_in_the_binding() {
    let brief = capability("bmm:bmad-product-brief");
    let mut input = prepare_input(
        &brief,
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "01",
        UnixMillis(10_000),
    );
    // Declaring a different run archetype than the reviewed binding fails.
    input.output_schema_id = BMAD_GOVERNED_CHANGE_SET_SCHEMA.to_owned();
    let mut coordinator = BmadCapabilityCoordinator::new();
    assert_eq!(
        coordinator.prepare(input).unwrap_err(),
        BmadCapabilityCoordinatorError::ResultArchetypeMismatch
    );
}

#[test]
fn decisions_bind_to_one_capability_and_manifest() {
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "01",
    );
    // Approving under another capability identity fails.
    assert_eq!(
        lifecycle
            .coordinator
            .approve(&ApproveCapabilityRunInput {
                capability_id: capability("bmm:bmad-dev-story"),
                manifest_hash: lifecycle.manifest_hash,
                approved_at: UnixMillis(20_000),
            })
            .unwrap_err(),
        BmadCapabilityCoordinatorError::CapabilityBindingMismatch
    );
    // Approving a foreign manifest hash fails.
    assert_eq!(
        lifecycle
            .coordinator
            .approve(&ApproveCapabilityRunInput {
                capability_id: lifecycle.capability.clone(),
                manifest_hash: desktop_runtime::sha256_bytes(b"foreign manifest"),
                approved_at: UnixMillis(20_000),
            })
            .unwrap_err(),
        BmadCapabilityCoordinatorError::ConsentBindingMismatch
    );
    approve(&mut lifecycle);
    // Submitting under another capability identity fails and preserves state.
    let directory = tempfile::tempdir().expect("tempdir");
    let store = LocalStore::open(directory.path(), &TestProtector).expect("store");
    assert_eq!(
        lifecycle
            .coordinator
            .submit(
                &SubmitCapabilityRunInput {
                    capability_id: capability("bmm:bmad-dev-story"),
                    manifest_hash: lifecycle.manifest_hash,
                    decision_id: lifecycle.decision_id.clone().expect("decision"),
                    submitted_at: UnixMillis(30_000),
                },
                &FixtureTransport,
                &CannedVerifier {
                    output: change_output(),
                    expected_capability: "bmm:bmad-dev-story",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::CapabilityBindingMismatch
    );
}

#[test]
fn an_output_of_the_wrong_archetype_terminates_the_flow() {
    let directory = tempfile::tempdir().expect("tempdir");
    let store = LocalStore::open(directory.path(), &TestProtector).expect("store");
    let mut lifecycle =
        prepared_lifecycle("bmm:bmad-dev-story", BMAD_GOVERNED_CHANGE_SET_SCHEMA, "01");
    approve(&mut lifecycle);
    // The verifier hands back a document artifact for a change-set run.
    assert_eq!(
        lifecycle
            .coordinator
            .submit(
                &SubmitCapabilityRunInput {
                    capability_id: lifecycle.capability.clone(),
                    manifest_hash: lifecycle.manifest_hash,
                    decision_id: lifecycle.decision_id.clone().expect("decision"),
                    submitted_at: UnixMillis(30_000),
                },
                &FixtureTransport,
                &CannedVerifier {
                    output: document_output(),
                    expected_capability: "bmm:bmad-dev-story",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::ResultArchetypeMismatch
    );
    assert_eq!(
        lifecycle.coordinator.terminal_reason_for_test(),
        Some(CapabilityTerminalReason::OutputRejected)
    );
    // Nothing was persisted for the rejected run.
    assert!(store
        .bmad_capability_run(&lifecycle.run_id)
        .expect("readable store")
        .is_none());
}

#[test]
fn cancelled_and_expired_decisions_never_reach_the_transport() {
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "01",
    );
    approve(&mut lifecycle);
    lifecycle
        .coordinator
        .cancel(&CancelCapabilityRunInput {
            capability_id: lifecycle.capability.clone(),
            manifest_hash: lifecycle.manifest_hash,
            decision_id: lifecycle.decision_id.clone().expect("decision"),
            cancelled_at: UnixMillis(21_000),
        })
        .expect("cancelled");
    let directory = tempfile::tempdir().expect("tempdir");
    let store = LocalStore::open(directory.path(), &TestProtector).expect("store");
    assert_eq!(
        lifecycle
            .coordinator
            .submit(
                &SubmitCapabilityRunInput {
                    capability_id: lifecycle.capability.clone(),
                    manifest_hash: lifecycle.manifest_hash,
                    decision_id: lifecycle.decision_id.clone().expect("decision"),
                    submitted_at: UnixMillis(22_000),
                },
                &FixtureTransport,
                &CannedVerifier {
                    output: document_output(),
                    expected_capability: "bmm:bmad-product-brief",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::Unauthorized
    );

    // A fresh approval that outlives its window expires at submit.
    let mut expired = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "02",
    );
    approve(&mut expired);
    assert_eq!(
        expired
            .coordinator
            .submit(
                &SubmitCapabilityRunInput {
                    capability_id: expired.capability.clone(),
                    manifest_hash: expired.manifest_hash,
                    decision_id: expired.decision_id.clone().expect("decision"),
                    submitted_at: UnixMillis(20_000 + 6 * 60 * 1_000),
                },
                &FixtureTransport,
                &CannedVerifier {
                    output: document_output(),
                    expected_capability: "bmm:bmad-product-brief",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::ConsentExpired
    );
    assert_eq!(
        expired.coordinator.terminal_reason_for_test(),
        Some(CapabilityTerminalReason::ConsentExpired)
    );
}

#[test]
fn invalidation_withdraws_the_active_flow() {
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "01",
    );
    approve(&mut lifecycle);
    lifecycle.coordinator.invalidate();
    assert_eq!(
        lifecycle
            .coordinator
            .approve(&ApproveCapabilityRunInput {
                capability_id: lifecycle.capability.clone(),
                manifest_hash: lifecycle.manifest_hash,
                approved_at: UnixMillis(25_000),
            })
            .unwrap_err(),
        BmadCapabilityCoordinatorError::Unauthorized
    );
    assert_eq!(
        lifecycle.coordinator.terminal_reason_for_test(),
        Some(CapabilityTerminalReason::Invalidated)
    );
}
