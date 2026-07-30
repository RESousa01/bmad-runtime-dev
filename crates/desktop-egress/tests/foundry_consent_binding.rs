#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "invalid static fixtures should fail the test at construction, and a \
              failing assertion needs the case name to be diagnosable"
)]
//! FND-042: the version two egress manifest preimage and the selection layer
//! that makes a profile or catalog change invalidate an existing approval.
//!
//! The golden vector below is the cross-language contract. The support plane
//! recomputes `localEgressManifestHash` from the members of
//! `model-access-request-v2.schema.json`; until this preimage existed it could
//! only compare the request's copy against the signed decision's copy, which is
//! agreement between two numbers the same client produced.

use desktop_egress::{
    local_egress_manifest_hash_v2, ApproveDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextCandidate, ContextClassification, ContextEgressManifest, ContextPreparer, EgressError,
    EgressLimits, LocalEgressManifestV2Input, LocalEgressManifestV2Item, LocalEgressManifestV2Tool,
    MemoryDecisionLedger, ModelInvocationBinding, ModelInvocationBindingDraft, ModelRequestMode,
    ModelSelectionBindingV2, PatternSecretScanner, PrepareContextInput, RetentionMode,
    LOCAL_EGRESS_MANIFEST_V2_SCHEMA,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};

/// The shared golden vector. Both languages must produce this digest for the
/// fixed draft in [`golden_manifest_input`].
///
/// The preimage is `sapphirus:context-egress-manifest:v2\n` followed by the
/// RFC 8785 canonical JSON of the draft.
const EXPECTED_LOCAL_EGRESS_MANIFEST_V2_HASH: &str =
    "sha256:dd79b16abb92e95a24c9e5dc2ded05aa5e3603549f7371a620e512e7520fd095";

