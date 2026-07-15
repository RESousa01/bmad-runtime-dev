#![allow(clippy::expect_used)]

use std::sync::Mutex;

use desktop_runtime::{
    sha256_bytes, BmadCatalogBuilder, BmadEntrypointKind, BmadHelpCatalogSource,
    BmadHelpConfidence, BmadHelpIntent, BmadKernelErrorCode, BmadLoadedPackage, BmadLoadedSkill,
    ContractId, CreateInertBmadHelpSession, DesktopLocalIdentity, InertBmadHelpSessionCoordinator,
    InertBmadHelpSessionError, MethodAdvanceDisposition, MethodAdvanceReceipt,
    MethodAdvanceRequest, MethodArtifactProvenance, MethodErrorCode, MethodExactBinding,
    MethodPersistenceEvent, MethodServiceError, MethodSession, MethodSessionRepository,
    MethodSessionScope, MethodState, UnixMillis,
};

#[derive(Debug, thiserror::Error)]
#[error("mock repository error")]
struct MockRepositoryError;

#[derive(Default)]
struct MockRepository {
    sessions: Mutex<Vec<MethodSession>>,
    reject_create: bool,
}

impl MockRepository {
    fn sessions(&self) -> Vec<MethodSession> {
        self.sessions.lock().expect("session lock").clone()
    }
}

impl MethodSessionRepository for MockRepository {
    type Error = MockRepositoryError;

    fn create_method_session(&self, session: &MethodSession) -> Result<(), Self::Error> {
        if self.reject_create {
            return Err(MockRepositoryError);
        }
        let mut sessions = self.sessions.lock().map_err(|_| MockRepositoryError)?;
        if sessions
            .iter()
            .any(|existing| existing.session_id() == session.session_id())
        {
            return Err(MockRepositoryError);
        }
        sessions.push(session.clone());
        Ok(())
    }

    fn load_method_session(
        &self,
        scope: &MethodSessionScope,
        session_id: &ContractId,
    ) -> Result<Option<MethodSession>, Self::Error> {
        Ok(self
            .sessions
            .lock()
            .map_err(|_| MockRepositoryError)?
            .iter()
            .find(|session| session.scope() == *scope && session.session_id() == *session_id)
            .cloned())
    }

    fn begin_method_advance(
        &self,
        _scope: &MethodSessionScope,
        _session_id: &ContractId,
        _observed_binding: &MethodExactBinding,
        _request: MethodAdvanceRequest,
    ) -> Result<MethodAdvanceReceipt, Self::Error> {
        Err(MockRepositoryError)
    }

    fn persist_method_transition(
        &self,
        _session: &MethodSession,
        _expected_previous_version: u64,
        _event: MethodPersistenceEvent,
    ) -> Result<(), Self::Error> {
        Err(MockRepositoryError)
    }

    fn validate_method_artifact_refs(
        &self,
        _provenance: &MethodArtifactProvenance,
        _binding: &MethodExactBinding,
        _disposition: MethodAdvanceDisposition,
        _refs: &[String],
    ) -> Result<(), Self::Error> {
        Err(MockRepositoryError)
    }
}

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn package() -> BmadLoadedPackage {
    BmadLoadedPackage {
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: id("pkgver_01J00000000000000000000000"),
        descriptor_hash: sha256_bytes(b"descriptor"),
        observed_inventory_hash: sha256_bytes(b"inventory"),
        skills: vec![BmadLoadedSkill {
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            display_name: "Create Architecture".to_owned(),
            description: "Create a bounded architecture spine.".to_owned(),
            entrypoint_kind: BmadEntrypointKind::StepJit,
            actions: vec!["create".to_owned()],
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
            execution_profile_hash: sha256_bytes(b"architecture profile"),
            capability_enabled: false,
            structurally_eligible: false,
        }],
    }
}

fn catalog() -> desktop_runtime::BmadCatalog {
    let source = BmadHelpCatalogSource::new(
        "bmm",
        concat!(
            "module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs\n",
            "BMad Method,bmad-architecture,Create Architecture,CA,Create a bounded architecture spine.,create,,3-solutioning,,,true,planning_artifacts,architecture\n"
        ),
    )
    .expect("catalog source");
    BmadCatalogBuilder::build(&package(), &[source]).expect("catalog")
}

fn create_input(intent: &str) -> CreateInertBmadHelpSession {
    CreateInertBmadHelpSession {
        session_id: id("session_01J00000000000000000000000"),
        project_id: id("project_01J00000000000000000000000"),
        run_id: id("run_01J00000000000000000000000"),
        local_identity: DesktopLocalIdentity::new(
            id("installation_01J00000000000000000000000"),
            id("authority_01J00000000000000000000000"),
            id("owner_scope_01J00000000000000000000000"),
            id("store_01J00000000000000000000000"),
            1,
        )
        .expect("local identity"),
        created_at: UnixMillis(1_000),
        intent: BmadHelpIntent::new(intent).expect("intent"),
    }
}

#[test]
fn coordinator_persists_only_an_explicitly_unbound_non_executable_session() {
    let repository = MockRepository::default();
    let created = InertBmadHelpSessionCoordinator::create(
        &repository,
        &catalog(),
        create_input("we need an architecture spine"),
    )
    .expect("inert Help session");

    assert_eq!(created.session.state(), MethodState::Created);
    assert_eq!(created.session.version(), 1);
    assert_eq!(
        created
            .session
            .current_binding()
            .expect_err("no fabricated binding")
            .code(),
        MethodErrorCode::MethodBindingStale
    );
    assert_eq!(
        created.recommendation.confidence,
        BmadHelpConfidence::Unknown
    );
    assert!(!created.recommendation.completion_claimed);
    assert_eq!(
        created.recommendation.blocker_codes,
        ["bmad_capability_disabled"]
    );

    let persisted = repository.sessions();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0], created.session);
}

#[test]
fn coordinator_does_not_write_when_catalog_evidence_cannot_ground_the_intent() {
    let repository = MockRepository::default();
    let error = InertBmadHelpSessionCoordinator::create(
        &repository,
        &catalog(),
        create_input("completely unrelated bananas"),
    )
    .expect_err("unmatched intent");

    assert_eq!(
        error.advisor_code(),
        Some(BmadKernelErrorCode::HelpEvidenceInsufficient)
    );
    assert!(repository.sessions().is_empty());
}

#[test]
fn coordinator_rejects_a_tampered_local_identity_before_repository_persistence() {
    let repository = MockRepository::default();
    let mut input = create_input("architecture");
    let mut value = serde_json::to_value(&input.local_identity).expect("identity JSON");
    value["identityHash"] = serde_json::json!(sha256_bytes(b"tampered").to_string());
    input.local_identity = serde_json::from_value(value).expect("shape remains parseable");

    let error = InertBmadHelpSessionCoordinator::create(&repository, &catalog(), input)
        .expect_err("tampered authority");
    assert!(matches!(error, InertBmadHelpSessionError::Identity(_)));
    assert!(repository.sessions().is_empty());
}

#[test]
fn coordinator_propagates_repository_failure_without_partial_success() {
    let repository = MockRepository {
        sessions: Mutex::default(),
        reject_create: true,
    };
    let error = InertBmadHelpSessionCoordinator::create(
        &repository,
        &catalog(),
        create_input("architecture"),
    )
    .expect_err("repository failure");
    assert!(matches!(
        error,
        InertBmadHelpSessionError::Session(MethodServiceError::Repository(_))
    ));
    assert!(repository.sessions().is_empty());
}
