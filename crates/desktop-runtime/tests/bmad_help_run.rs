#![allow(clippy::expect_used)]

use desktop_runtime::{
    canonical_hash_without_field, BmadCatalog, BmadCatalogBuilder, BmadHelpBindingCompiler,
    BmadHelpCatalogSource, BmadKernelErrorCode, BmadLoadedMethodPackage, BmadLoadedPackage,
    BmadLocationClass, BmadPackageLoader, BmadQualifiedHelpSource, BmadSealedHelpInvocation,
    BmadSourceEntry, BmadSourceKind, BmadSourceSnapshot, BmadTrustedHelpModelProfile,
    BmadTrustedHelpModelProfileData, Sha256Digest,
};
use serde_json::{json, Value};

const DESCRIPTOR_PATH: &str = "normalized/bmad-help.package.json";
const SEMANTIC_LEDGER_PATH: &str = "semantic-source-ledger.json";
const ADOPTION_LEDGER_PATH: &str = "adoption-ledger.json";
const HELP_INSTRUCTION_PATH: &str = "runtime/method/6.10.0/bmad-help.instructions.md";

fn foundation_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/bmad-foundation")
        .join(relative)
}

fn source_entry(path: &str, location: BmadLocationClass) -> BmadSourceEntry {
    BmadSourceEntry::new(
        path,
        std::fs::read(foundation_path(path)).expect("foundation resource"),
        BmadSourceKind::SealedFoundation,
        location,
    )
    .expect("valid source entry")
}

