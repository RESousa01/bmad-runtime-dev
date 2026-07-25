#![allow(clippy::expect_used)]

use desktop_ipc::{
    project_bmad_library, project_bmad_library_with_runtime_menu_bindings,
    BmadProjectionAvailability, BmadProjectionError, BmadRuntimeMenuCapabilityBinding,
    CommandEnvelopeValidator, IpcValidationContext, IpcValidationError,
    MAX_BMAD_LIBRARY_PROJECTION_BYTES,
};
use desktop_runtime::{
    sha256_bytes, BmadAgentRoster, BmadCatalog, BmadCatalogBuilder, BmadEntrypointKind,
    BmadHelpCatalogSource, BmadLibraryProjectionScope, BmadLoadedPackage, BmadLoadedSkill,
    BmadProjectionInvalidationScope, ContractId, LocalCommand, ProjectionEvent,
    ProjectionEventKind, Sha256Digest, UnixMillis,
};
use serde_json::{json, Value};

const PACKAGE_VERSION_ID: &str =
    "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86";
const REVIEWED_METHOD_MENU_BINDINGS: [BmadRuntimeMenuCapabilityBinding<'static>; 25] = [
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "BP",
        capability_id: "bmm:bmad-brainstorming",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "MR",
        capability_id: "bmm:bmad-market-research",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "DR",
        capability_id: "bmm:bmad-domain-research",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "TR",
        capability_id: "bmm:bmad-technical-research",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "CB",
        capability_id: "bmm:bmad-product-brief",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "WB",
        capability_id: "bmm:bmad-prfaq",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-analyst",
        menu_code: "DP",
        capability_id: "bmm:bmad-document-project",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-tech-writer",
        menu_code: "DP",
        capability_id: "bmm:bmad-document-project",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-tech-writer",
        menu_code: "WD",
        capability_id: "bmm:tech-writer-write-document",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-tech-writer",
        menu_code: "MG",
        capability_id: "bmm:tech-writer-mermaid-gen",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-tech-writer",
        menu_code: "VD",
        capability_id: "bmm:tech-writer-validate-doc",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-tech-writer",
        menu_code: "EC",
        capability_id: "bmm:tech-writer-explain-concept",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-pm",
        menu_code: "PRD",
        capability_id: "bmm:bmad-prd",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-pm",
        menu_code: "CE",
        capability_id: "bmm:bmad-create-epics-and-stories",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-pm",
        menu_code: "IR",
        capability_id: "bmm:bmad-check-implementation-readiness",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-pm",
        menu_code: "CC",
        capability_id: "bmm:bmad-correct-course",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-ux-designer",
        menu_code: "CU",
        capability_id: "bmm:bmad-ux",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-architect",
        menu_code: "IR",
        capability_id: "bmm:bmad-check-implementation-readiness",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "DS",
        capability_id: "bmm:bmad-dev-story",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "QD",
        capability_id: "bmm:bmad-quick-dev",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "QA",
        capability_id: "bmm:bmad-qa-generate-e2e-tests",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "CR",
        capability_id: "bmm:bmad-code-review",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "SP",
        capability_id: "bmm:bmad-sprint-planning",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "CS",
        capability_id: "bmm:bmad-create-story",
    },
    BmadRuntimeMenuCapabilityBinding {
        agent_code: "bmad-agent-dev",
        menu_code: "ER",
        capability_id: "bmm:bmad-retrospective",
    },
];

fn package() -> BmadLoadedPackage {
    BmadLoadedPackage {
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: ContractId::new(PACKAGE_VERSION_ID).expect("package version ID"),
        descriptor_hash: sha256_bytes(b"descriptor"),
        observed_inventory_hash: sha256_bytes(b"inventory"),
        skills: vec![
            BmadLoadedSkill {
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
            },
            BmadLoadedSkill {
                module_code: "core".to_owned(),
                skill_name: "bmad-help".to_owned(),
                display_name: "BMad Help".to_owned(),
                description: "Provide source-grounded guidance.".to_owned(),
                entrypoint_kind: BmadEntrypointKind::Direct,
                actions: Vec::new(),
                distribution_profile: "sapphirus_package".to_owned(),
                install_profile: "SapphirusManagedV1".to_owned(),
                validation_profile: "MethodOfficialSkillV6".to_owned(),
                execution_profile_hash: sha256_bytes(b"help profile"),
                capability_enabled: false,
                structurally_eligible: true,
            },
        ],
    }
}

