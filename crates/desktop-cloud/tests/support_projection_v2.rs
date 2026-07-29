//! FND-042: the version two request and consent projection.
//!
//! Two properties are under test. The wire hashes are re-derived over the
//! selection, so a profile, catalog, variant, request-mode, context-snapshot, or
//! capability transplant cannot be presented under an approval that was given
//! for something else. And the version two local egress manifest hash is
//! *recomputed* by the projection from the exact projected items, so it is a
//! number the support plane can verify rather than one the client asserted.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "a fixture that does not hold is a broken test, and it must stop the run"
)]

use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::Mutex;

use async_trait::async_trait;
use desktop_cloud::{
    project_model_access_request_v2, resolve_model_selection,
    validate_model_capabilities, AuthorizedModelRequest, CanonicalProjectionInputsV2, CloudError,
    ConsentSubject, ConsentWindow, DeclaredModelTool, HttpExecutor, HttpResponse,
    InstallationConsentSigner, LeaseBinding, ModelCapabilityRejection, ModelCatalogRequest,
    ModelSelectionOutcome, OutboundHttpRequest, PersistedModelSelection, RegistrationBinding,
    RequestedModelWork, ResolvedModelSelection, SupportApiOrigin, SupportApiTransport,
    TenantPolicyBinding, VerifiedModelCatalog,
};
use desktop_egress::{
    local_egress_manifest_hash_v2, ApproveDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextCandidate, ContextClassification, ContextPreparer, EgressLimits,
    LocalEgressManifestV2Input, LocalEgressManifestV2Item, MemoryDecisionLedger,
    ModelInvocationBindingDraft, ModelInvocationBindingV2, ModelRequestMode,
    ModelSelectionBindingV2, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use serde_json::{json, Value};
use zeroize::Zeroizing;

/// Named so the transplant table below stays inside the complexity budget.
type SelectionMutation = (&'static str, Box<dyn Fn(&mut ModelSelectionBindingV2)>);

const CONTENT: &str = "safe context";
const PROFILE_ID: &str = "fixture-mid";
const VARIANT_ID: &str = "balanced";
const CATALOG_VERSION: &str = "v7.00112233445566aa";

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn catalog_hash() -> Sha256Digest {
    Sha256Digest::parse(&format!("sha256:{}", "1".repeat(64))).expect("catalog hash")
}

struct FakeInstallationSigner;

impl InstallationConsentSigner for FakeInstallationSigner {
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "the trait signature fixes the return lifetime"
    )]
    fn key_id(&self) -> &str {
        "installation-key-1"
    }

    fn sign(&self, signed_payload_hash: &Sha256Digest) -> Result<String, CloudError> {
        Ok(format!("sig_{}", signed_payload_hash.hex_value()))
    }
}

struct Fixture {
    request: AuthorizedModelRequest,
    selection: ModelInvocationBindingV2,
    consumption_hash: Sha256Digest,
}

