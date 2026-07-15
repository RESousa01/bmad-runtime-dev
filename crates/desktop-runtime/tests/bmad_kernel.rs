#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, BmadConfigGraph, BmadConfigGraphKind, BmadConfigLayer,
    BmadConfigResolver, BmadKernelErrorCode, BmadLocationClass, BmadPackageLoader, BmadSourceEntry,
    BmadSourceKind, BmadSourceSnapshot, Sha256Digest,
};
use serde_json::{json, Value};

fn foundation_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation")
        .join(relative)
}

fn source_entry(path: &str, bytes: Vec<u8>, location: BmadLocationClass) -> BmadSourceEntry {
    BmadSourceEntry::new(path, bytes, BmadSourceKind::SealedFoundation, location)
        .expect("valid source entry")
}

fn valid_foundation_snapshot() -> (BmadSourceSnapshot, Sha256Digest) {
    let managed_paths = [
        "runtime/method/6.10.0/architect-persona.instructions.md",
        "runtime/method/6.10.0/architecture-create.instructions.md",
        "runtime/method/6.10.0/bmad-help.instructions.md",
    ];
    let managed = managed_paths
        .iter()
        .map(|path| {
            source_entry(
                path,
                std::fs::read(foundation_path(path)).expect("managed instruction"),
                BmadLocationClass::ManagedProjection,
            )
        })
        .collect::<Vec<_>>();
    let descriptor_bytes = std::fs::read(foundation_path("normalized/bmad-help.package.json"))
        .expect("repository-owned normalized descriptor");

    let ledger = std::fs::read(foundation_path("semantic-source-ledger.json"))
        .expect("semantic source ledger");
    let ledger_hash = desktop_runtime::sha256_bytes(&ledger);
    let mut entries = managed;
    entries.push(source_entry(
        "semantic-source-ledger.json",
        ledger,
        BmadLocationClass::ManagedMetadata,
    ));
    entries.push(source_entry(
        "normalized/bmad-help.package.json",
        descriptor_bytes,
        BmadLocationClass::ManagedMetadata,
    ));
    (
        BmadSourceSnapshot::new(entries).expect("foundation snapshot"),
        ledger_hash,
    )
}

fn replace_snapshot_entry(
    snapshot: &BmadSourceSnapshot,
    path: &str,
    replacement: &[u8],
) -> BmadSourceSnapshot {
    let entries = snapshot
        .entries()
        .iter()
        .map(|entry| {
            if entry.path() == path {
                BmadSourceEntry::new(
                    path,
                    replacement.to_vec(),
                    entry.source_kind(),
                    entry.location(),
                )
                .expect("replacement entry")
            } else {
                entry.clone()
            }
        })
        .collect();
    BmadSourceSnapshot::new(entries).expect("replacement snapshot")
}

#[test]
fn sealed_loader_uses_generated_contract_shape_and_observed_final_bytes() {
    let (snapshot, ledger_hash) = valid_foundation_snapshot();
    let loaded = BmadPackageLoader::load(&snapshot, ledger_hash).expect("sealed package");

    assert_eq!(loaded.package_name, "bmad-method");
    assert_eq!(loaded.package_version, "6.10.0");
    assert_eq!(
        loaded.observed_inventory_hash,
        snapshot.observed_inventory_hash()
    );
    assert_eq!(loaded.skills.len(), 2);
    assert!(loaded.skills.iter().all(|skill| !skill.capability_enabled));
    let help = loaded
        .skills
        .iter()
        .find(|skill| skill.skill_name == "bmad-help")
        .expect("help skill");
    assert_eq!(help.entrypoint_kind.as_str(), "direct");
    assert!(help.structurally_eligible);
    assert!(loaded
        .skills
        .iter()
        .filter(|skill| skill.skill_name != "bmad-help")
        .all(|skill| !skill.structurally_eligible));
}