fn sealed_catalog() -> (BmadLoadedPackage, BmadCatalog) {
    let package = package();
    let graph: Value = serde_json::from_slice(include_bytes!(
        "../../../packages/bmad-foundation/normalized/bmad-help-action-graph.json"
    ))
    .expect("sealed help graph");
    let sources = graph["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .map(|source| {
            let rows = serde_json::from_value::<Vec<Vec<String>>>(source["rows"].clone())
                .expect("normalized rows");
            BmadHelpCatalogSource::from_rows(
                source["moduleCode"].as_str().expect("module code"),
                &rows,
            )
            .expect("catalog source")
        })
        .collect::<Vec<_>>();
    let graph_hash =
        Sha256Digest::parse(graph["graphHash"].as_str().expect("graph hash")).expect("digest");
    let catalog =
        BmadCatalogBuilder::build_bound(&package, &sources, graph_hash).expect("sealed catalog");
    (package, catalog)
}

fn sealed_roster(catalog: &BmadCatalog, package: &BmadLoadedPackage) -> BmadAgentRoster {
    let bytes =
        include_bytes!("../../../packages/bmad-foundation/normalized/bmm-agent-roster.json");
    BmadAgentRoster::load_normalized(
        bytes,
        catalog,
        &package.package_version_id,
        sha256_bytes(bytes),
    )
    .expect("sealed roster")
}

fn context() -> IpcValidationContext {
    IpcValidationContext {
        expected_window_label: "main".to_owned(),
        renderer_session_id: ContractId::new("renderer_01J00000000000000000000000")
            .expect("renderer session"),
        installation_id: ContractId::new("install_01J00000000000000000000000")
            .expect("installation"),
        now: UnixMillis(10_000),
        allowed_commands: vec!["bmad.library.snapshot".to_owned()],
    }
}

fn envelope(payload: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "desktop-ipc-command.v1",
        "requestId": "request_01J00000000000000000000000",
        "command": "bmad.library.snapshot",
        "windowLabel": "main",
        "rendererSessionId": "renderer_01J00000000000000000000000",
        "installationId": "install_01J00000000000000000000000",
        "issuedAt": 10_000,
        "payload": payload,
    }))
    .expect("envelope")
}

#[test]
fn library_snapshot_payload_is_closed_bound_and_read_only() {
    let parsed = CommandEnvelopeValidator::parse(
        &envelope(&json!({"scope": "installed_method", "cursor": null})),
        &context(),
    )
    .expect("valid BMAD library command");
    assert_eq!(parsed.command().name(), "bmad.library.snapshot");
    assert!(!parsed.command().is_mutating());
    assert!(matches!(
        parsed.command(),
        LocalCommand::BmadLibrarySnapshot {
            scope: BmadLibraryProjectionScope::InstalledMethod,
            cursor: None,
        }
    ));

    for forbidden in [
        "ownerId",
        "projectId",
        "delivery",
        "workspaceId",
        "packageBytes",
        "sourcePath",
        "availability",
        "confidence",
        "actionAuthority",
    ] {
        let mut payload = json!({"scope": "installed_method", "cursor": null});
        payload[forbidden] = json!("renderer supplied");
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(&payload), &context()),
            Err(IpcValidationError::InvalidPayload)
        ));
    }
}

#[test]
fn snapshot_payload_rejects_unknown_scope_bad_cursor_and_unadvertised_capability() {
    for payload in [
        json!({"scope": "all_packages", "cursor": null}),
        json!({"scope": "installed_method", "cursor": "x".repeat(257)}),
        json!({"scope": "installed_method", "cursor": "bad\u{0000}cursor"}),
        json!({"scope": "installed_method", "cursor": "bad\u{202e}cursor"}),
    ] {
        assert!(matches!(
            CommandEnvelopeValidator::parse(&envelope(&payload), &context()),
            Err(IpcValidationError::InvalidPayload)
        ));
    }

    let mut unavailable = context();
    unavailable.allowed_commands.clear();
    assert!(matches!(
        CommandEnvelopeValidator::parse(
            &envelope(&json!({"scope": "installed_method", "cursor": null})),
            &unavailable,
        ),
        Err(IpcValidationError::CapabilityUnavailable)
    ));

    let mut wrong_window = envelope(&json!({"scope": "installed_method", "cursor": null}));
    let mut value: Value = serde_json::from_slice(&wrong_window).expect("envelope value");
    value["windowLabel"] = json!("attacker");
    wrong_window = serde_json::to_vec(&value).expect("wrong window envelope");
    assert!(matches!(
        CommandEnvelopeValidator::parse(&wrong_window, &context()),
        Err(IpcValidationError::BindingMismatch)
    ));
}