/// Builds one complete authorized request plus the sealed version two selection
/// that authorizes it. Each call uses a fresh ledger so the single-use decision
/// authority is exercised honestly rather than shared.
fn fixture(mutate: impl FnOnce(&mut ModelSelectionBindingV2)) -> Fixture {
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_01J0000000000000"),
            project_ref: id("project_01J0000000000000"),
            run_ref: id("run_01J0000000000000"),
            purpose: "bmad_help".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("sapphirus.bmad-method-help-proposal.v1"),
            canonical_output_schema_hash: sha256_bytes(b"schema"),
            provider_profile_hash: sha256_bytes(b"provider-profile"),
            model_profile_hash: sha256_bytes(b"model-profile"),
            deployment_hash: sha256_bytes(b"deployment"),
            policy_hash: sha256_bytes(b"policy"),
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(61_000),
            limits: EgressLimits {
                maximum_context_items: 8,
                maximum_context_bytes: 64 * 1024,
                maximum_token_estimate: 16_000,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_01J0000000000000"),
                relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
                semantic_role: "source".to_owned(),
                language: Some("en".to_owned()),
                classification: ContextClassification::Internal,
                content: CONTENT.to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("manifest");
    let binding = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_01J0000000000000"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("install_01J0000000000000"),
        session_authority_hash: sha256_bytes(b"session-authority"),
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
        consent_disclosure_hash: sha256_bytes(b"consent-disclosure-v1"),
    }
    .seal()
    .expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_01J0000000000000"),
            issued_at: UnixMillis(1_500),
            expires_at: UnixMillis(31_500),
        })
        .expect("decision");
    let consumption = service
        .consume(ConsumeDecisionInput {
            decision: &decision,
            binding: &binding,
            invocation_id: id("invoke_01J0000000000000"),
            consumed_at: UnixMillis(2_000),
        })
        .expect("consumption");

    let mut facts = ModelSelectionBindingV2 {
        request_mode: ModelRequestMode::StreamingChat,
        model_profile_id: PROFILE_ID.to_owned(),
        requested_variant_id: Some(VARIANT_ID.to_owned()),
        catalog_version: CATALOG_VERSION.to_owned(),
        catalog_hash: catalog_hash(),
        model_profile_hash: sha256_bytes(b"model-profile"),
        model_capability_hash: sha256_bytes(b"capability"),
        context_snapshot_id: id("snapshot_01J0000000000000"),
        budget_class: "interactive-standard".to_owned(),
        local_egress_manifest_hash: sha256_bytes(b"placeholder"),
    };
    mutate(&mut facts);
    facts.local_egress_manifest_hash = expected_manifest_hash(&facts);
    let selection = facts.seal(&binding).expect("sealed selection");
    let consumption_hash = selection
        .consumption_hash_for(&consumption)
        .expect("version two consumption hash");
    let request =
        AuthorizedModelRequest::new(&manifest, &binding, consumption).expect("authorized request");
    Fixture {
        request,
        selection,
        consumption_hash,
    }
}

/// The version two manifest hash the projection will recompute, built here from
/// the same facts so the test proves agreement rather than restating a constant.
fn expected_manifest_hash(facts: &ModelSelectionBindingV2) -> Sha256Digest {
    local_egress_manifest_hash_v2(&LocalEgressManifestV2Input {
        request_id: id("request_01J0000000000000"),
        registration_id: id("dreg_01J0000000000000"),
        purpose: "bmad_help".to_owned(),
        model_role: "planner".to_owned(),
        request_mode: facts.request_mode,
        model_profile_id: facts.model_profile_id.clone(),
        requested_variant_id: facts.requested_variant_id.clone(),
        catalog_version: facts.catalog_version.clone(),
        model_profile_hash: facts.model_profile_hash,
        model_capability_hash: facts.model_capability_hash,
        context_snapshot_id: facts.context_snapshot_id.clone(),
        canonical_output_schema_id: (facts.request_mode == ModelRequestMode::StructuredResponse)
            .then(|| id("sapphirus.bmad-method-help-proposal.v1")),
        canonical_output_schema_hash: (facts.request_mode
            == ModelRequestMode::StructuredResponse)
            .then(|| sha256_bytes(b"schema")),
        retention_mode: RetentionMode::TransientNoStore,
        budget_class: facts.budget_class.clone(),
        items: vec![LocalEgressManifestV2Item {
            client_item_id: id("item_01J0000000000000"),
            relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
            semantic_role: "source".to_owned(),
            language: "en".to_owned(),
            content_hash: sha256_bytes(CONTENT.as_bytes()),
            byte_count: CONTENT.len() as u64,
            classification: ContextClassification::Internal,
        }],
        tools: Vec::new(),
    })
    .expect("expected manifest hash")
}

fn registration() -> RegistrationBinding {
    RegistrationBinding {
        registration_id: id("dreg_01J0000000000000"),
        installation_public_key_hash: sha256_bytes(b"installation-public-key"),
    }
}

fn lease() -> LeaseBinding {
    LeaseBinding {
        lease_id: id("lease_01J0000000000000"),
        lease_hash: sha256_bytes(b"lease"),
    }
}

fn policy() -> TenantPolicyBinding {
    TenantPolicyBinding {
        policy_id: id("policy_01J0000000000000"),
        policy_version: NonZeroU64::new(7).expect("non-zero policy version"),
        policy_hash: sha256_bytes(b"policy"),
    }
}

fn subject() -> ConsentSubject {
    ConsentSubject {
        tenant_hash: sha256_bytes(b"tenant"),
        subject_hash: sha256_bytes(b"subject"),
    }
}

fn window() -> ConsentWindow {
    ConsentWindow {
        issued_at: UnixMillis(1_000),
        not_before: UnixMillis(1_000),
        expires_at: UnixMillis(301_000),
        nonce_hash: sha256_bytes(b"nonce"),
    }
}