#[expect(
    clippy::too_many_lines,
    reason = "the sealed snapshot fixture enumerates the full reviewed managed set"
)]
fn sealed_snapshot() -> (BmadSourceSnapshot, Sha256Digest, Sha256Digest) {
    let semantic = source_entry(SEMANTIC_LEDGER_PATH, BmadLocationClass::ManagedMetadata);
    let adoption = source_entry(ADOPTION_LEDGER_PATH, BmadLocationClass::ManagedMetadata);
    let semantic_hash = semantic.content_hash();
    let adoption_hash = adoption.content_hash();
    let entries = vec![
        semantic,
        adoption,
        source_entry(DESCRIPTOR_PATH, BmadLocationClass::ManagedMetadata),
        source_entry(
            "runtime/method/6.10.0/architect-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/architecture-create.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(HELP_INSTRUCTION_PATH, BmadLocationClass::ManagedProjection),
        source_entry(
            "runtime/method/6.10.0/analyst-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/dev-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/pm-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/tech-writer-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/ux-designer-persona.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/brainstorming.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/domain-research.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/market-research.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/prfaq.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/product-brief.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/technical-research.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/code-review.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/correct-course.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/create-epics-and-stories.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/create-story.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/dev-story.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/implementation-readiness.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/qa-tests.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/quick-dev.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/retrospective.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/sprint-planning.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/ux-design.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/document-project.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/explain-concept.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/mermaid-gen.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/prd.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/validate-doc.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
        source_entry(
            "runtime/method/6.10.0/write-document.instructions.md",
            BmadLocationClass::ManagedProjection,
        ),
    ];
    (
        BmadSourceSnapshot::new(entries).expect("sealed source snapshot"),
        semantic_hash,
        adoption_hash,
    )
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::parse(value).expect("qualified digest")
}

fn loaded_method_and_catalog() -> (BmadLoadedMethodPackage, BmadCatalog) {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let loaded = BmadPackageLoader::load(&snapshot, semantic_hash, adoption_hash)
        .expect("qualified Method package");
    let (sources, graph_hash) = help_catalog_sources();
    let catalog = BmadCatalogBuilder::build_bound(loaded.package(), &sources, graph_hash)
        .expect("bound catalog");
    (loaded, catalog)
}

fn help_catalog_sources() -> (Vec<BmadHelpCatalogSource>, Sha256Digest) {
    let graph: Value = serde_json::from_slice(
        &std::fs::read(foundation_path("normalized/bmad-help-action-graph.json"))
            .expect("Help action graph"),
    )
    .expect("Help action graph JSON");
    let sources = graph["sources"]
        .as_array()
        .expect("catalog sources")
        .iter()
        .map(|source| {
            let rows: Vec<Vec<String>> =
                serde_json::from_value(source["rows"].clone()).expect("normalized rows");
            BmadHelpCatalogSource::from_rows(
                source["moduleCode"].as_str().expect("module code"),
                &rows,
            )
            .expect("catalog source")
        })
        .collect::<Vec<_>>();
    let graph_hash = Sha256Digest::parse(graph["graphHash"].as_str().expect("graph hash"))
        .expect("qualified graph hash");
    (sources, graph_hash)
}

fn trusted_model_profile() -> BmadTrustedHelpModelProfile {
    BmadTrustedHelpModelProfile::from_host_assertion(BmadTrustedHelpModelProfileData {
        provider_id: "azure-openai-managed".to_owned(),
        model_id: "gpt-5.2".to_owned(),
        deployment_id: "sapphirus-help".to_owned(),
        model_profile_hash: desktop_runtime::sha256_bytes(b"qualified model profile"),
        model_capability_hash: desktop_runtime::sha256_bytes(b"qualified model capability"),
        context_window_profile_hash: desktop_runtime::sha256_bytes(b"qualified context window"),
        egress_profile_hash: desktop_runtime::sha256_bytes(b"qualified egress profile"),
        request_schema_hash: desktop_runtime::sha256_bytes(b"qualified D2 request schema"),
    })
    .expect("trusted inert model profile")
}

#[test]
fn loader_retains_the_exact_sealed_help_source_and_qualified_identity_chain() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let loaded = BmadPackageLoader::load(&snapshot, semantic_hash, adoption_hash)
        .expect("qualified Method package");
    let package = loaded.package();
    let help = loaded.help_invocation();

    assert_package_identity(package, help);
    assert_instruction_identity(&snapshot, help);
    assert_policy_and_lineage_identity(help, semantic_hash, adoption_hash);
    assert_redacted_debug(help);
}

fn assert_package_identity(package: &BmadLoadedPackage, help: &BmadSealedHelpInvocation) {
    assert_eq!(package.package_name, "bmad-method");
    assert_eq!(package.package_version, "6.10.0");
    assert_eq!(help.package_version_id(), &package.package_version_id);
    assert_eq!(help.descriptor_hash(), package.descriptor_hash);
    assert_eq!(
        help.observed_inventory_hash(),
        package.observed_inventory_hash
    );
    assert_eq!(
        help.source_snapshot_hash(),
        digest("sha256:8b33c55a4d67d0b258fedbb75d1cb09dbc7f711bc9bdc794d8b052b31fce6d86")
    );
    assert_eq!(
        help.final_inventory_hash(),
        digest("sha256:5e19f129bf2ffd0dd341f94b042eee1c8f340557b0bd805df15c002645bd161f")
    );

    assert_eq!(help.module_code(), "core");
    assert_eq!(help.skill_name(), "bmad-help");
    assert_eq!(
        help.source_entrypoint_path(),
        "src/core-skills/bmad-help/SKILL.md"
    );
    assert_eq!(
        help.source_entrypoint_hash(),
        digest("sha256:718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642")
    );
    assert_eq!(
        help.resource_set_hash(),
        digest("sha256:cc52d4eaa9c81f268dbec83dd56b4dfd1c983765e83b588b33f2c34908a57b94")
    );
    assert_eq!(
        help.projection_hash(),
        digest("sha256:8b33eab28f8518af84da7801178a21bc5e2bedf3c66b960eabf9c760e2bac838")
    );
    assert_eq!(
        help.skill_descriptor_hash(),
        digest("sha256:8ff723060d7dee20e5f9aa45c02c386a7baf25037a3463081524ffc0d40195da")
    );
    assert_eq!(
        help.execution_profile_hash(),
        digest("sha256:02b16af451ee6b4a60d0e446f0ea911b6b57f0c646845ed8bdd81ce09f2e1485")
    );
}

fn assert_instruction_identity(snapshot: &BmadSourceSnapshot, help: &BmadSealedHelpInvocation) {
    assert_eq!(help.managed_instruction_path(), HELP_INSTRUCTION_PATH);
    assert_eq!(help.managed_instruction_format(), "SapphirusManagedV1");
    assert_eq!(help.instruction_bytes().len(), 1_283);
    assert_eq!(
        help.managed_instruction_hash(),
        digest("sha256:d3d3c91d516d32546c446503d88957f6e499c504370b6749b5936f786643df66")
    );
    assert_eq!(
        desktop_runtime::sha256_bytes(help.instruction_bytes()),
        help.managed_instruction_hash()
    );
    let snapshot_instruction = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path() == HELP_INSTRUCTION_PATH)
        .expect("snapshot instruction");
    assert_eq!(
        help.instruction_bytes().as_ptr(),
        snapshot_instruction.bytes().as_ptr(),
        "the sealed wrapper must share the package-owned bytes"
    );
}