#[test]
fn sealed_library_projection_is_bounded_separate_and_disclosure_safe() {
    let (package, catalog) = sealed_catalog();
    let roster = sealed_roster(&catalog, &package);
    let projection = project_bmad_library(
        &package,
        &catalog,
        &roster,
        BmadLibraryProjectionScope::InstalledMethod,
        None,
    )
    .expect("safe projection");

    assert_eq!(projection.installed_skills.len(), 2);
    assert_eq!(projection.help_actions.len(), 2);
    assert_eq!(projection.method_agents.len(), 6);
    assert_eq!(
        projection
            .method_agents
            .iter()
            .map(|agent| agent.menus.len())
            .sum::<usize>(),
        26
    );
    assert!(projection.help_actions.iter().any(|action| {
        action.module_code == "bmm"
            && action.skill_name == "bmad-architecture"
            && action.action.as_deref() == Some("create")
    }));
    let winston = projection
        .method_agents
        .iter()
        .find(|agent| agent.agent_code == "bmad-agent-architect")
        .expect("Winston");
    assert_eq!(
        (&winston.name, &winston.title),
        (&"Winston".to_owned(), &"System Architect".to_owned())
    );
    let paige = projection
        .method_agents
        .iter()
        .find(|agent| agent.agent_code == "bmad-agent-tech-writer")
        .expect("Paige");
    assert_eq!(
        paige
            .menus
            .iter()
            .filter(|menu| menu.target_kind.as_str() == "prompt_reference")
            .count(),
        4
    );

    let bytes = serde_json::to_vec(&projection).expect("projection JSON");
    assert!(bytes.len() <= MAX_BMAD_LIBRARY_PROJECTION_BYTES);
    let safe = String::from_utf8(bytes).expect("UTF-8 projection");
    for forbidden in [
        "sha256:",
        "packageVersionId",
        "sourceLocalMemberLabel",
        "sourceMemberHash",
        "sourceOrdinal",
        "outputLocations",
        "rawSourceRow",
        "profileHash",
        "rosterHash",
        "authorityRef",
        "casRef",
        "accessToken",
        ".md",
        "C:\\\\",
    ] {
        assert!(!safe.contains(forbidden), "leaked {forbidden}");
    }
}

fn assert_projected_menu_capability(
    projection: &desktop_ipc::BmadLibrarySnapshotProjection,
    agent_code: &str,
    menu_code: &str,
    expected: Option<&str>,
) {
    let menu = projection
        .method_agents
        .iter()
        .find(|agent| agent.agent_code == agent_code)
        .and_then(|agent| agent.menus.iter().find(|menu| menu.code == menu_code))
        .expect("reviewed menu projection");
    assert_eq!(menu.capability_id.as_deref(), expected);
    assert_eq!(
        serde_json::to_value(menu).expect("menu projection JSON")["capabilityId"],
        expected.map_or(Value::Null, |value| Value::String(value.to_owned())),
        "the validated capability identity must cross IPC with its menu path"
    );
}