fn project(
    fixture: Fixture,
    tools: &[DeclaredModelTool],
) -> Result<desktop_cloud::CanonicalModelAccessRequestV2, CloudError> {
    let registration = registration();
    let lease = lease();
    let policy = policy();
    let subject = subject();
    let window = window();
    project_model_access_request_v2(
        fixture.request,
        &CanonicalProjectionInputsV2 {
            registration: &registration,
            lease: &lease,
            policy: &policy,
            subject: &subject,
            window: &window,
            selection: &fixture.selection,
            consumption_hash: fixture.consumption_hash,
            tools,
            signer: &FakeInstallationSigner,
        },
    )
}

#[test]
fn the_version_two_request_carries_the_selection_and_no_azure_authority() {
    let projected = project(fixture(|_| {}), &[]).expect("a projected version two request");
    let value: Value = serde_json::to_value(&projected).expect("serializable");

    assert_eq!(value["schemaVersion"], "desktop-model-access-request.v2");
    assert_eq!(value["requestMode"], "streaming_chat");
    assert_eq!(value["modelProfileId"], PROFILE_ID);
    assert_eq!(value["requestedVariantId"], VARIANT_ID);
    assert_eq!(value["catalogVersion"], CATALOG_VERSION);
    assert_eq!(
        value["consent"]["schemaVersion"],
        "sapphirus.model-context-consent.v2"
    );
    // Streaming chat produces no structured output, so the schema pair is
    // absent -- written as null rather than omitted.
    assert!(value["canonicalOutputSchemaId"].is_null());
    assert!(value["consent"]["canonicalOutputSchemaId"].is_null());

    // The exact outbound bytes stay visible in the request that the review
    // surface was built from.
    assert_eq!(value["items"][0]["content"], CONTENT);
    assert_eq!(value["items"][0]["byteCount"], CONTENT.len());
    assert_eq!(value["items"][0]["classification"], "internal");

    // Nothing on the wire can carry Azure deployment or project authority.
    let rendered = serde_json::to_string(&value).expect("rendered");
    for forbidden in [
        "deploymentHash",
        "deploymentName",
        "projectEndpoint",
        "apiVersion",
        "subscriptionId",
        "azure",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "the version two request must not carry {forbidden}",
        );
    }
}

/// The manifest hash on the wire is the one the projection recomputed. A
/// selection sealed with any other value cannot be projected at all, which is
/// what makes the support plane's recomputation meaningful.
#[test]
fn the_manifest_hash_is_recomputed_rather_than_trusted() {
    let projected = project(fixture(|_| {}), &[]).expect("a projected request");
    let value: Value = serde_json::to_value(&projected).expect("serializable");
    let expected = expected_manifest_hash(&ModelSelectionBindingV2 {
        request_mode: ModelRequestMode::StreamingChat,
        model_profile_id: PROFILE_ID.to_owned(),
        requested_variant_id: Some(VARIANT_ID.to_owned()),
        catalog_version: CATALOG_VERSION.to_owned(),
        catalog_hash: catalog_hash(),
        model_profile_hash: sha256_bytes(b"model-profile"),
        model_capability_hash: sha256_bytes(b"capability"),
        context_snapshot_id: id("snapshot_01J0000000000000"),
        budget_class: "interactive-standard".to_owned(),
        local_egress_manifest_hash: sha256_bytes(b"unused"),
    });
    assert_eq!(value["localEgressManifestHash"], expected.to_string());
    assert_eq!(value["consent"]["manifestHash"], expected.to_string());

    // A selection whose sealed manifest hash does not describe the context that
    // is actually being sent is refused, not silently corrected.
    let mut tampered = fixture(|_| {});
    let stale = ModelSelectionBindingV2 {
        request_mode: ModelRequestMode::StreamingChat,
        model_profile_id: PROFILE_ID.to_owned(),
        requested_variant_id: Some(VARIANT_ID.to_owned()),
        catalog_version: CATALOG_VERSION.to_owned(),
        catalog_hash: catalog_hash(),
        model_profile_hash: sha256_bytes(b"model-profile"),
        model_capability_hash: sha256_bytes(b"capability"),
        context_snapshot_id: id("snapshot_01J0000000000000"),
        budget_class: "interactive-standard".to_owned(),
        local_egress_manifest_hash: sha256_bytes(b"stale context"),
    };
    // Re-seal against a fresh binding built the same way, so only the manifest
    // hash differs.
    let replacement = fixture(|_| {});
    tampered.selection = stale
        .seal(&fresh_binding())
        .expect("sealed against a fresh binding");
    tampered.consumption_hash = replacement.consumption_hash;
    assert_eq!(
        project(tampered, &[]).err(),
        Some(CloudError::CanonicalProjectionInvalid),
    );
}