fn assert_policy_and_lineage_identity(
    help: &BmadSealedHelpInvocation,
    semantic_hash: Sha256Digest,
    adoption_hash: Sha256Digest,
) {
    assert_eq!(help.distribution_profile(), "sapphirus_package");
    assert_eq!(help.install_profile(), "SapphirusManagedV1");
    assert_eq!(help.validation_profile(), "MethodOfficialSkillV6");
    assert_eq!(help.blocked_tool_intents(), ["file_read", "web"]);
    assert_eq!(
        help.source_member_ids(),
        [
            "method-001",
            "method-002",
            "method-003",
            "method-004",
            "method-005"
        ]
    );
    assert_eq!(help.semantic_ledger_hash(), semantic_hash);
    assert_eq!(help.adoption_ledger_hash(), adoption_hash);
    assert_eq!(
        help.module_metadata_hash(),
        digest("sha256:46f8972746f0d4e49358fdf94b0c1ba856fd7a8eb66abc75d5aaff0624540479")
    );
    assert_eq!(
        help.module_help_catalog_hash(),
        digest("sha256:e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d")
    );
    assert_eq!(
        help.central_config_graph_hash(),
        digest("sha256:b1896b35cc8596efe832f031b7579e3c9150aefab009a47be20318944fb7dce1")
    );
    assert_eq!(
        help.central_config_resolution_hash(),
        digest("sha256:2b40628510f83e186fe5e60414aecdddfd1ae823334a2ab1ecf94d52705d97ba")
    );

    let replacements = help.host_input_replacements();
    assert_eq!(replacements.len(), 2);
    assert_eq!(replacements[0].tool_intent(), "file_read");
    assert_eq!(replacements[0].input_kind(), "catalog_snapshot");
    assert_eq!(
        replacements[0].input_schema_hash(),
        digest("sha256:4dc4d3136db3c7ac2a40c61f12658db27791f525e8559f67bdaac7a018a50ddc")
    );
    assert_eq!(replacements[1].tool_intent(), "web");
    assert_eq!(replacements[1].input_kind(), "unavailable_fact");
    assert_eq!(
        replacements[1].input_schema_hash(),
        digest("sha256:00584aeb615fd1e6ba32e4e781862cf77b6525b2a7c0dca095e6ba9adf084697")
    );
    let source_paths = help
        .source_closure()
        .iter()
        .map(BmadQualifiedHelpSource::path)
        .collect::<Vec<_>>();
    assert_eq!(
        source_paths,
        [
            "src/bmm-skills/module-help.csv",
            "src/bmm-skills/module.yaml",
            "src/core-skills/bmad-help/SKILL.md",
            "src/core-skills/module-help.csv",
            "src/core-skills/module.yaml",
        ]
    );
    assert_eq!(help.source_closure()[2].treatment(), "adapt");
}