#[test]
fn loader_rejects_ledger_drift_staging_substitution_and_managed_resource_tamper() {
    let (snapshot, ledger_hash) = valid_foundation_snapshot();
    let wrong_ledger = desktop_runtime::sha256_bytes(b"different semantic ledger");
    assert_eq!(
        BmadPackageLoader::load(&snapshot, wrong_ledger)
            .expect_err("ledger drift")
            .code(),
        BmadKernelErrorCode::SemanticLedgerMismatch
    );

    let descriptor_entry = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path() == "normalized/bmad-help.package.json")
        .expect("descriptor entry");
    let mut descriptor: Value =
        serde_json::from_slice(descriptor_entry.bytes()).expect("descriptor JSON");
    descriptor["finalCompositeInventoryHash"] =
        json!("sha256:1111111111111111111111111111111111111111111111111111111111111111");
    descriptor["descriptorHash"] = json!(canonical_hash_without_field(
        "bmad-package-descriptor",
        1,
        &descriptor,
        "descriptorHash",
    )
    .expect("descriptor self hash")
    .to_string());
    let staging = replace_snapshot_entry(
        &snapshot,
        "normalized/bmad-help.package.json",
        &serde_json::to_vec_pretty(&descriptor).expect("descriptor bytes"),
    );
    assert_eq!(
        BmadPackageLoader::load(&staging, ledger_hash)
            .expect_err("staging hash cannot substitute for observed bytes")
            .code(),
        BmadKernelErrorCode::FinalInventoryMismatch
    );

    let tampered = replace_snapshot_entry(
        &snapshot,
        "runtime/method/6.10.0/bmad-help.instructions.md",
        b"tampered but internally re-hashed entry",
    );
    assert_eq!(
        BmadPackageLoader::load(&tampered, ledger_hash)
            .expect_err("descriptor-to-resource binding must fail")
            .code(),
        BmadKernelErrorCode::ManagedResourceMismatch
    );
}

#[test]
fn source_snapshot_rejects_aliases_escape_controls_and_size_overflow() {
    assert!(BmadSourceEntry::new(
        "../SKILL.md",
        vec![],
        BmadSourceKind::MethodComposite,
        BmadLocationClass::HostNativeAgents,
    )
    .is_err());
    assert!(BmadSourceEntry::new(
        "skills/CON/readme.md",
        vec![],
        BmadSourceKind::MethodComposite,
        BmadLocationClass::HostNativeAgents,
    )
    .is_err());
    assert!(BmadSourceEntry::new(
        "skills/a/\u{202e}evil.md",
        vec![],
        BmadSourceKind::MethodComposite,
        BmadLocationClass::HostNativeAgents,
    )
    .is_err());
    assert!(BmadSourceEntry::new(
        "skills/a/SKILL.md",
        vec![0_u8; 1_048_577],
        BmadSourceKind::MethodComposite,
        BmadLocationClass::HostNativeAgents,
    )
    .is_err());
}

fn layer(
    graph: BmadConfigGraphKind,
    kind: &str,
    ordinal: u8,
    required: bool,
    entries: Value,
) -> BmadConfigLayer {
    BmadConfigLayer::valid(graph, kind, ordinal, required, entries).expect("config layer")
}