fn fresh_binding() -> desktop_egress::ModelInvocationBinding {
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_01J0000000000000"),
            project_ref: id("project_01J0000000000000"),
            run_ref: id("run_01J0000000000000"),
            purpose: "bmad_help".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("sapphirus.bmad-method-help-proposal.v1"),
            canonical_output_schema_hash: sha256_bytes(b"schema"),
            provider_profile_hash: sha256_bytes(b"provider-profile"),
            model_profile_hash: sha256_bytes(b"model-profile"),
            deployment_hash: sha256_bytes(b"deployment"),
            policy_hash: sha256_bytes(b"policy"),
            region: "westeurope".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(61_000),
            limits: EgressLimits {
                maximum_context_items: 8,
                maximum_context_bytes: 64 * 1024,
                maximum_token_estimate: 16_000,
            },
            candidates: vec![ContextCandidate {
                client_item_id: id("item_01J0000000000000"),
                relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
                semantic_role: "source".to_owned(),
                language: Some("en".to_owned()),
                classification: ContextClassification::Internal,
                content: CONTENT.to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("manifest");
    ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id: id("request_01J0000000000000"),
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: id("install_01J0000000000000"),
        session_authority_hash: sha256_bytes(b"session-authority"),
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
        consent_disclosure_hash: sha256_bytes(b"consent-disclosure-v1"),
    }
    .seal()
    .expect("binding")
}

/// Profile, catalog, and variant transplant: the same reviewed context under a
/// different model is a different signed decision.
#[test]
fn a_profile_catalog_or_variant_transplant_changes_the_signed_envelope() {
    let baseline: Value =
        serde_json::to_value(project(fixture(|_| {}), &[]).expect("baseline")).expect("value");
    let baseline_envelope = baseline["consent"]["consentEnvelopeHash"]
        .as_str()
        .expect("envelope hash")
        .to_owned();

    let transplants: Vec<SelectionMutation> = vec![
        (
            "profile",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.model_profile_id = "fixture-alpha".to_owned();
            }),
        ),
        (
            "catalog epoch",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.catalog_version = "v8.00112233445566aa".to_owned();
            }),
        ),
        (
            "variant",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.requested_variant_id = Some("thorough".to_owned());
            }),
        ),
        (
            "capability set",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.model_capability_hash = sha256_bytes(b"widened capability");
            }),
        ),
        (
            "context snapshot",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.context_snapshot_id = id("snapshot_01J0000000000009");
            }),
        ),
        (
            "request mode",
            Box::new(|facts: &mut ModelSelectionBindingV2| {
                facts.request_mode = ModelRequestMode::StructuredResponse;
            }),
        ),
    ];
    for (name, mutate) in transplants {
        let projected: Value = serde_json::to_value(
            project(fixture(|facts| mutate(facts)), &[]).expect("a projected transplant"),
        )
        .expect("value");
        assert_ne!(
            projected["consent"]["consentEnvelopeHash"]
                .as_str()
                .expect("envelope hash"),
            baseline_envelope,
            "{name} did not change the signed consent envelope",
        );
    }
}

/// A structured request carries both halves of the output schema pair; a
/// streaming request carries neither.
#[test]
fn the_output_schema_pair_follows_the_request_mode() {
    let structured: Value = serde_json::to_value(
        project(
            fixture(|facts| facts.request_mode = ModelRequestMode::StructuredResponse),
            &[],
        )
        .expect("structured"),
    )
    .expect("value");
    assert_eq!(
        structured["canonicalOutputSchemaId"],
        "sapphirus.bmad-method-help-proposal.v1"
    );
    assert!(structured["canonicalOutputSchemaHash"].is_string());
    assert_eq!(structured["requestMode"], "structured_response");
}

// --- capability validation, before review is shown -------------------------

struct ScriptedExecutor {
    responses: Mutex<VecDeque<HttpResponse>>,
}