fn assert_redacted_debug(help: &BmadSealedHelpInvocation) {
    let first_line = std::str::from_utf8(help.instruction_bytes())
        .expect("managed instruction UTF-8")
        .lines()
        .next()
        .expect("instruction line");
    let debug = format!("{help:?}");
    assert!(!debug.contains(first_line));
    assert!(debug.contains("<redacted:1283 bytes>"));
}

#[test]
fn loader_rejects_nested_help_hash_drift_after_the_outer_descriptor_is_resealed() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let descriptor_entry = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path() == DESCRIPTOR_PATH)
        .expect("descriptor entry");
    let mut descriptor: Value =
        serde_json::from_slice(descriptor_entry.bytes()).expect("descriptor JSON");
    let help = descriptor["skills"]
        .as_array_mut()
        .expect("skills")
        .iter_mut()
        .find(|skill| skill["skillName"] == "bmad-help")
        .expect("Help skill");
    help["executionProfile"]["validationProfile"] = json!("MethodStepWorkflowV6");
    descriptor["descriptorHash"] = json!(canonical_hash_without_field(
        "bmad-package-descriptor",
        1,
        &descriptor,
        "descriptorHash",
    )
    .expect("descriptor self hash")
    .to_string());

    let replacement = serde_json::to_vec_pretty(&descriptor).expect("descriptor bytes");
    let entries = snapshot
        .entries()
        .iter()
        .map(|entry| {
            if entry.path() == DESCRIPTOR_PATH {
                BmadSourceEntry::new(
                    DESCRIPTOR_PATH,
                    replacement.clone(),
                    entry.source_kind(),
                    entry.location(),
                )
                .expect("replacement descriptor")
            } else {
                entry.clone()
            }
        })
        .collect();
    let tampered = BmadSourceSnapshot::new(entries).expect("tampered snapshot");
    assert_eq!(
        BmadPackageLoader::load(&tampered, semantic_hash, adoption_hash)
            .expect_err("nested profile hash drift must fail closed")
            .code(),
        BmadKernelErrorCode::SealedHelpMismatch
    );
}

#[test]
fn loader_rejects_coordinated_projection_and_skill_hash_substitution() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let mut descriptor = descriptor_value(&snapshot);
    let projection = descriptor["instructionProjections"]
        .as_array_mut()
        .expect("instruction projections")
        .iter_mut()
        .find(|candidate| candidate["managedInstruction"]["path"] == HELP_INSTRUCTION_PATH)
        .expect("Help projection");
    projection["blockedToolIntents"] = json!(["web", "file_read"]);
    reseal(projection, "bmad-instruction-projection", "projectionHash");
    let substituted_projection_hash = projection["projectionHash"].clone();

    let skill = descriptor["skills"]
        .as_array_mut()
        .expect("skills")
        .iter_mut()
        .find(|candidate| candidate["skillName"] == "bmad-help")
        .expect("Help skill");
    skill["instructionProjectionHash"] = substituted_projection_hash;
    reseal(skill, "bmad-skill-descriptor", "skillDescriptorHash");
    reseal(&mut descriptor, "bmad-package-descriptor", "descriptorHash");

    let substituted = replace_descriptor(&snapshot, &descriptor);
    assert_eq!(
        BmadPackageLoader::load(&substituted, semantic_hash, adoption_hash)
            .expect_err("coordinated authority substitution must fail closed")
            .code(),
        BmadKernelErrorCode::SealedHelpMismatch
    );
}

#[test]
fn loader_rejects_resealed_source_inventory_drift_from_the_help_closure() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let mut descriptor = descriptor_value(&snapshot);
    let inventory_entry = descriptor["resourceInventory"]
        .as_array_mut()
        .expect("resource inventory")
        .iter_mut()
        .find(|candidate| candidate["path"] == "src/core-skills/module-help.csv")
        .expect("core Help catalog inventory entry");
    inventory_entry["contentHash"] =
        json!("sha256:1111111111111111111111111111111111111111111111111111111111111111");
    reseal(&mut descriptor, "bmad-package-descriptor", "descriptorHash");

    let substituted = replace_descriptor(&snapshot, &descriptor);
    assert_eq!(
        BmadPackageLoader::load(&substituted, semantic_hash, adoption_hash)
            .expect_err("source inventory drift must fail closed")
            .code(),
        BmadKernelErrorCode::SealedHelpMismatch
    );
}