#[test]
fn runtime_menu_bindings_project_all_reviewed_method_menus_as_available_skill_targets() {
    let (package, catalog) = sealed_catalog();
    let roster = sealed_roster(&catalog, &package);
    let projection = project_bmad_library_with_runtime_menu_bindings(
        &package,
        &catalog,
        &roster,
        BmadLibraryProjectionScope::InstalledMethod,
        None,
        &[("core", "bmad-help")],
        &REVIEWED_METHOD_MENU_BINDINGS,
        Vec::new(),
        Vec::new(),
    )
    .expect("runtime capability overlay");

    let help = projection
        .installed_skills
        .iter()
        .find(|skill| skill.skill_name == "bmad-help")
        .expect("Help skill");
    assert_eq!(help.availability, BmadProjectionAvailability::Available);

    assert_eq!(
        projection
            .method_agents
            .iter()
            .flat_map(|agent| &agent.menus)
            .count(),
        26
    );
    assert!(projection
        .method_agents
        .iter()
        .all(|agent| agent.availability == BmadProjectionAvailability::Available));
    assert_eq!(
        projection
            .method_agents
            .iter()
            .flat_map(|agent| &agent.menus)
            .filter(|menu| menu.availability == BmadProjectionAvailability::Available)
            .count(),
        25
    );
    assert!(projection
        .method_agents
        .iter()
        .flat_map(|agent| &agent.menus)
        .filter(|menu| menu.availability == BmadProjectionAvailability::Available)
        .all(|menu| {
            menu.availability == BmadProjectionAvailability::Available
                && menu.target_kind.as_str() == "skill_target"
        }));

    for (menu_code, capability_id) in [
        ("MR", "bmm:bmad-market-research"),
        ("DR", "bmm:bmad-domain-research"),
    ] {
        assert_projected_menu_capability(
            &projection,
            "bmad-agent-analyst",
            menu_code,
            Some(capability_id),
        );
    }

    let architecture = projection
        .method_agents
        .iter()
        .find(|agent| agent.agent_code == "bmad-agent-architect")
        .and_then(|agent| agent.menus.iter().find(|menu| menu.code == "CA"))
        .expect("deferred architecture menu");
    assert_eq!(
        architecture.availability,
        BmadProjectionAvailability::CapabilityDisabled
    );
    assert_projected_menu_capability(&projection, "bmad-agent-architect", "CA", None);

    let paige = projection
        .method_agents
        .iter()
        .find(|agent| agent.agent_code == "bmad-agent-tech-writer")
        .expect("Paige");
    for (menu_code, display_label) in [
        ("WD", "Write Document"),
        ("MG", "Generate Mermaid"),
        ("VD", "Validate Document"),
        ("EC", "Explain Concept"),
    ] {
        let menu = paige
            .menus
            .iter()
            .find(|menu| menu.code == menu_code)
            .expect("reviewed Paige menu path");
        assert_eq!(menu.availability, BmadProjectionAvailability::Available);
        assert_eq!(menu.target_kind.as_str(), "skill_target");
        assert_eq!(menu.display_label, display_label);
    }
}

#[test]
fn runtime_menu_bindings_reject_unknown_duplicate_and_mismatched_bindings() {
    let (package, catalog) = sealed_catalog();
    let roster = sealed_roster(&catalog, &package);
    assert_eq!(
        project_bmad_library_with_runtime_menu_bindings(
            &package,
            &catalog,
            &roster,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
            &[],
            &[BmadRuntimeMenuCapabilityBinding {
                agent_code: "bmad-agent-analyst",
                menu_code: "not-reviewed",
                capability_id: "bmm:bmad-brainstorming",
            }],
            Vec::new(),
            Vec::new(),
        )
        .expect_err("unknown runtime capability must fail closed"),
        BmadProjectionError::Unavailable
    );
    assert_eq!(
        project_bmad_library_with_runtime_menu_bindings(
            &package,
            &catalog,
            &roster,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
            &[],
            &[
                REVIEWED_METHOD_MENU_BINDINGS[0],
                REVIEWED_METHOD_MENU_BINDINGS[0],
            ],
            Vec::new(),
            Vec::new(),
        )
        .expect_err("duplicate runtime menu path must fail closed"),
        BmadProjectionError::Unavailable
    );

    let mut swapped_capabilities = REVIEWED_METHOD_MENU_BINDINGS;
    let first_capability = swapped_capabilities[1].capability_id;
    swapped_capabilities[1].capability_id = swapped_capabilities[2].capability_id;
    swapped_capabilities[2].capability_id = first_capability;
    assert_eq!(
        project_bmad_library_with_runtime_menu_bindings(
            &package,
            &catalog,
            &roster,
            BmadLibraryProjectionScope::InstalledMethod,
            None,
            &[],
            &swapped_capabilities,
            Vec::new(),
            Vec::new(),
        )
        .expect_err("known capability IDs bound to the wrong paths must fail closed"),
        BmadProjectionError::Unavailable
    );
}

#[test]
fn non_null_cursor_is_a_gap_and_invalidation_event_has_the_exact_public_name() {
    let (package, catalog) = sealed_catalog();
    let roster = sealed_roster(&catalog, &package);
    assert_eq!(
        project_bmad_library(
            &package,
            &catalog,
            &roster,
            BmadLibraryProjectionScope::InstalledMethod,
            Some("stale-cursor"),
        )
        .expect_err("v1 has no continuation state"),
        BmadProjectionError::Gap
    );

    let event = ProjectionEvent {
        sequence: 7,
        occurred_at: UnixMillis(20_000),
        event: ProjectionEventKind::BmadProjectionChanged {
            scope: BmadProjectionInvalidationScope::Library,
        },
    };
    let value = serde_json::to_value(event).expect("projection event");
    assert_eq!(value["event"]["type"], "bmad.projection_changed");
    assert_eq!(value["event"]["projection"]["scope"], "library");
    assert!(value["event"]["projection"]
        .as_object()
        .is_some_and(|fields| fields.len() == 1));
}