#[test]
fn config_resolver_preserves_three_graphs_and_exact_merge_semantics() {
    let central = BmadConfigGraph::new(
        BmadConfigGraphKind::MethodCentralToml,
        vec![
            layer(
                BmadConfigGraphKind::MethodCentralToml,
                "installer_team",
                0,
                true,
                json!({
                    "core": {"communication_language": "English", "tags": ["base"]},
                    "agents": [{"code": "architect", "tone": "measured"}],
                    "unknown_extension": {"kept": true}
                }),
            ),
            layer(
                BmadConfigGraphKind::MethodCentralToml,
                "installer_user",
                1,
                false,
                json!({
                    "core": {"communication_language": "Dutch", "tags": ["user"]},
                    "agents": [
                        {"code": "architect", "tone": "concise"},
                        {"code": "analyst", "tone": "curious"}
                    ]
                }),
            ),
            layer(
                BmadConfigGraphKind::MethodCentralToml,
                "custom_team",
                2,
                false,
                json!({"unknown_extension": null}),
            ),
        ],
    )
    .expect("central graph");
    let skill = BmadConfigGraph::new(
        BmadConfigGraphKind::SkillCustomizationToml,
        vec![layer(
            BmadConfigGraphKind::SkillCustomizationToml,
            "packaged_default",
            0,
            true,
            json!({"persona": {"principles": ["boring technology"]}}),
        )],
    )
    .expect("skill graph");
    let compatibility = BmadConfigGraph::new(
        BmadConfigGraphKind::CompatibilityYaml,
        vec![layer(
            BmadConfigGraphKind::CompatibilityYaml,
            "method_module_yaml",
            0,
            true,
            json!({"project_name": "Example"}),
        )],
    )
    .expect("compatibility graph");

    let resolved = BmadConfigResolver::resolve(&central, &skill, &compatibility)
        .expect("three separate config resolutions");
    assert_eq!(
        resolved.central.value["core"]["communication_language"],
        "Dutch"
    );
    assert_eq!(
        resolved.central.value["core"]["tags"],
        json!(["base", "user"])
    );
    assert_eq!(
        resolved.central.value["agents"]
            .as_array()
            .expect("agents")
            .len(),
        2
    );
    assert_eq!(resolved.central.value["agents"][0]["tone"], "concise");
    assert_eq!(resolved.central.value["unknown_extension"]["kept"], true);
    assert!(resolved
        .central
        .warnings
        .iter()
        .any(|warning| warning.code == "config_deletion_unsupported"));
    assert_eq!(
        resolved.skill.graph_kind,
        BmadConfigGraphKind::SkillCustomizationToml
    );
    assert_eq!(resolved.compatibility.value["project_name"], "Example");
    assert_ne!(
        resolved.central.resolution_hash,
        resolved.skill.resolution_hash
    );
}

#[test]
fn config_resolver_skips_optional_invalid_layers_but_rejects_required_or_policy_data() {
    let optional = BmadConfigGraph::new(
        BmadConfigGraphKind::MethodCentralToml,
        vec![
            layer(
                BmadConfigGraphKind::MethodCentralToml,
                "installer_team",
                0,
                true,
                json!({"project": "safe"}),
            ),
            BmadConfigLayer::invalid(
                BmadConfigGraphKind::MethodCentralToml,
                "installer_user",
                1,
                false,
                "invalid TOML",
            )
            .expect("optional invalid layer"),
        ],
    )
    .expect("optional graph");
    let resolved = BmadConfigResolver::resolve_graph(&optional).expect("skip optional invalid");
    assert_eq!(resolved.value["project"], "safe");
    assert!(resolved
        .warnings
        .iter()
        .any(|warning| warning.code == "config_optional_layer_invalid"));

    let required = BmadConfigGraph::new(
        BmadConfigGraphKind::MethodCentralToml,
        vec![BmadConfigLayer::invalid(
            BmadConfigGraphKind::MethodCentralToml,
            "installer_team",
            0,
            true,
            "invalid base",
        )
        .expect("required invalid layer")],
    )
    .expect("required graph");
    assert_eq!(
        BmadConfigResolver::resolve_graph(&required)
            .expect_err("required base fails closed")
            .code(),
        BmadKernelErrorCode::ConfigMergeConflict
    );

    let policy = BmadConfigGraph::new(
        BmadConfigGraphKind::MethodCentralToml,
        vec![layer(
            BmadConfigGraphKind::MethodCentralToml,
            "installer_team",
            0,
            true,
            json!({"airlock": {"bypass": true}}),
        )],
    )
    .expect("policy graph");
    assert_eq!(
        BmadConfigResolver::resolve_graph(&policy)
            .expect_err("workspace config cannot grant policy")
            .code(),
        BmadKernelErrorCode::ConfigPolicyForbidden
    );
}