#[test]
fn loader_rejects_adoption_ledger_substitution() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let wrong_adoption = desktop_runtime::sha256_bytes(b"different adoption ledger");
    assert_ne!(wrong_adoption, adoption_hash);
    assert_eq!(
        BmadPackageLoader::load(&snapshot, semantic_hash, wrong_adoption)
            .expect_err("adoption substitution")
            .code(),
        BmadKernelErrorCode::AdoptionLedgerMismatch
    );
}

#[test]
fn compiler_derives_the_exact_non_runnable_no_agent_help_binding() {
    let (loaded, catalog) = loaded_method_and_catalog();
    let model = trusted_model_profile();
    let compiled = BmadHelpBindingCompiler::compile(loaded.help_invocation(), &catalog, &model)
        .expect("exact inert Help binding");

    assert!(!compiled.runnable());
    assert!(!compiled.completion_claimed());
    assert_eq!(
        compiled.instruction_bytes().as_ptr(),
        loaded.help_invocation().instruction_bytes().as_ptr()
    );
    assert_eq!(compiled.catalog_candidates(), catalog.help_actions);
    let binding = compiled.exact_binding();
    assert_eq!(binding.capability_key.module_code, "core");
    assert_eq!(binding.capability_key.skill_name, "bmad-help");
    assert_eq!(binding.capability_key.normalized_action, None);
    assert_eq!(binding.agent_roster_hash, None);
    assert_eq!(binding.agent_binding_hash, None);
    assert_eq!(binding.agent_binding, None);
    assert_eq!(binding.entrypoint_kind, "direct");
    assert!(binding
        .execution_profile
        .data
        .invocation_modes
        .actions
        .is_empty());
    assert!(binding
        .execution_profile
        .data
        .declared_tool_intents
        .is_empty());
    assert_eq!(
        binding.execution_profile.data.completion_evidence,
        ["artifact"]
    );
    assert_eq!(binding.model_binding_hash, model.model_binding_hash());
    assert_eq!(binding.egress_profile_hash, model.egress_profile_hash());
    binding.binding_hash().expect("canonical exact binding");

    assert_eq!(
        compiled.customization_hash(),
        digest("sha256:41d2f0d68d0a47e8fb9eeccd89f0409f2ab08a72eb1a76500d87a0919ecb9c8a")
    );
    assert_eq!(
        compiled.validation_profile_hash(),
        digest("sha256:ddd086622be73b637cdcb3562b4459ac3853f1aeae34a53c43775af66e4cbdf0")
    );
    assert_eq!(
        serde_json::to_value(compiled.step_table()).expect("step table JSON"),
        json!({
            "initialStepKey": "recommend",
            "steps": {"recommend": null},
            "tableHash": compiled.step_table().table_hash()
        })
    );
}

#[test]
fn compiler_uses_generated_schema_identities_and_the_trusted_request_schema() {
    let (loaded, catalog) = loaded_method_and_catalog();
    let model = trusted_model_profile();
    let compiled = BmadHelpBindingCompiler::compile(loaded.help_invocation(), &catalog, &model)
        .expect("exact inert Help binding");

    assert_eq!(compiled.request_schema_hash(), model.request_schema_hash());
    assert_eq!(
        compiled.proposal_schema_closure_hash(),
        digest("sha256:b86f154fd9ba6a7575171b849645e44a1753d191b4e3480e985b6042159af965")
    );
    assert_eq!(
        compiled.recommendation_schema_closure_hash(),
        digest("sha256:5cff85e4f40af521df317763fa86f405c11e410005c7a8901d5ed72a25320242")
    );
    assert_eq!(
        compiled.result_schema_closure_hash(),
        digest("sha256:600432affbca9baec428903211df9c4c2a1fc6ba595959013236e1a0f16bc746")
    );
    assert_eq!(
        compiled
            .exact_binding()
            .model_binding
            .data
            .response_schema_hash,
        compiled.proposal_schema_closure_hash()
    );
    assert_eq!(
        compiled.exact_binding().method_schema_hash,
        compiled.result_schema_closure_hash()
    );
}

