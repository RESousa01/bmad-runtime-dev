#![allow(clippy::expect_used)]

mod prepare {
    use desktop_runtime::{
        BmadHelpIntent, ContractId, MethodSessionRepository, MethodState, UnixMillis,
    };

    #[cfg(not(feature = "deterministic-help"))]
    use super::super::coordinator::BmadHelpCoordinatorError;
    use super::super::coordinator::PrepareBmadHelpReviewInput;
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