/// Named so the mutation tables below stay inside the complexity budget.
type ManifestMutation = (&'static str, Box<dyn Fn(&mut LocalEgressManifestV2Input)>);
type SelectionMutation = (&'static str, Box<dyn Fn(&mut ModelSelectionBindingV2)>);

const FIXED: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn label(value: &str) -> RelativeWorkspacePath {
    RelativeWorkspacePath::new(value).expect("valid fixture label")
}

fn fixed() -> Sha256Digest {
    Sha256Digest::parse(FIXED).expect("valid fixture digest")
}

/// The fixed version two manifest draft. Every member is present in
/// `model-access-request-v2.schema.json`, which is the property that makes this
/// preimage server-recomputable.
fn golden_manifest_input() -> LocalEgressManifestV2Input {
    LocalEgressManifestV2Input {
        request_id: id("request_01J00000000000000000000000"),
        registration_id: id("dreg_AAAAAAAAAAAAAAAAAAAAAAAAAA"),
        purpose: "bmad_help".to_owned(),
        model_role: "planner".to_owned(),
        request_mode: ModelRequestMode::StreamingChat,
        model_profile_id: "balanced-reasoning".to_owned(),
        requested_variant_id: None,
        catalog_version:
            "7:sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        model_profile_hash: fixed(),
        model_capability_hash: fixed(),
        context_snapshot_id: id("snapshot_01J00000000000000000000000"),
        canonical_output_schema_id: None,
        canonical_output_schema_hash: None,
        retention_mode: RetentionMode::TransientNoStore,
        budget_class: "interactive-standard".to_owned(),
        items: vec![LocalEgressManifestV2Item {
            client_item_id: id("item_01J00000000000000000000000"),
            relative_label: label("README.md"),
            semantic_role: "reference".to_owned(),
            language: "markdown".to_owned(),
            content_hash: fixed(),
            byte_count: 128,
            classification: ContextClassification::Internal,
        }],
        tools: Vec::new(),
    }
}

#[test]
fn the_version_two_manifest_hash_matches_the_shared_golden_vector() {
    let actual = local_egress_manifest_hash_v2(&golden_manifest_input()).expect("golden hash");
    assert_eq!(
        actual.to_string(),
        EXPECTED_LOCAL_EGRESS_MANIFEST_V2_HASH,
        "the version two local egress manifest preimage drifted; both languages \
         must move together",
    );
    assert_eq!(
        LOCAL_EGRESS_MANIFEST_V2_SCHEMA,
        "sapphirus.context-egress-manifest.v2",
    );
}

/// Every member of the draft is load-bearing. A support plane that recomputed
/// this while ignoring one of them would accept a request whose signed decision
/// covered something else.
#[test]
fn every_manifest_member_changes_the_version_two_hash() {
    let baseline = local_egress_manifest_hash_v2(&golden_manifest_input()).expect("baseline");
    let mutations: Vec<ManifestMutation> = vec![
        (
            "requestId",
            Box::new(|input| input.request_id = id("request_01J00000000000000000000001")),
        ),
        (
            "registrationId",
            Box::new(|input| input.registration_id = id("dreg_BBBBBBBBBBBBBBBBBBBBBBBBBB")),
        ),
        (
            "purpose",
            Box::new(|input| input.purpose = "chat".to_owned()),
        ),
        (
            "modelRole",
            Box::new(|input| input.model_role = "reviewer".to_owned()),
        ),
        (
            "requestMode",
            Box::new(|input| input.request_mode = ModelRequestMode::StructuredResponse),
        ),
        (
            "modelProfileId",
            Box::new(|input| input.model_profile_id = "fast-drafting".to_owned()),
        ),
        (
            "requestedVariantId",
            Box::new(|input| input.requested_variant_id = Some("high".to_owned())),
        ),
        (
            "catalogVersion",
            Box::new(|input| input.catalog_version = "8:sha256:00".to_owned()),
        ),
        (
            "modelProfileHash",
            Box::new(|input| input.model_profile_hash = sha256_bytes(b"other profile")),
        ),
        (
            "modelCapabilityHash",
            Box::new(|input| input.model_capability_hash = sha256_bytes(b"other capability")),
        ),
        (
            "contextSnapshotId",
            Box::new(|input| input.context_snapshot_id = id("snapshot_01J00000000000000000000001")),
        ),
        (
            "canonicalOutputSchema",
            Box::new(|input| {
                input.canonical_output_schema_id = Some(id("sapphirus.bmad-method-help-proposal"));
                input.canonical_output_schema_hash = Some(fixed());
            }),
        ),
        (
            "budgetClass",
            Box::new(|input| input.budget_class = "interactive-premium".to_owned()),
        ),
        (
            "item.contentHash",
            Box::new(|input| input.items[0].content_hash = sha256_bytes(b"rewritten bytes")),
        ),
        (
            "item.byteCount",
            Box::new(|input| input.items[0].byte_count = 129),
        ),
        (
            "item.classification",
            Box::new(|input| input.items[0].classification = ContextClassification::Confidential),
        ),
        (
            "item.relativeLabel",
            Box::new(|input| input.items[0].relative_label = label("docs/README.md")),
        ),
        (
            "item.semanticRole",
            Box::new(|input| input.items[0].semantic_role = "primary".to_owned()),
        ),
        (
            "item.language",
            Box::new(|input| input.items[0].language = "plaintext".to_owned()),
        ),
        (
            "item.clientItemId",
            Box::new(|input| input.items[0].client_item_id = id("item_01J00000000000000000000001")),
        ),
        (
            "tools",
            Box::new(|input| {
                input.tools.push(LocalEgressManifestV2Tool {
                    tool_name: "search_workspace".to_owned(),
                    description: "Find files".to_owned(),
                    parameters_schema_id: id("sapphirus.tool-search"),
                    parameters_schema_hash: fixed(),
                });
            }),
        ),
    ];
    for (name, mutate) in mutations {
        let mut input = golden_manifest_input();
        mutate(&mut input);
        let mutated = local_egress_manifest_hash_v2(&input).expect("mutated hash");
        assert_ne!(mutated, baseline, "{name} did not change the manifest hash");
    }
}

/// A null variant and the literal string are different preimages, exactly as
/// the version two consent envelope requires of its three nullable members.
#[test]
fn an_absent_variant_is_not_the_string_null() {
    let mut named = golden_manifest_input();
    named.requested_variant_id = Some("null".to_owned());
    assert_ne!(
        local_egress_manifest_hash_v2(&named).expect("named"),
        local_egress_manifest_hash_v2(&golden_manifest_input()).expect("absent"),
    );
}

#[test]
fn the_manifest_refuses_shapes_that_could_not_be_reviewed() {
    let mut empty_items = golden_manifest_input();
    empty_items.items.clear();
    assert_eq!(
        local_egress_manifest_hash_v2(&empty_items),
        Err(EgressError::InvalidManifest),
    );

    let mut duplicated = golden_manifest_input();
    let duplicate = duplicated.items[0].clone();
    duplicated.items.push(duplicate);
    assert_eq!(
        local_egress_manifest_hash_v2(&duplicated),
        Err(EgressError::InvalidManifest),
    );

    // A schema identifier and its hash are one fact; half of it would leave a
    // structured request signed with an unbound output schema.
    let mut half_schema = golden_manifest_input();
    half_schema.canonical_output_schema_id = Some(id("sapphirus.bmad-method-help-proposal"));
    assert_eq!(
        local_egress_manifest_hash_v2(&half_schema),
        Err(EgressError::InvalidManifest),
    );
}

fn fixture_manifest() -> ContextEgressManifest {
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_ref"),
            project_ref: id("project_ref"),
            run_ref: id("run_ref"),
            purpose: "bmad_help".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("sapphirus.bmad-method-help-proposal"),
            canonical_output_schema_hash: fixed(),
            provider_profile_hash: fixed(),
            model_profile_hash: fixed(),
            deployment_hash: fixed(),
            policy_hash: fixed(),
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(61_000),
            limits: EgressLimits {
                maximum_context_items: 8,
                maximum_context_bytes: 8_192,
                maximum_token_estimate: 4_096,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_01J00000000000000000000000"),
                relative_label: label("README.md"),
                semantic_role: "reference".to_owned(),
                language: Some("markdown".to_owned()),
                classification: ContextClassification::Internal,
                content: "safe reviewed context".to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("fixture manifest")
}

fn fixture_binding(manifest: &ContextEgressManifest) -> ModelInvocationBinding {
    ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_01J00000000000000000000000"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("install_01J00000000000000000000000"),
        session_authority_hash: fixed(),
        manifest_hash: manifest.manifest_hash,
        purpose: manifest.draft.purpose.clone(),
        model_role: manifest.draft.model_role.clone(),
        canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
        canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
        provider_profile_hash: manifest.draft.provider_profile_hash,
        model_profile_hash: manifest.draft.model_profile_hash,
        deployment_hash: manifest.draft.deployment_hash,
        policy_hash: manifest.draft.policy_hash,
        region: manifest.draft.region.clone(),
        retention_mode: manifest.draft.retention_mode,
        consent_disclosure_hash: sha256_bytes(b"reviewed disclosure"),
    }
    .seal()
    .expect("sealed binding")
}

fn fixture_selection() -> ModelSelectionBindingV2 {
    ModelSelectionBindingV2 {
        request_mode: ModelRequestMode::StreamingChat,
        model_profile_id: "balanced-reasoning".to_owned(),
        requested_variant_id: None,
        catalog_version: "7:sha256:00".to_owned(),
        catalog_hash: fixed(),
        model_profile_hash: fixed(),
        model_capability_hash: fixed(),
        context_snapshot_id: id("snapshot_01J00000000000000000000000"),
        budget_class: "interactive-standard".to_owned(),
        local_egress_manifest_hash: local_egress_manifest_hash_v2(&golden_manifest_input())
            .expect("manifest hash"),
    }
}

/// The behaviour FND-042 names: a profile or catalog change invalidates an
/// existing approval, because every hash that crosses the wire moves with it.
#[test]
fn a_transplanted_profile_catalog_or_variant_changes_every_wire_hash() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest);
    let baseline = fixture_selection()
        .seal(&binding)
        .expect("sealed selection");

    let transplants: Vec<SelectionMutation> = vec![
        (
            "profile",
            Box::new(|selection| selection.model_profile_id = "fast-drafting".to_owned()),
        ),
        (
            "profile hash",
            Box::new(|selection| selection.model_profile_hash = sha256_bytes(b"transplanted")),
        ),
        (
            "catalog version",
            Box::new(|selection| selection.catalog_version = "8:sha256:00".to_owned()),
        ),
        (
            "catalog hash",
            Box::new(|selection| selection.catalog_hash = sha256_bytes(b"other catalog")),
        ),
        (
            "variant",
            Box::new(|selection| selection.requested_variant_id = Some("high".to_owned())),
        ),
        (
            "request mode",
            Box::new(|selection| selection.request_mode = ModelRequestMode::StructuredResponse),
        ),
        (
            "context snapshot",
            Box::new(|selection| {
                selection.context_snapshot_id = id("snapshot_01J00000000000000000000009");
            }),
        ),
        (
            "capability hash",
            Box::new(|selection| selection.model_capability_hash = sha256_bytes(b"widened")),
        ),
        (
            "budget class",
            Box::new(|selection| selection.budget_class = "batch-cheap".to_owned()),
        ),
        (
            "local egress manifest",
            Box::new(|selection| selection.local_egress_manifest_hash = sha256_bytes(b"other")),
        ),
    ];
    for (name, mutate) in transplants {
        let mut selection = fixture_selection();
        mutate(&mut selection);
        let sealed = selection.seal(&binding).expect("sealed transplant");
        assert_ne!(
            sealed.invocation_binding_hash(),
            baseline.invocation_binding_hash(),
            "{name} did not change the invocation binding hash",
        );
        assert_ne!(
            sealed.consent_disclosure_hash(),
            baseline.consent_disclosure_hash(),
            "{name} did not change the consent disclosure hash",
        );
    }
}

