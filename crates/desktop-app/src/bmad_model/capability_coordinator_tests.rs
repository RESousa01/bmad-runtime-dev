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
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
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
                    workspace_id: id("workspace_01J00000000000000000000000"),
                    workspace_grant_epoch: 1,
                    workspace_context_read_epoch: 1,
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
                workspace_id: id("workspace_01J00000000000000000000000"),
                workspace_grant_epoch: 1,
                workspace_context_read_epoch: 1,
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
                workspace_id: id("workspace_01J00000000000000000000000"),
                workspace_grant_epoch: 1,
                workspace_context_read_epoch: 1,
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
                    workspace_id: id("workspace_01J00000000000000000000000"),
                    workspace_grant_epoch: 1,
                    workspace_context_read_epoch: 1,
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
                    workspace_id: id("workspace_01J00000000000000000000000"),
                    workspace_grant_epoch: 1,
                    workspace_context_read_epoch: 1,
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
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
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
                    workspace_id: id("workspace_01J00000000000000000000000"),
                    workspace_grant_epoch: 1,
                    workspace_context_read_epoch: 1,
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
                    workspace_id: id("workspace_01J00000000000000000000000"),
                    workspace_grant_epoch: 1,
                    workspace_context_read_epoch: 1,
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
                workspace_id: id("workspace_01J00000000000000000000000"),
                workspace_grant_epoch: 1,
                workspace_context_read_epoch: 1,
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

fn submit_input(lifecycle: &Lifecycle, submitted_at: UnixMillis) -> SubmitCapabilityRunInput {
    SubmitCapabilityRunInput {
        workspace_id: id("workspace_01J00000000000000000000000"),
        workspace_grant_epoch: 1,
        workspace_context_read_epoch: 1,
        capability_id: lifecycle.capability.clone(),
        manifest_hash: lifecycle.manifest_hash,
        decision_id: lifecycle.decision_id.clone().expect("decision"),
        submitted_at,
    }
}

#[test]
fn split_dispatch_records_the_result_and_conflicts_while_in_flight(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "07",
    );
    approve(&mut lifecycle);

    let dispatch = lifecycle
        .coordinator
        .begin_dispatch(&submit_input(&lifecycle, UnixMillis(30_000)))
        .expect("dispatch begun");

    // The run is parked in `Dispatching`; the coordinator is single-active, so a
    // fresh prepare must fail closed while the round trip runs unlocked.
    assert_eq!(
        lifecycle
            .coordinator
            .prepare(prepare_input(
                &lifecycle.capability,
                BMAD_DOCUMENT_ARTIFACT_SCHEMA,
                "08",
                UnixMillis(10_000),
            ))
            .unwrap_err(),
        BmadCapabilityCoordinatorError::Conflict
    );

    // The caller runs the transport with the coordinator lock released.
    let (_dispatched, raw_output) = FixtureTransport
        .send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.submitted_at,
        )
        .expect("transport output");

    let completed = lifecycle
        .coordinator
        .finalize_dispatch(
            dispatch.dispatch_id,
            &raw_output,
            &CannedVerifier {
                output: document_output(),
                expected_capability: "bmm:bmad-product-brief",
            },
            &store,
        )
        .expect("completed run");
    assert_eq!(completed.result_kind, "document_artifact");
    let record = store
        .bmad_capability_run(&lifecycle.run_id)?
        .expect("durable run");
    assert_eq!(record.result_kind.as_deref(), Some("document_artifact"));
    Ok(())
}

