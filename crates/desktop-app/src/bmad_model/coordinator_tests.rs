#![allow(clippy::expect_used)]

mod prepare {
    #[cfg(feature = "deterministic-help")]
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "deterministic-help")]
    use std::sync::Arc;

    #[cfg(feature = "deterministic-help")]
    use desktop_cloud::{
        AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput,
    };
    #[cfg(feature = "deterministic-help")]
    use desktop_runtime::sha256_bytes;
    use desktop_runtime::{
        BmadHelpIntent, ContractId, MethodSessionRepository, MethodState, UnixMillis,
    };

    #[cfg(not(feature = "deterministic-help"))]
    use super::super::coordinator::BmadHelpCoordinatorError;
    use super::super::coordinator::PrepareBmadHelpReviewInput;
    #[cfg(feature = "deterministic-help")]
    use super::super::coordinator::{
        ApproveBmadHelpReviewInput, BmadHelpCoordinator, BmadHelpCoordinatorError,
        CancelBmadHelpReviewInput, SubmitBmadHelpReviewInput,
    };
    #[cfg(feature = "deterministic-help")]
    use super::super::transport::BmadHelpTransport;
    use crate::bmad_foundation::{load_bmad_foundation, BmadLoadedFoundation};
    use crate::commands::create_bmad_help_run_for_test;
    use crate::state::HostState;

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("qualified fixture identifier")
    }

    fn foundation() -> BmadLoadedFoundation {
        load_bmad_foundation(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation"),
        )
        .expect("sealed foundation")
    }

    fn ready_workspace_state() -> (HostState, tempfile::TempDir, tempfile::TempDir, String) {
        let storage = tempfile::tempdir().expect("temporary authority store");
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let state = HostState::initialize(Some(storage.path().join("authority")))
            .expect("ready host state");
        let authority = state.ready_authority().expect("ready authority");
        let projection = state
            .workspace
            .grant("project_01J00000000000000000000000", workspace.path())
            .expect("workspace grant");
        let binding = state
            .workspace
            .authority_binding(&projection.workspace_id)
            .expect("workspace binding");
        state
            .persist_workspace(
                &authority,
                projection.clone(),
                workspace.path(),
                &binding.root_identity_hash,
                &id("request_01J00000000000000000000001"),
            )
            .expect("persisted workspace");
        drop(authority);
        (state, storage, workspace, projection.workspace_id)
    }

    #[cfg(feature = "deterministic-help")]
    struct CountingDeterministicTransport(Arc<AtomicUsize>);

    #[cfg(feature = "deterministic-help")]
    impl BmadHelpTransport for CountingDeterministicTransport {
        fn send(
            &self,
            request: AuthorizedModelRequest,
            deterministic_fixture: &str,
            now: UnixMillis,
        ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            desktop_cloud::DeterministicModelTransport.send_fixture(
                request,
                deterministic_fixture.to_owned(),
                now,
            )
        }
    }

    #[cfg(feature = "deterministic-help")]
    #[derive(Clone, Copy)]
    enum FailureMode {
        Transport,
        InvalidSchema,
        InvalidReceipt,
    }

    #[cfg(feature = "deterministic-help")]
    struct FailingDeterministicTransport {
        calls: Arc<AtomicUsize>,
        mode: FailureMode,
    }

    #[cfg(feature = "deterministic-help")]
    impl BmadHelpTransport for FailingDeterministicTransport {
        fn send(
            &self,
            request: AuthorizedModelRequest,
            deterministic_fixture: &str,
            now: UnixMillis,
        ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if matches!(self.mode, FailureMode::Transport) {
                return Err(CloudError::TransportFailed);
            }
            let (dispatched, mut output) = desktop_cloud::DeterministicModelTransport
                .send_fixture(request, deterministic_fixture.to_owned(), now)?;
            match self.mode {
                FailureMode::Transport => unreachable!("returned before fixture dispatch"),
                FailureMode::InvalidSchema => {
                    output.payload_json = r#"{"unexpected":"renderer-controlled"}"#.to_owned();
                    output.payload_hash = sha256_bytes(output.payload_json.as_bytes());
                    output.receipt.result_hash = output.payload_hash;
                    output.receipt.output_bytes = u64::try_from(output.payload_json.len())
                        .map_err(|_| CloudError::InvalidModelOutput)?;
                }
                FailureMode::InvalidReceipt => {
                    output.receipt.proof = "substituted-proof".to_owned();
                }
            }
            Ok((dispatched, output))
        }
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn deterministic_prepare_persists_v3_and_returns_only_review_projection() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000020"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            UnixMillis(1_000),
        )
        .expect("retained Help run");
        let renderer_session_id = state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let projection = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(2_000),
                },
            )
            .expect("prepared deterministic review");

        assert_eq!(projection.renderer_session_id, renderer_session_id);
        assert_eq!(projection.workspace_id.as_str(), workspace_id);
        assert_eq!(projection.run_id, creation.run_id);
        assert_eq!(projection.session_id, creation.session_id);
        assert_eq!(projection.context.items.len(), 4);
        assert_eq!(
            projection
                .context
                .items
                .iter()
                .map(|item| item.semantic_role.as_str())
                .collect::<Vec<_>>(),
            [
                "sealed_instruction",
                "current_intent",
                "catalog_candidate",
                "evidence_fact",
            ]
        );
        assert!(projection.development_only);
        assert_eq!(
            projection.destination_label,
            "Deterministic local model — development only"
        );
        assert!(coordinator.has_active_review());
        assert!(!coordinator.active_fixture_for_test().is_empty());

        let authority = state.ready_authority().expect("ready authority");
        let retained = state
            .method_store(&authority)
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load retained Method session")
            .expect("retained Method session");
        assert_eq!(retained.state(), MethodState::ContextReviewRequired);
        assert_eq!(retained.version(), 3);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn approval_is_exact_short_lived_and_transport_free() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000022"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(Box::new(
            CountingDeterministicTransport(Arc::clone(&calls)),
        ));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");

        let wrong = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: sha256_bytes(b"substituted manifest"),
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect_err("manifest substitution");
        assert_eq!(wrong, BmadHelpCoordinatorError::ConsentBindingMismatch);
        assert!(coordinator.has_active_review());

        let approved = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved exact review");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(approved.manifest_hash, review.context.manifest_hash);
        assert!(approved.send_eligible);
        assert!(approved.expires_at <= review.context.expires_at);

        let authority = state.ready_authority().expect("ready authority");
        let retained = state
            .method_store(&authority)
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(retained.state(), MethodState::ContextReviewRequired);
        assert_eq!(retained.version(), 3);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn cancel_and_renderer_rebind_invalidate_without_dispatch() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000023"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(Box::new(
            CountingDeterministicTransport(Arc::clone(&calls)),
        ));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");
        coordinator
            .cancel(
                &state,
                CancelBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id.clone(),
                    cancelled_at: UnixMillis(now.0 + 3),
                },
            )
            .expect("cancelled review");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(coordinator);
        drop(renderer);

        state.bind_renderer("main").expect("renderer rebound");
        assert!(!state.bmad_model.lock().has_active_review());
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn submit_dispatches_once_after_v5_and_atomically_completes_v6() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000024"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(Box::new(
            CountingDeterministicTransport(Arc::clone(&calls)),
        ));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");
        let completed = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id.clone(),
                    submitted_at: UnixMillis(now.0 + 3),
                },
            )
            .expect("completed one-shot request");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let serialized = serde_json::to_string(&completed).expect("safe completed projection");
        assert!(serialized.contains("completed"));
        assert!(!serialized.contains("deterministic-fake-no-trust"));
        assert!(!serialized.contains("providerProfileHash"));

        let duplicate = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id,
                    submitted_at: UnixMillis(now.0 + 4),
                },
            )
            .expect_err("duplicate submit");
        assert_eq!(duplicate, BmadHelpCoordinatorError::ConsentAlreadyConsumed);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let authority = state.ready_workspace_commit().expect("ready authority");
        let session = state
            .method_store(authority.authority())
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(session.state(), MethodState::Completed);
        assert_eq!(session.version(), 6);
        assert!(matches!(
            state
                .latest_bmad_help_run(
                    authority.authority(),
                    &id(&workspace_id),
                    authority.workspace_catalog_version(),
                )
                .expect("latest completed Help"),
            desktop_store::BmadHelpRunLatest::Completed(_)
        ));
    }

    #[cfg(feature = "deterministic-help")]
    fn assert_consumed_submit_failure(
        mode: FailureMode,
        expected_error: BmadHelpCoordinatorError,
        request_id: &str,
    ) {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id(request_id),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() =
            BmadHelpCoordinator::with_transport_for_test(Box::new(FailingDeterministicTransport {
                calls: Arc::clone(&calls),
                mode,
            }));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");
        let error = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id.clone(),
                    submitted_at: UnixMillis(now.0 + 3),
                },
            )
            .expect_err("consumed request must fail closed");
        assert_eq!(error, expected_error);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let replay = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id,
                    submitted_at: UnixMillis(now.0 + 4),
                },
            )
            .expect_err("consumed failure cannot be resumed or replayed");
        assert_eq!(replay, BmadHelpCoordinatorError::ConsentAlreadyConsumed);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let authority = state.ready_workspace_commit().expect("ready authority");
        let session = state
            .method_store(authority.authority())
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(session.state(), MethodState::Advancing);
        assert_eq!(session.version(), 5);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn transport_schema_and_receipt_failures_are_single_use_and_nonresumable() {
        assert_consumed_submit_failure(
            FailureMode::Transport,
            BmadHelpCoordinatorError::TransportFailed,
            "request_01J00000000000000000000025",
        );
        assert_consumed_submit_failure(
            FailureMode::InvalidSchema,
            BmadHelpCoordinatorError::InvalidModelOutput,
            "request_01J00000000000000000000026",
        );
        assert_consumed_submit_failure(
            FailureMode::InvalidReceipt,
            BmadHelpCoordinatorError::ReceiptInvalid,
            "request_01J00000000000000000000027",
        );
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the substitution and expiry assertions intentionally share one approved decision so transport-free state preservation is proven across attempts"
    )]
    fn substituted_or_expired_submit_never_dispatches() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000028"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(Box::new(
            CountingDeterministicTransport(Arc::clone(&calls)),
        ));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");

        let substituted_manifest = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: sha256_bytes(b"substituted manifest"),
                    decision_id: approved.decision_id.clone(),
                    submitted_at: UnixMillis(now.0 + 3),
                },
            )
            .expect_err("substituted manifest");
        assert_eq!(
            substituted_manifest,
            BmadHelpCoordinatorError::ConsentBindingMismatch
        );
        let substituted_decision = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: id("decision_01J00000000000000000000029"),
                    submitted_at: UnixMillis(now.0 + 3),
                },
            )
            .expect_err("substituted decision");
        assert_eq!(
            substituted_decision,
            BmadHelpCoordinatorError::ConsentBindingMismatch
        );
        let expired = coordinator
            .submit(
                &state,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id,
                    submitted_at: UnixMillis(approved.expires_at.0 + 1),
                },
            )
            .expect_err("expired decision");
        assert_eq!(expired, BmadHelpCoordinatorError::ConsentExpired);
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let authority = state.ready_workspace_commit().expect("ready authority");
        let session = state
            .method_store(authority.authority())
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(session.state(), MethodState::ContextReviewRequired);
        assert_eq!(session.version(), 3);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn renderer_and_workspace_authority_drift_clear_send_authority() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000030"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let calls = Arc::new(AtomicUsize::new(0));
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(Box::new(
            CountingDeterministicTransport(Arc::clone(&calls)),
        ));
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                },
            )
            .expect("prepared review");
        coordinator
            .approve(
                &state,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");
        drop(coordinator);
        drop(renderer);

        state.bind_renderer("main").expect("renderer rebound");
        assert!(!state.bmad_model.lock().has_active_review());
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let revoked = state
            .workspace
            .revoke(&workspace_id)
            .expect("workspace revoked");
        assert_eq!(revoked.grant_epoch, 2);
        assert!(state.workspace.authorize_scope(&workspace_id, 1).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[cfg(not(feature = "deterministic-help"))]
    #[test]
    fn default_prepare_fails_offline_before_context_or_state_mutation() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000021"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            UnixMillis(1_000),
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let mut coordinator = state.bmad_model.lock();
        let error = coordinator
            .prepare(
                &state,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(2_000),
                },
            )
            .expect_err("default model composition is offline");

        assert_eq!(error, BmadHelpCoordinatorError::SupportPlaneOffline);
        assert!(!coordinator.has_active_review());
        let authority = state.ready_authority().expect("ready authority");
        let retained = state
            .method_store(&authority)
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load retained Method session")
            .expect("retained Method session");
        assert_eq!(retained.state(), MethodState::Created);
        assert_eq!(retained.version(), 1);
    }
}