#[test]
fn compiler_rejects_catalog_mutation_after_native_binding() {
    let (loaded, mut catalog) = loaded_method_and_catalog();
    catalog.help_actions[0].display_name = "substituted display".to_owned();
    assert_eq!(
        BmadHelpBindingCompiler::compile(
            loaded.help_invocation(),
            &catalog,
            &trusted_model_profile(),
        )
        .expect_err("catalog projection substitution")
        .code(),
        BmadKernelErrorCode::SealedHelpBindingMismatch
    );
}

#[test]
fn compiler_rejects_a_coordinated_catalog_graph_substitution() {
    let (snapshot, semantic_hash, adoption_hash) = sealed_snapshot();
    let loaded = BmadPackageLoader::load(&snapshot, semantic_hash, adoption_hash)
        .expect("qualified Method package");
    let (sources, expected_graph_hash) = help_catalog_sources();
    let substituted_graph_hash = desktop_runtime::sha256_bytes(b"substituted Help action graph");
    assert_ne!(substituted_graph_hash, expected_graph_hash);
    let substituted_catalog =
        BmadCatalogBuilder::build_bound(loaded.package(), &sources, substituted_graph_hash)
            .expect("internally consistent substituted catalog");

    assert_eq!(
        BmadHelpBindingCompiler::compile(
            loaded.help_invocation(),
            &substituted_catalog,
            &trusted_model_profile(),
        )
        .expect_err("the reviewed graph identity is fixed")
        .code(),
        BmadKernelErrorCode::SealedHelpBindingMismatch
    );
}

#[test]
fn compiler_and_trusted_model_debug_output_are_redacted() {
    let (loaded, catalog) = loaded_method_and_catalog();
    let model = trusted_model_profile();
    let compiled = BmadHelpBindingCompiler::compile(loaded.help_invocation(), &catalog, &model)
        .expect("exact inert Help binding");

    let compiled_debug = format!("{compiled:?}");
    let model_debug = format!("{model:?}");
    assert!(compiled_debug.contains("<redacted:1283 bytes>"));
    assert!(!compiled_debug.contains("Managed Method help guidance"));
    assert!(!model_debug.contains("azure-openai-managed"));
    assert!(!model_debug.contains("gpt-5.2"));
    assert!(!model_debug.contains("sapphirus-help"));
}

fn descriptor_value(snapshot: &BmadSourceSnapshot) -> Value {
    let entry = snapshot
        .entries()
        .iter()
        .find(|entry| entry.path() == DESCRIPTOR_PATH)
        .expect("descriptor entry");
    serde_json::from_slice(entry.bytes()).expect("descriptor JSON")
}

fn reseal(value: &mut Value, purpose: &str, field: &str) {
    value[field] = json!(canonical_hash_without_field(purpose, 1, value, field)
        .expect("nested self hash")
        .to_string());
}

fn replace_descriptor(snapshot: &BmadSourceSnapshot, descriptor: &Value) -> BmadSourceSnapshot {
    let replacement = serde_json::to_vec_pretty(descriptor).expect("descriptor bytes");
    let entries = snapshot
        .entries()
        .iter()
        .map(|entry| {
            if entry.path() == DESCRIPTOR_PATH {
                BmadSourceEntry::new(
                    DESCRIPTOR_PATH,
                    replacement.clone(),
                    entry.source_kind(),
                    entry.location(),
                )
                .expect("replacement descriptor")
            } else {
                entry.clone()
            }
        })
        .collect();
    BmadSourceSnapshot::new(entries).expect("replacement snapshot")
}