#[async_trait]
impl HttpExecutor for ScriptedExecutor {
    async fn execute(&self, _request: OutboundHttpRequest) -> Result<HttpResponse, CloudError> {
        self.responses
            .lock()
            .expect("response lock")
            .pop_front()
            .ok_or(CloudError::TransportFailed)
    }
}

fn profile_value(profile_id: &str, capabilities: &Value, limits: &Value) -> Value {
    json!({
        "profileId": profile_id,
        "displayName": "Fixture Profile",
        "description": "A synthetic fixture profile. It names no deployment.",
        "family": null,
        "provider": "microsoft_foundry",
        "availability": "available",
        "statusDetail": null,
        "capabilities": capabilities,
        "limits": limits,
        "variants": [{
            "variantId": VARIANT_ID,
            "displayName": "Balanced",
            "reasoningEffort": "medium",
            "maxOutputTokens": 8_000
        }],
        "defaultVariantId": VARIANT_ID,
        "qualifiedAt": "2026-07-28T00:00:00.000Z"
    })
}

fn text_only_capabilities() -> Value {
    json!({
        "textInput": true,
        "imageInput": false,
        "pdfInput": false,
        "attachments": false,
        "toolCalling": false,
        "structuredOutput": false,
        "reasoning": true
    })
}

fn tight_limits() -> Value {
    json!({
        "contextTokens": 100,
        "maxInputTokens": 100,
        "maxOutputTokens": 4_000,
        "maxAttachmentBytes": null
    })
}

async fn verified_catalog(default_profile_id: &str, profiles: Vec<Value>) -> VerifiedModelCatalog {
    let body = serde_json::to_string(&json!({
        "schemaVersion": "sapphirus.model-catalog.v1",
        "catalogVersion": CATALOG_VERSION,
        "catalogHash": format!("sha256:{}", "1".repeat(64)),
        "catalogState": "fresh",
        "policyVersion": 3,
        "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "deploymentSnapshotHash":
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        "generatedAt": "2026-07-28T00:00:00.000Z",
        "expiresAt": "2026-07-28T00:05:00.000Z",
        "defaultProfileId": default_profile_id,
        "profiles": profiles,
        "unavailableProfiles": []
    }))
    .expect("fixture catalog serializes");
    let transport = SupportApiTransport::new(
        SupportApiOrigin::new("https://support.example.test").expect("origin"),
        ScriptedExecutor {
            responses: Mutex::new(VecDeque::from(vec![HttpResponse::new(
                200,
                Some("application/json".to_owned()),
                Some(body.len() as u64),
                body.as_bytes().to_vec(),
            )])),
        },
    );
    transport
        .fetch_model_catalog(
            Zeroizing::new("token".to_owned()),
            &ModelCatalogRequest {
                subject_hash: "sha256:fixture-subject",
                cached: None,
                idempotency_key: "capability-fixture",
            },
            UnixMillis(1),
        )
        .await
        .expect("fixture catalog verifies")
        .cached()
        .catalog()
        .clone()
}

fn selection_for(catalog: &VerifiedModelCatalog) -> ResolvedModelSelection {
    match resolve_model_selection(
        catalog,
        Some(&PersistedModelSelection {
            profile_id: PROFILE_ID.to_owned(),
            variant_id: VARIANT_ID.to_owned(),
            catalog_version: CATALOG_VERSION.to_owned(),
            catalog_hash: format!("sha256:{}", "1".repeat(64)),
            selected_at: UnixMillis(10),
        }),
        &[],
    ) {
        ModelSelectionOutcome::Selected { selection, .. } => selection,
        other => panic!("the fixture selection must resolve, got {other:?}"),
    }
}

fn modest_work(tools: &[DeclaredModelTool]) -> RequestedModelWork<'_> {
    RequestedModelWork {
        request_mode: ModelRequestMode::StreamingChat,
        context_item_count: 1,
        total_outbound_bytes: 40,
        total_token_estimate: 10,
        attachment_bytes: 0,
        uses_image_input: false,
        uses_pdf_input: false,
        tools,
        requires_structured_output: false,
        requested_output_tokens: None,
    }
}