/// The version two consumption hash is derived from a real single-use
/// consumption, so it cannot be minted beside a consumption the ledger never
/// produced or beside a different selection.
#[test]
fn the_version_two_consumption_hash_binds_the_selection_it_was_spent_under() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest);
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_01J00000000000000000000000"),
            issued_at: UnixMillis(1_500),
            expires_at: UnixMillis(60_000),
        })
        .expect("approved decision");
    let consumption = service
        .consume(ConsumeDecisionInput {
            decision: &decision,
            binding: &binding,
            invocation_id: id("invoke_01J00000000000000000000000"),
            consumed_at: UnixMillis(2_000),
        })
        .expect("consumed decision");

    let sealed = fixture_selection()
        .seal(&binding)
        .expect("sealed selection");
    let baseline = sealed
        .consumption_hash_for(&consumption)
        .expect("version two consumption hash");

    let mut moved = fixture_selection();
    moved.model_profile_id = "fast-drafting".to_owned();
    let moved = moved.seal(&binding).expect("sealed transplant");
    assert_ne!(
        moved
            .consumption_hash_for(&consumption)
            .expect("transplanted consumption hash"),
        baseline,
    );

    // A consumption produced for a different reviewed manifest is refused
    // outright rather than rehashed under this selection.
    let other_manifest = fixture_manifest();
    let mut other_draft = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_01J00000000000000000000001"),
        tenant_ref: other_manifest.draft.tenant_ref.clone(),
        project_ref: other_manifest.draft.project_ref.clone(),
        run_ref: other_manifest.draft.run_ref.clone(),
        installation_id: id("install_01J00000000000000000000000"),
        session_authority_hash: fixed(),
        manifest_hash: other_manifest.manifest_hash,
        purpose: other_manifest.draft.purpose.clone(),
        model_role: other_manifest.draft.model_role.clone(),
        canonical_output_schema_id: other_manifest.draft.canonical_output_schema_id.clone(),
        canonical_output_schema_hash: other_manifest.draft.canonical_output_schema_hash,
        provider_profile_hash: other_manifest.draft.provider_profile_hash,
        model_profile_hash: other_manifest.draft.model_profile_hash,
        deployment_hash: other_manifest.draft.deployment_hash,
        policy_hash: other_manifest.draft.policy_hash,
        region: other_manifest.draft.region.clone(),
        retention_mode: other_manifest.draft.retention_mode,
        consent_disclosure_hash: sha256_bytes(b"a different disclosure"),
    };
    other_draft.request_id = id("request_01J00000000000000000000002");
    let other_binding = other_draft.seal().expect("other binding");
    let other_sealed = fixture_selection()
        .seal(&other_binding)
        .expect("sealed against another binding");
    assert_eq!(
        other_sealed.consumption_hash_for(&consumption),
        Err(EgressError::DecisionBindingMismatch),
    );
}

#[test]
fn a_selection_with_an_empty_member_cannot_be_sealed() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest);
    for mutate in [
        Box::new(|selection: &mut ModelSelectionBindingV2| selection.model_profile_id.clear())
            as Box<dyn Fn(&mut ModelSelectionBindingV2)>,
        Box::new(|selection: &mut ModelSelectionBindingV2| selection.catalog_version.clear()),
        Box::new(|selection: &mut ModelSelectionBindingV2| selection.budget_class.clear()),
        Box::new(|selection: &mut ModelSelectionBindingV2| {
            selection.requested_variant_id = Some(String::new());
        }),
    ] {
        let mut selection = fixture_selection();
        mutate(&mut selection);
        assert_eq!(
            selection.seal(&binding).err(),
            Some(EgressError::InvalidInvocationBinding),
        );
    }
}