#[test]
fn invalidate_during_dispatch_drops_the_result() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "09",
    );
    approve(&mut lifecycle);

    let dispatch = lifecycle
        .coordinator
        .begin_dispatch(&submit_input(&lifecycle, UnixMillis(30_000)))
        .expect("dispatch begun");

    // Sign-out / workspace revocation lands while the model round trip runs. This
    // is only possible because the coordinator mutex is released during dispatch.
    lifecycle.coordinator.invalidate();

    let (_dispatched, raw_output) = FixtureTransport
        .send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.submitted_at,
        )
        .expect("transport output");

    assert_eq!(
        lifecycle
            .coordinator
            .finalize_dispatch(
                dispatch.dispatch_id,
                &raw_output,
                &CannedVerifier {
                    output: document_output(),
                    expected_capability: "bmm:bmad-product-brief",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::Unauthorized
    );
    // The unauthorized result was never recorded.
    assert!(store.bmad_capability_run(&lifecycle.run_id)?.is_none());
    assert_eq!(
        lifecycle.coordinator.terminal_reason_for_test(),
        Some(CapabilityTerminalReason::Invalidated)
    );
    Ok(())
}

#[test]
fn a_superseded_dispatch_cannot_finalize_or_abort_a_newer_run(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "0a",
    );
    approve(&mut lifecycle);

    // First dispatch begins; its transport is conceptually still in flight.
    let stale = lifecycle
        .coordinator
        .begin_dispatch(&submit_input(&lifecycle, UnixMillis(30_000)))
        .expect("first dispatch begun");
    let stale_id = stale.dispatch_id;

    // Sign-out invalidates it, then a whole new run is prepared, approved, and
    // dispatched before the first dispatch's caller returns — only reachable
    // because the coordinator mutex is released during dispatch.
    lifecycle.coordinator.invalidate();
    let review = lifecycle
        .coordinator
        .prepare(prepare_input(
            &lifecycle.capability,
            BMAD_DOCUMENT_ARTIFACT_SCHEMA,
            "0b",
            UnixMillis(40_000),
        ))
        .expect("second review");
    let second_run_id = review.run_id.clone();
    let approved = lifecycle
        .coordinator
        .approve(&ApproveCapabilityRunInput {
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
            capability_id: lifecycle.capability.clone(),
            manifest_hash: review.manifest_hash,
            approved_at: UnixMillis(45_000),
        })
        .expect("second approved");
    let second = lifecycle
        .coordinator
        .begin_dispatch(&SubmitCapabilityRunInput {
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
            capability_id: lifecycle.capability.clone(),
            manifest_hash: review.manifest_hash,
            decision_id: approved.decision_id.clone(),
            submitted_at: UnixMillis(50_000),
        })
        .expect("second dispatch begun");
    assert_ne!(stale_id, second.dispatch_id);

    // The first (stale) dispatch's transport returns now. Its finalize must be
    // dropped as a conflict and must NOT touch the second dispatch.
    let (_stale_dispatched, stale_output) = FixtureTransport
        .send(
            stale.authorized_request,
            &stale.deterministic_fixture,
            stale.submitted_at,
        )
        .expect("stale transport output");
    assert_eq!(
        lifecycle
            .coordinator
            .finalize_dispatch(
                stale_id,
                &stale_output,
                &CannedVerifier {
                    output: document_output(),
                    expected_capability: "bmm:bmad-product-brief",
                },
                &store,
            )
            .unwrap_err(),
        BmadCapabilityCoordinatorError::Conflict
    );
    // A stale abort is also a no-op on the newer dispatch.
    lifecycle
        .coordinator
        .abort_dispatch(stale_id, CapabilityTerminalReason::ConsentConsumed);

    // The second dispatch is still live and finalizes into its own run.
    let (_second_dispatched, second_output) = FixtureTransport
        .send(
            second.authorized_request,
            &second.deterministic_fixture,
            second.submitted_at,
        )
        .expect("second transport output");
    let completed = lifecycle
        .coordinator
        .finalize_dispatch(
            second.dispatch_id,
            &second_output,
            &CannedVerifier {
                output: document_output(),
                expected_capability: "bmm:bmad-product-brief",
            },
            &store,
        )
        .expect("second completed run");
    assert_eq!(completed.run_id, second_run_id);
    // The stale (first) run was never recorded; the second one was.
    assert!(store.bmad_capability_run(&lifecycle.run_id)?.is_none());
    assert!(store.bmad_capability_run(&second_run_id)?.is_some());
    Ok(())
}

/// The point of P14: a completed `document_artifact` leaves its decisions in
/// the doc workspace's log, so a *later* run against the same folder starts
/// from them instead of from nothing.
///
/// This is continuity across runs. Within a single run the log is still a
/// projection of the finished artifact rather than the source it was distilled
/// from, which is not what upstream does.
#[test]
fn a_completed_document_run_leaves_its_decisions_in_the_doc_workspace_log(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-product-brief",
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        "14",
    );
    approve(&mut lifecycle);
    let dispatch = lifecycle
        .coordinator
        .begin_dispatch(&SubmitCapabilityRunInput {
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
            capability_id: lifecycle.capability.clone(),
            manifest_hash: lifecycle.manifest_hash,
            decision_id: lifecycle.decision_id.clone().expect("decision"),
            submitted_at: UnixMillis(30_000),
        })
        .expect("dispatch");
    let (_dispatched, raw_output) = FixtureTransport
        .send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.submitted_at,
        )
        .expect("transport output");
    lifecycle
        .coordinator
        .finalize_dispatch(
            dispatch.dispatch_id,
            &raw_output,
            &CannedVerifier {
                output: document_output(),
                expected_capability: "bmm:bmad-product-brief",
            },
            &store,
        )
        .expect("completed run");

    let entries =
        store.memlog_entries("workspace_01J00000000000000000000000", "docs/product-brief")?;
    let rendered: Vec<(&str, &str)> = entries
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.text.as_str()))
        .collect();
    assert_eq!(
        rendered,
        vec![
            ("event", "bmm:bmad-product-brief create: Product brief"),
            ("decision", "Problem: A bounded problem statement."),
        ]
    );
    // The log survives the process: a fresh handle on the same file sees it,
    // which is what makes the next run's context possible at all.
    drop(store);
    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(
        reopened
            .memlog_entries("workspace_01J00000000000000000000000", "docs/product-brief")?
            .len(),
        2
    );
    Ok(())
}

/// A governed change set is not a document, so it leaves no log — continuity
/// is scoped to the artifacts that actually accumulate across runs.
#[test]
fn a_change_set_run_writes_no_memlog() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut lifecycle = prepared_lifecycle(
        "bmm:bmad-code-review",
        BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        "15",
    );
    approve(&mut lifecycle);
    let dispatch = lifecycle
        .coordinator
        .begin_dispatch(&SubmitCapabilityRunInput {
            workspace_id: id("workspace_01J00000000000000000000000"),
            workspace_grant_epoch: 1,
            workspace_context_read_epoch: 1,
            capability_id: lifecycle.capability.clone(),
            manifest_hash: lifecycle.manifest_hash,
            decision_id: lifecycle.decision_id.clone().expect("decision"),
            submitted_at: UnixMillis(30_000),
        })
        .expect("dispatch");
    let (_dispatched, raw_output) = FixtureTransport
        .send(
            dispatch.authorized_request,
            &dispatch.deterministic_fixture,
            dispatch.submitted_at,
        )
        .expect("transport output");
    lifecycle
        .coordinator
        .finalize_dispatch(
            dispatch.dispatch_id,
            &raw_output,
            &CannedVerifier {
                output: change_output(),
                expected_capability: "bmm:bmad-code-review",
            },
            &store,
        )
        .expect("completed run");

    assert!(store
        .memlog_entries("workspace_01J00000000000000000000000", "docs/code-review")?
        .is_empty());
    Ok(())
}