#[tokio::test]
async fn capabilities_are_checked_against_the_profile_before_review() {
    let catalog =
        verified_catalog(PROFILE_ID, vec![profile_value(PROFILE_ID, &text_only_capabilities(), &tight_limits())])
            .await;
    let selection = selection_for(&catalog);
    let no_tools: [DeclaredModelTool; 0] = [];
    validate_model_capabilities(&catalog, &selection, &modest_work(&no_tools))
        .expect("plain text within limits is accepted");

    let tool = [DeclaredModelTool {
        tool_name: "search_workspace".to_owned(),
        description: "Find files".to_owned(),
        parameters_schema_id: id("sapphirus.tool-search"),
        parameters_schema_hash: sha256_bytes(b"tool schema"),
    }];
    let cases: Vec<(RequestedModelWork<'_>, ModelCapabilityRejection)> = vec![
        (
            RequestedModelWork {
                uses_image_input: true,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::ImageInputUnsupported,
        ),
        (
            RequestedModelWork {
                uses_pdf_input: true,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::PdfInputUnsupported,
        ),
        (
            RequestedModelWork {
                attachment_bytes: 1_024,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::AttachmentsUnsupported,
        ),
        (
            modest_work(&tool),
            ModelCapabilityRejection::ToolCallingUnsupported,
        ),
        (
            RequestedModelWork {
                requires_structured_output: true,
                request_mode: ModelRequestMode::StructuredResponse,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::StructuredOutputUnsupported,
        ),
        (
            RequestedModelWork {
                total_token_estimate: 10_000,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::ContextTokensExceeded,
        ),
        (
            RequestedModelWork {
                requested_output_tokens: NonZeroU64::new(100_000),
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::OutputTokensExceeded,
        ),
        (
            RequestedModelWork {
                context_item_count: 0,
                total_outbound_bytes: 0,
                ..modest_work(&no_tools)
            },
            ModelCapabilityRejection::NoContext,
        ),
    ];
    for (work, expected) in cases {
        assert_eq!(
            validate_model_capabilities(&catalog, &selection, &work),
            Err(expected),
            "the profile must refuse this request before review",
        );
    }
}

/// A selection made against a different catalog cannot be validated against this
/// one, and a profile that is no longer selectable is refused by identity rather
/// than by capability.
#[tokio::test]
async fn a_stale_selection_is_refused_rather_than_reinterpreted() {
    let catalog =
        verified_catalog(PROFILE_ID, vec![profile_value(PROFILE_ID, &text_only_capabilities(), &tight_limits())])
            .await;
    let selection = selection_for(&catalog);
    let no_tools: [DeclaredModelTool; 0] = [];

    let newer = verified_catalog("fixture-alpha", vec![profile_value(
        "fixture-alpha",
        &text_only_capabilities(),
        &tight_limits(),
    )])
    .await;
    assert_eq!(
        validate_model_capabilities(&newer, &selection, &modest_work(&no_tools)),
        Err(ModelCapabilityRejection::ProfileNotSelectable),
    );
    let _ = catalog;
}

/// Streaming chat produces plain text. A request that both streams and demands a
/// canonical output schema is asking for two different results.
#[tokio::test]
async fn streaming_chat_may_not_demand_a_structured_output_schema() {
    let capable = json!({
        "textInput": true,
        "imageInput": false,
        "pdfInput": false,
        "attachments": false,
        "toolCalling": true,
        "structuredOutput": true,
        "reasoning": true
    });
    let catalog =
        verified_catalog(PROFILE_ID, vec![profile_value(PROFILE_ID, &capable, &tight_limits())]).await;
    let selection = selection_for(&catalog);
    let no_tools: [DeclaredModelTool; 0] = [];
    assert_eq!(
        validate_model_capabilities(
            &catalog,
            &selection,
            &RequestedModelWork {
                requires_structured_output: true,
                request_mode: ModelRequestMode::StreamingChat,
                ..modest_work(&no_tools)
            },
        ),
        Err(ModelCapabilityRejection::RequestModeConflict),
    );

    // A duplicated or empty tool name is a malformed declaration, not a
    // capability question.
    let duplicated = [
        DeclaredModelTool {
            tool_name: "search".to_owned(),
            description: "Find".to_owned(),
            parameters_schema_id: id("sapphirus.tool-search"),
            parameters_schema_hash: sha256_bytes(b"a"),
        },
        DeclaredModelTool {
            tool_name: "search".to_owned(),
            description: "Find again".to_owned(),
            parameters_schema_id: id("sapphirus.tool-search"),
            parameters_schema_hash: sha256_bytes(b"b"),
        },
    ];
    assert_eq!(
        validate_model_capabilities(&catalog, &selection, &modest_work(&duplicated)),
        Err(ModelCapabilityRejection::ToolSetInvalid),
    );
}
