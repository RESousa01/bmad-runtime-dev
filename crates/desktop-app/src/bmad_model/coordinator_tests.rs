#![allow(clippy::expect_used)]

mod prepare {
    #[cfg(feature = "deterministic-help")]
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "deterministic-help")]
    use std::sync::mpsc::{channel, Receiver, Sender};
    #[cfg(feature = "deterministic-help")]
    use std::sync::{Arc, Mutex as StdMutex};
    #[cfg(feature = "deterministic-help")]
    use std::time::Duration;

    #[cfg(feature = "deterministic-help")]
    use desktop_cloud::{
        AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput,
    };
    #[cfg(feature = "deterministic-help")]
    use desktop_runtime::{sha256_bytes, LocalError, Sha256Digest};
    use desktop_runtime::{
        BmadHelpIntent, ContractId, MethodSessionRepository, MethodState, UnixMillis,
    };

    #[cfg(not(feature = "deterministic-help"))]
    use super::super::coordinator::BmadHelpCoordinatorError;
    use super::super::coordinator::PrepareBmadHelpReviewInput;
    #[cfg(feature = "deterministic-help")]
    use super::super::coordinator::{
        ApproveBmadHelpReviewInput, BmadHelpCoordinator, BmadHelpCoordinatorError,
        BmadHelpTerminalReason, CancelBmadHelpReviewInput, FinalizeBmadHelpDispatchInput,
        SubmitBmadHelpReviewInput,
    };
    #[cfg(feature = "deterministic-help")]
    use super::super::transport::BmadHelpTransport;
    use crate::bmad_foundation::{load_bmad_foundation, BmadLoadedFoundation};
    use crate::commands::create_bmad_help_run_for_test;
    #[cfg(feature = "deterministic-help")]
    use crate::commands::submit_bmad_help_review_for_test;
    use crate::state::HostState;
    #[cfg(feature = "deterministic-help")]
    use crate::wire::HostCommandData;

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

    fn transition_authority<'a>(
        state: &'a HostState,
        workspace_id: &str,
    ) -> (
        crate::state::ReadyWorkspaceCommitGuard<'a>,
        desktop_workspace::WorkspaceScopeAuthorityGuard<'a>,
    ) {
        let authority = state
            .ready_workspace_commit()
            .expect("ready workspace commit authority");
        let workspace_authority = state
            .workspace
            .authorize_scope(workspace_id, 1)
            .expect("workspace scope authority");
        (authority, workspace_authority)
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let projection = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(2_000),
                    workspace_context: Vec::new(),
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");

        let wrong = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
    fn edit_escalation_leaves_a_prepared_help_review_valid() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000041"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let review = {
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review")
        };

        // ADR-0002 independence: escalating D3 edit authority advances only
        // the governed-edit epoch; the in-review D2 Help request survives.
        let escalated = state
            .workspace
            .enable_governed_edits(&workspace_id)
            .expect("enable governed edits");
        assert_eq!(escalated.grant_epoch, 1);
        assert!(escalated.governed_edit_epoch > 1);

        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approval survives an edit-authority escalation");
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn context_read_withdrawal_invalidates_a_prepared_help_review() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000042"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let review = {
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review")
        };

        // ADR-0002: withdrawing context-read authority invalidates the D2
        // lifecycle even though the binding epoch is unchanged.
        state
            .workspace
            .advance_context_read_epoch(&workspace_id)
            .expect("advance context-read epoch");

        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let rejected = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect_err("context-read withdrawal must invalidate the review");
        assert_eq!(rejected, BmadHelpCoordinatorError::Unauthorized);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn sign_out_invalidates_a_prepared_review_between_command_boundaries() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000025"),
            &id(&workspace_id),
            1,
            BmadHelpIntent::new("Review architecture readiness").expect("bounded intent"),
            now,
        )
        .expect("retained Help run");
        state.bind_renderer("main").expect("renderer binding");

        let review = {
            let renderer = state
                .renderer_session_authority("main")
                .expect("renderer authority");
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review")
        };

        assert_eq!(state.sign_out_model().expect("broker-free sign-out"), 2);

        let renderer = state
            .renderer_session_authority("main")
            .expect("new renderer authority");
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let error = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect_err("sign-out must invalidate the retained review");
        assert_eq!(error, BmadHelpCoordinatorError::Unauthorized);
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
    #[expect(
        clippy::too_many_lines,
        reason = "the end-to-end D2 success fixture keeps the six Method transitions and one dispatch assertion together"
    )]
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
                &authority,
                &workspace_authority,
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
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
        drop(workspace_authority);
        drop(authority);

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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let error = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(2_000),
                    workspace_context: Vec::new(),
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

    // -----------------------------------------------------------------------
    // The Help round trip, held open on purpose.
    //
    // `submit` used to hold the host commit barrier, the broker's revocation
    // barrier, and the coordinator mutex across `transport.send`. Every test
    // below needs the provider call to actually be *in flight* while something
    // else happens, which a transport that answers immediately cannot express.
    // -----------------------------------------------------------------------

    /// How long a probe may take before we call the lock held rather than slow.
    /// Generous: on the fixed code every probe returns in microseconds, and the
    /// only thing a long timeout costs is how quickly a regression is reported.
    #[cfg(feature = "deterministic-help")]
    const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Ceiling on every wait that a deadlock would otherwise make infinite.
    /// These tests exist to catch parked hosts, so they must fail, not hang.
    #[cfg(feature = "deterministic-help")]
    const FLIGHT_TIMEOUT: Duration = Duration::from_secs(20);

    /// A deterministic transport that parks inside `send` until it is released.
    ///
    /// This is the seam that makes "while the request is in flight" a reachable
    /// state at all. Without it the round trip is over before any other thread
    /// can observe which locks the submitting thread is holding.
    #[cfg(feature = "deterministic-help")]
    struct GatedDeterministicTransport {
        calls: Arc<AtomicUsize>,
        entered: Sender<()>,
        // A `Receiver` is `Send` but not `Sync`, and the transport is shared.
        release: StdMutex<Receiver<()>>,
    }

    #[cfg(feature = "deterministic-help")]
    impl BmadHelpTransport for GatedDeterministicTransport {
        fn send(
            &self,
            request: AuthorizedModelRequest,
            deterministic_fixture: &str,
            now: UnixMillis,
        ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _ = self.entered.send(());
            let _ = self
                .release
                .lock()
                .expect("release gate")
                .recv_timeout(FLIGHT_TIMEOUT);
            desktop_cloud::DeterministicModelTransport.send_fixture(
                request,
                deterministic_fixture.to_owned(),
                now,
            )
        }
    }

    /// A Help lifecycle carried as far as "approved, ready to submit", with the
    /// provider call gated so the submission can be caught mid-flight.
    #[cfg(feature = "deterministic-help")]
    struct GatedSubmission {
        state: Arc<HostState>,
        workspace_id: String,
        manifest_hash: Sha256Digest,
        decision_id: ContractId,
        submitted_at: UnixMillis,
        calls: Arc<AtomicUsize>,
        in_flight: Receiver<()>,
        release: Sender<()>,
        _storage: tempfile::TempDir,
        _workspace: tempfile::TempDir,
    }

    #[cfg(feature = "deterministic-help")]
    fn gated_submission(request_id: &str) -> GatedSubmission {
        let foundation = foundation();
        let (state, storage, workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
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
        let (entered, in_flight) = channel();
        let (release, gate) = channel();
        *state.bmad_model.lock() =
            BmadHelpCoordinator::with_transport_for_test(Box::new(GatedDeterministicTransport {
                calls: Arc::clone(&calls),
                entered,
                release: StdMutex::new(gate),
            }));
        let (manifest_hash, decision_id) = {
            let renderer = state
                .renderer_session_authority("main")
                .expect("renderer authority");
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            let review = coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review");
            let approved = coordinator
                .approve(
                    &state,
                    &authority,
                    &workspace_authority,
                    ApproveBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        manifest_hash: review.context.manifest_hash,
                        approved_at: UnixMillis(now.0 + 2),
                    },
                )
                .expect("approved review");
            (approved.manifest_hash, approved.decision_id)
        };
        GatedSubmission {
            state: Arc::new(state),
            workspace_id,
            manifest_hash,
            decision_id,
            submitted_at: UnixMillis(now.0 + 3),
            calls,
            in_flight,
            release,
            _storage: storage,
            _workspace: workspace,
        }
    }

    /// Runs the real submission command on a detached worker thread.
    ///
    /// Detached rather than joined, for the reason `spawn_turn` in the chat
    /// tests gives: a joined thread would wait forever on a parked worker, so
    /// the very regressions these tests exist to catch would hang the suite
    /// instead of failing it.
    #[cfg(feature = "deterministic-help")]
    fn spawn_submission(
        fixture: &GatedSubmission,
    ) -> Receiver<Result<HostCommandData, LocalError>> {
        let (sender, receiver) = channel();
        let state = Arc::clone(&fixture.state);
        let workspace_id = id(&fixture.workspace_id);
        let manifest_hash = fixture.manifest_hash;
        let decision_id = fixture.decision_id.clone();
        let submitted_at = fixture.submitted_at;
        std::thread::spawn(move || {
            let renderer = state
                .renderer_session_authority("main")
                .expect("renderer authority");
            let outcome = submit_bmad_help_review_for_test(
                &state,
                &renderer,
                workspace_id,
                1,
                manifest_hash,
                decision_id,
                submitted_at,
            );
            drop(renderer);
            let _ = sender.send(outcome);
        });
        receiver
    }

    #[cfg(feature = "deterministic-help")]
    fn awaited(
        receiver: &Receiver<Result<HostCommandData, LocalError>>,
    ) -> Result<HostCommandData, LocalError> {
        receiver
            .recv_timeout(FLIGHT_TIMEOUT)
            .expect("the Help submission never returned: the host is deadlocked")
    }

    /// No host authority may survive the provider round trip.
    ///
    /// The submission used to hold three at once across `transport.send`:
    ///
    /// * `ReadyWorkspaceCommitGuard` — the `workspace_commits` mutex plus a
    ///   `boot_mode` read guard. `revoke_workspace` and `enter_recovery` both
    ///   take it, so a revocation could not even begin while a Help request was
    ///   in flight.
    /// * `WorkspaceScopeAuthorityGuard` — the broker's task-fair revocation
    ///   barrier, which every broker method re-acquires.
    /// * The `bmad_model` coordinator mutex, which every other Help command
    ///   needs.
    ///
    /// The probe below takes all three, in the lane's own lock order, while the
    /// request is in flight. It cannot pass vacuously: it is released only
    /// after `in_flight` has been signalled from inside `send`, so the round
    /// trip is provably still open, and `calls` is asserted so a fixture that
    /// silently stopped dispatching would fail rather than pass.
    #[cfg(feature = "deterministic-help")]
    #[test]
    fn help_submit_holds_no_host_authority_across_the_provider_round_trip() {
        let fixture = gated_submission("request_01J00000000000000000000060");
        let submission = spawn_submission(&fixture);

        fixture
            .in_flight
            .recv_timeout(FLIGHT_TIMEOUT)
            .expect("the submission reached the provider");
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);

        let (probed, probe_done) = channel::<()>();
        let probing = Arc::clone(&fixture.state);
        let probe_workspace = fixture.workspace_id.clone();
        std::thread::spawn(move || {
            // The lane's own order: host commit barrier, then broker revocation
            // barrier, then the coordinator. Acquiring them in this order can
            // never deadlock against the lane itself, so a probe that does not
            // return means the lane is holding one of them.
            let authority = probing
                .ready_workspace_commit()
                .expect("host commit authority");
            let scope = probing
                .workspace
                .authorize_scope(&probe_workspace, 1)
                .expect("workspace scope authority");
            let coordinator = probing.bmad_model.lock();
            drop(coordinator);
            drop(scope);
            drop(authority);
            let _ = probed.send(());
        });
        let all_free = probe_done.recv_timeout(PROBE_TIMEOUT).is_ok();

        // Released before the assertion so a failing probe reports a failure
        // rather than wedging the suite behind a parked worker.
        let _ = fixture.release.send(());
        let outcome = awaited(&submission);

        assert!(
            all_free,
            "a host authority was still held during the provider round trip",
        );
        assert!(matches!(
            outcome,
            Ok(HostCommandData::BmadHelpRunCompleted(_))
        ));
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    }

    /// The broker's revocation barrier specifically.
    ///
    /// A read guard does not block another reader, so the probe above cannot
    /// tell a released `WorkspaceScopeAuthorityGuard` from a retained one. Only
    /// a broker *writer* can, and `advance_context_read_epoch` is one — it is
    /// also exactly what a model sign-out performs, so this is the real
    /// mid-flight event, not a stand-in for it.
    ///
    /// Non-vacuous in both directions: the withdrawal must complete inside the
    /// probe window (it cannot if the barrier is held), *and* the submission
    /// must then be refused rather than recorded against the withdrawn grant.
    #[cfg(feature = "deterministic-help")]
    #[test]
    fn a_context_read_withdrawal_can_land_while_a_help_request_is_in_flight() {
        let fixture = gated_submission("request_01J00000000000000000000061");
        let submission = spawn_submission(&fixture);

        fixture
            .in_flight
            .recv_timeout(FLIGHT_TIMEOUT)
            .expect("the submission reached the provider");

        let (withdrawn, withdrawal_done) = channel::<()>();
        let withdrawing = Arc::clone(&fixture.state);
        let withdraw_workspace = fixture.workspace_id.clone();
        std::thread::spawn(move || {
            withdrawing
                .workspace
                .advance_context_read_epoch(&withdraw_workspace)
                .expect("context-read withdrawal");
            let _ = withdrawn.send(());
        });
        let landed = withdrawal_done.recv_timeout(PROBE_TIMEOUT).is_ok();

        let _ = fixture.release.send(());
        let outcome = awaited(&submission);

        assert!(
            landed,
            "the broker's revocation barrier was held across the provider round trip",
        );
        let refusal = outcome.expect_err("a withdrawn grant must not complete the run");
        assert_eq!(refusal.code, desktop_runtime::LocalErrorCode::Unauthorized);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    }

    /// The authority re-check inside `finalize_submit`.
    ///
    /// Driven without threads: `begin_submit` and `finalize_submit` are separate
    /// calls, so everything between them *is* the in-flight window, and the
    /// withdrawal below lands there deterministically.
    ///
    /// Aimed squarely at `validate_prepared_scope`. `advance_context_read_epoch`
    /// leaves the grant epoch alone, so `authorize_scope` still succeeds and the
    /// command-level re-authorization cannot be what refuses this — only the
    /// coordinator's own comparison of `context_read_epoch` against the epoch
    /// the review was prepared under can. Delete that check and the run
    /// completes.
    #[cfg(feature = "deterministic-help")]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the mid-flight withdrawal fixture keeps prepare, approve, dispatch, withdrawal, and finalize in one visible order"
    )]
    fn finalize_re_checks_the_authority_the_dispatch_began_under() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        let creation = create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000062"),
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

        let dispatch = {
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            let review = coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review");
            let approved = coordinator
                .approve(
                    &state,
                    &authority,
                    &workspace_authority,
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
                .begin_submit(
                    &state,
                    &authority,
                    &workspace_authority,
                    SubmitBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        manifest_hash: approved.manifest_hash,
                        decision_id: approved.decision_id,
                        submitted_at: UnixMillis(now.0 + 3),
                    },
                )
                .expect("began the dispatch")
        };
        let dispatch_id = dispatch.dispatch_id;

        // The round trip, with nothing held — the same shape the command uses.
        let (dispatched, raw_output) = dispatch.send().expect("deterministic round trip");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // ADR-0002: withdrawing context-read authority invalidates the D2
        // lifecycle even though the binding epoch is unchanged. This is the
        // event that used to be unobservable, because the lane held the
        // barrier — and the commit barrier — for the whole call.
        state
            .workspace
            .advance_context_read_epoch(&workspace_id)
            .expect("advance context-read epoch");

        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let refused = coordinator
            .finalize_submit(
                &state,
                &authority,
                &workspace_authority,
                FinalizeBmadHelpDispatchInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    dispatch_id,
                    dispatched,
                    raw_output,
                },
            )
            .expect_err("a mid-flight context-read withdrawal must refuse the result");

        assert_eq!(refused, BmadHelpCoordinatorError::Unauthorized);
        // The decision is spent and the context has already left the machine, so
        // the refusal is terminal-consumed, never a restore to `Approved`.
        assert_eq!(
            coordinator.terminal_reason_for_test(),
            Some(BmadHelpTerminalReason::ConsentConsumed),
        );
        assert!(!coordinator.is_completed_for_test());

        // Nothing was written: the session is still the v5 the advance receipt
        // left it at, never the v6 a completed run reaches.
        let session = state
            .method_store(authority.authority())
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(session.state(), MethodState::Advancing);
        assert_eq!(session.version(), 5);
    }

    /// The dispatch identity, which only exists because of the split.
    ///
    /// Releasing the coordinator across the round trip admits a shape that the
    /// one-shot path could not have: a finalize arriving for a dispatch that no
    /// longer occupies the slot. `dispatch_id` is what refuses it, and a stale
    /// finalize must also leave the slot exactly as it found it — so the correct
    /// finalize below must still succeed afterwards. Drop the id comparison and
    /// the first call completes the run, which makes the second fail.
    #[cfg(feature = "deterministic-help")]
    #[test]
    fn a_finalize_quoting_the_wrong_dispatch_is_refused_without_disturbing_the_slot() {
        let foundation = foundation();
        let (state, _storage, _workspace, workspace_id) = ready_workspace_state();
        let now = crate::state::now();
        create_bmad_help_run_for_test(
            &state,
            &foundation,
            &id("request_01J00000000000000000000068"),
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
        let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
        let mut coordinator = state.bmad_model.lock();
        let review = coordinator
            .prepare(
                &state,
                &authority,
                &workspace_authority,
                &foundation,
                PrepareBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    created_at: UnixMillis(now.0 + 1),
                    workspace_context: Vec::new(),
                },
            )
            .expect("prepared review");
        let approved = coordinator
            .approve(
                &state,
                &authority,
                &workspace_authority,
                ApproveBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: review.context.manifest_hash,
                    approved_at: UnixMillis(now.0 + 2),
                },
            )
            .expect("approved review");
        let dispatch = coordinator
            .begin_submit(
                &state,
                &authority,
                &workspace_authority,
                SubmitBmadHelpReviewInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    manifest_hash: approved.manifest_hash,
                    decision_id: approved.decision_id,
                    submitted_at: UnixMillis(now.0 + 3),
                },
            )
            .expect("began the dispatch");
        let dispatch_id = dispatch.dispatch_id;
        let (dispatched, raw_output) = dispatch.send().expect("deterministic round trip");

        let superseded = coordinator
            .finalize_submit(
                &state,
                &authority,
                &workspace_authority,
                FinalizeBmadHelpDispatchInput {
                    renderer_session: &renderer,
                    workspace_id: id(&workspace_id),
                    workspace_grant_epoch: 1,
                    dispatch_id: dispatch_id.wrapping_add(1),
                    dispatched,
                    raw_output,
                },
            )
            .expect_err("a finalize naming another dispatch must be refused");
        assert_eq!(superseded, BmadHelpCoordinatorError::ConsentAlreadyConsumed);
        // Left exactly as it was found: still `Advancing`, so no terminal reason
        // was written and the run did not complete. These two are what the id
        // comparison buys — without it the stale call completes the run, and
        // both flip.
        assert_eq!(coordinator.terminal_reason_for_test(), None);
        assert!(!coordinator.is_completed_for_test());
        assert!(!coordinator.has_active_review());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// Which terminal transition each reachable failure lands in.
    ///
    /// The acceptance criterion for the begin/finalize split is that no
    /// transition changed state or reason, so the table asserts the landing
    /// state, not just the error code — several distinct transitions share one
    /// `BmadHelpCoordinatorError`, and the success case is not an error at all.
    #[cfg(feature = "deterministic-help")]
    #[derive(Clone, Copy)]
    enum TerminalCase {
        /// Refused before the decision is consumed: submitted outside the
        /// approved window. Lands `Terminal(ConsentExpired)`, dispatches nothing.
        ExpiredBeforeDispatch,
        /// The transport itself fails. This is the transition that moved out of
        /// the coordinator and into `abort_dispatch`; it must still land the
        /// identical `Terminal(ConsentConsumed)`.
        TransportFailure,
        /// The reply parses but is not the sealed canonical schema.
        SchemaRejected,
        /// The reply is well-formed but its receipt does not verify.
        ReceiptRejected,
        /// The happy path: `Completed`, at v6.
        Completed,
    }

    #[cfg(feature = "deterministic-help")]
    #[expect(
        clippy::too_many_lines,
        reason = "the transition table drives one full lifecycle per case and asserts the landing state of each"
    )]
    fn assert_terminal_transition(case: TerminalCase, request_id: &str) {
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
        let transport: Box<dyn BmadHelpTransport> = match case {
            TerminalCase::TransportFailure => Box::new(FailingDeterministicTransport {
                calls: Arc::clone(&calls),
                mode: FailureMode::Transport,
            }),
            TerminalCase::SchemaRejected => Box::new(FailingDeterministicTransport {
                calls: Arc::clone(&calls),
                mode: FailureMode::InvalidSchema,
            }),
            TerminalCase::ReceiptRejected => Box::new(FailingDeterministicTransport {
                calls: Arc::clone(&calls),
                mode: FailureMode::InvalidReceipt,
            }),
            TerminalCase::ExpiredBeforeDispatch | TerminalCase::Completed => {
                Box::new(CountingDeterministicTransport(Arc::clone(&calls)))
            }
        };
        *state.bmad_model.lock() = BmadHelpCoordinator::with_transport_for_test(transport);
        let renderer = state
            .renderer_session_authority("main")
            .expect("renderer authority");
        let (manifest_hash, decision_id, expires_at) = {
            let (authority, workspace_authority) = transition_authority(&state, &workspace_id);
            let mut coordinator = state.bmad_model.lock();
            let review = coordinator
                .prepare(
                    &state,
                    &authority,
                    &workspace_authority,
                    &foundation,
                    PrepareBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        created_at: UnixMillis(now.0 + 1),
                        workspace_context: Vec::new(),
                    },
                )
                .expect("prepared review");
            let approved = coordinator
                .approve(
                    &state,
                    &authority,
                    &workspace_authority,
                    ApproveBmadHelpReviewInput {
                        renderer_session: &renderer,
                        workspace_id: id(&workspace_id),
                        workspace_grant_epoch: 1,
                        manifest_hash: review.context.manifest_hash,
                        approved_at: UnixMillis(now.0 + 2),
                    },
                )
                .expect("approved review");
            (
                approved.manifest_hash,
                approved.decision_id,
                approved.expires_at,
            )
        };
        let submitted_at = match case {
            TerminalCase::ExpiredBeforeDispatch => UnixMillis(expires_at.0 + 1),
            _ => UnixMillis(now.0 + 3),
        };

        // Driven through the production command, so the table is asserting the
        // real begin -> unlocked send -> finalize sequencing and not a
        // test-only shortcut around it.
        let outcome = submit_bmad_help_review_for_test(
            &state,
            &renderer,
            id(&workspace_id),
            1,
            manifest_hash,
            decision_id,
            submitted_at,
        );

        let coordinator = state.bmad_model.lock();
        let (expected_reason, expected_calls, expected_state, expected_version) = match case {
            TerminalCase::ExpiredBeforeDispatch => (
                Some(BmadHelpTerminalReason::ConsentExpired),
                0,
                MethodState::ContextReviewRequired,
                3,
            ),
            TerminalCase::TransportFailure
            | TerminalCase::SchemaRejected
            | TerminalCase::ReceiptRejected => (
                Some(BmadHelpTerminalReason::ConsentConsumed),
                1,
                MethodState::Advancing,
                5,
            ),
            TerminalCase::Completed => (None, 1, MethodState::Completed, 6),
        };
        assert_eq!(coordinator.terminal_reason_for_test(), expected_reason);
        assert_eq!(
            coordinator.is_completed_for_test(),
            expected_reason.is_none()
        );
        assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
        match case {
            TerminalCase::Completed => assert!(matches!(
                outcome,
                Ok(HostCommandData::BmadHelpRunCompleted(_))
            )),
            _ => {
                outcome.expect_err("every non-completing case must be refused");
            }
        }
        drop(coordinator);

        let authority = state.ready_authority().expect("ready authority");
        let session = state
            .method_store(&authority)
            .expect("Method store")
            .load_method_session(&creation.scope, &creation.session_id)
            .expect("load Method")
            .expect("retained Method");
        assert_eq!(session.state(), expected_state);
        assert_eq!(session.version(), expected_version);
    }

    #[cfg(feature = "deterministic-help")]
    #[test]
    fn every_reachable_terminal_transition_survives_the_dispatch_split() {
        assert_terminal_transition(
            TerminalCase::ExpiredBeforeDispatch,
            "request_01J00000000000000000000063",
        );
        assert_terminal_transition(
            TerminalCase::TransportFailure,
            "request_01J00000000000000000000064",
        );
        assert_terminal_transition(
            TerminalCase::SchemaRejected,
            "request_01J00000000000000000000065",
        );
        assert_terminal_transition(
            TerminalCase::ReceiptRejected,
            "request_01J00000000000000000000066",
        );
        assert_terminal_transition(
            TerminalCase::Completed,
            "request_01J00000000000000000000067",
        );
    }
}
