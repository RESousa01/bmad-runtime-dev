//! Host composition for the ADR-0005 capability runs (readiness Task 7).
//!
//! Maps every reviewed menu capability to its sealed instruction
//! projection and output archetype, assembles the exact reviewed context
//! through governed workspace reads, and drives the generic
//! [`BmadCapabilityCoordinator`] lifecycle. Model output is parsed from
//! untrusted JSON into the sealed archetype constructors and rejected on
//! any drift.

use desktop_cloud::{AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput};
use desktop_egress::{
    ContextCandidate, ContextClassification, ContextPreparer, EgressLimits,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput,
};
use desktop_runtime::{
    canonical_hash, sha256_bytes, BmadBuilderDraftFile, BmadBuilderDraftKind, BmadCandidateChange,
    BmadCapabilityOutput, BmadClosureCapabilityId, BmadDocumentArtifact, BmadDocumentSection,
    BmadGovernedChangeSet, BmadInactiveBuilderDraft, ContractId, RelativeWorkspacePath,
    Sha256Digest, UnixMillis, BMAD_DOCUMENT_ARTIFACT_SCHEMA, BMAD_GOVERNED_CHANGE_SET_SCHEMA,
    BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
};
use serde::Serialize;
use serde_json::Value;

use crate::bmad_model::capability_coordinator::{
    capability_purpose, ApproveCapabilityRunInput, BmadCapabilityCoordinatorError,
    BmadCapabilityOutputVerifier, BmadCapabilityTransport, CancelCapabilityRunInput,
    PrepareCapabilityRunInput, SubmitCapabilityRunInput,
};
use crate::bmad_model::config::{current_help_model_configuration, HelpModelMode};
use crate::bmad_model::context::derived_contract_id;

/// One reviewed menu capability binding: identity, archetype, projection.
pub(crate) struct CapabilityBinding {
    pub id: &'static str,
    pub output_schema_id: &'static str,
    pub projection_path: &'static str,
}

/// The 24 unique reviewed menu capabilities (ADR-0005). Builder authoring
/// operations join in the Task 8 lane.
pub(crate) const CAPABILITY_TABLE: [CapabilityBinding; 24] = [
    CapabilityBinding {
        id: "bmm:bmad-brainstorming",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/brainstorming.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-market-research",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/market-research.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-domain-research",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/domain-research.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-technical-research",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/technical-research.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-product-brief",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/product-brief.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-prfaq",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/prfaq.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-document-project",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/document-project.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:tech-writer-write-document",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/write-document.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:tech-writer-mermaid-gen",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/mermaid-gen.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:tech-writer-validate-doc",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/validate-doc.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:tech-writer-explain-concept",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/explain-concept.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-prd",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/prd.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-create-epics-and-stories",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/create-epics-and-stories.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-check-implementation-readiness",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/implementation-readiness.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-correct-course",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/correct-course.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-ux",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/ux-design.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-architecture",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/architecture-create.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-dev-story",
        output_schema_id: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        projection_path: "runtime/method/6.10.0/dev-story.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-quick-dev",
        output_schema_id: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        projection_path: "runtime/method/6.10.0/quick-dev.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-qa-generate-e2e-tests",
        output_schema_id: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        projection_path: "runtime/method/6.10.0/qa-tests.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-code-review",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/code-review.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-sprint-planning",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/sprint-planning.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-create-story",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/create-story.instructions.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-retrospective",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "runtime/method/6.10.0/retrospective.instructions.md",
    },
];

pub(crate) fn capability_binding(capability_id: &str) -> Option<&'static CapabilityBinding> {
    CAPABILITY_TABLE
        .iter()
        .find(|binding| binding.id == capability_id)
}

/// Parses one untrusted wire capability result into the sealed archetype
/// constructors. Every bound is re-enforced by the constructors; anything
/// outside the closed shape is rejected.
pub(crate) fn parse_capability_result(
    value: &Value,
) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
    let object = value
        .as_object()
        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
    let result_kind = object
        .get("resultKind")
        .and_then(Value::as_str)
        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
    match result_kind {
        "document_artifact" => {
            let artifact = object
                .get("documentArtifact")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
            if artifact.get("schemaVersion").and_then(Value::as_str)
                != Some(BMAD_DOCUMENT_ARTIFACT_SCHEMA)
            {
                return Err(BmadCapabilityCoordinatorError::OutputRejected);
            }
            let sections = artifact
                .get("sections")
                .and_then(Value::as_array)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?
                .iter()
                .map(|section| {
                    let section = section
                        .as_object()
                        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
                    Ok(BmadDocumentSection {
                        heading: required_string(section.get("heading"))?,
                        body: required_string(section.get("body"))?,
                    })
                })
                .collect::<Result<Vec<_>, BmadCapabilityCoordinatorError>>()?;
            let evidence_refs = optional_string_array(artifact.get("evidenceRefs"))?
                .into_iter()
                .map(|value| {
                    ContractId::new(value)
                        .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let open_questions = optional_string_array(artifact.get("openQuestions"))?;
            let mermaid_text = match artifact.get("mermaidText") {
                None | Some(Value::Null) => None,
                Some(Value::String(text)) => Some(text.clone()),
                Some(_) => return Err(BmadCapabilityCoordinatorError::OutputRejected),
            };
            BmadDocumentArtifact::new(
                required_string(artifact.get("title"))?,
                sections,
                evidence_refs,
                open_questions,
                mermaid_text,
            )
            .map(BmadCapabilityOutput::DocumentArtifact)
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
        }
        "governed_change_set" => {
            let change_set = object
                .get("governedChangeSet")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
            if change_set.get("schemaVersion").and_then(Value::as_str)
                != Some(BMAD_GOVERNED_CHANGE_SET_SCHEMA)
            {
                return Err(BmadCapabilityCoordinatorError::OutputRejected);
            }
            let changes = change_set
                .get("changes")
                .and_then(Value::as_array)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?
                .iter()
                .map(parse_candidate_change)
                .collect::<Result<Vec<_>, _>>()?;
            BmadGovernedChangeSet::new(required_string(change_set.get("summary"))?, changes)
                .map(BmadCapabilityOutput::GovernedChangeSet)
                .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
        }
        "inactive_builder_draft" => {
            let draft = object
                .get("inactiveBuilderDraft")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
            if draft.get("schemaVersion").and_then(Value::as_str)
                != Some(BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA)
            {
                return Err(BmadCapabilityCoordinatorError::OutputRejected);
            }
            let draft_kind = match draft.get("draftKind").and_then(Value::as_str) {
                Some("agent") => BmadBuilderDraftKind::Agent,
                Some("workflow") => BmadBuilderDraftKind::Workflow,
                _ => return Err(BmadCapabilityCoordinatorError::OutputRejected),
            };
            let files = draft
                .get("files")
                .and_then(Value::as_array)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?
                .iter()
                .map(|file| {
                    let file = file
                        .as_object()
                        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
                    Ok(BmadBuilderDraftFile {
                        path: required_relative_path(file.get("path"))?,
                        content: required_string(file.get("content"))?,
                    })
                })
                .collect::<Result<Vec<_>, BmadCapabilityCoordinatorError>>()?;
            BmadInactiveBuilderDraft::new(
                draft_kind,
                required_string(draft.get("title"))?,
                required_string(draft.get("revisionNote"))?,
                files,
            )
            .map(BmadCapabilityOutput::InactiveBuilderDraft)
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
        }
        _ => Err(BmadCapabilityCoordinatorError::OutputRejected),
    }
}

fn required_string(value: Option<&Value>) -> Result<String, BmadCapabilityCoordinatorError> {
    value
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)
}

fn required_relative_path(
    value: Option<&Value>,
) -> Result<RelativeWorkspacePath, BmadCapabilityCoordinatorError> {
    RelativeWorkspacePath::new(
        value
            .and_then(Value::as_str)
            .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?,
    )
    .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
}

fn optional_string_array(
    value: Option<&Value>,
) -> Result<Vec<String>, BmadCapabilityCoordinatorError> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or(BmadCapabilityCoordinatorError::OutputRejected)
            })
            .collect(),
        Some(_) => Err(BmadCapabilityCoordinatorError::OutputRejected),
    }
}

fn parse_candidate_change(
    value: &Value,
) -> Result<BmadCandidateChange, BmadCapabilityCoordinatorError> {
    let change = value
        .as_object()
        .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?;
    let path = required_relative_path(change.get("path"))?;
    match change.get("operation").and_then(Value::as_str) {
        Some("create") => Ok(BmadCandidateChange::Create {
            path,
            content: required_string(change.get("content"))?,
        }),
        Some("replace") => Ok(BmadCandidateChange::Replace {
            path,
            content: required_string(change.get("content"))?,
            preimage_sha256: required_digest(change.get("preimageSha256"))?,
        }),
        Some("delete") => Ok(BmadCandidateChange::Delete {
            path,
            preimage_sha256: required_digest(change.get("preimageSha256"))?,
        }),
        _ => Err(BmadCapabilityCoordinatorError::OutputRejected),
    }
}

fn required_digest(value: Option<&Value>) -> Result<Sha256Digest, BmadCapabilityCoordinatorError> {
    Sha256Digest::parse(
        value
            .and_then(Value::as_str)
            .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?,
    )
    .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)
}

/// Wire-shape verifier: untrusted model JSON in, sealed archetype out.
pub(crate) struct WireCapabilityOutputVerifier;

impl BmadCapabilityOutputVerifier for WireCapabilityOutputVerifier {
    fn verify(
        &self,
        _capability_id: &BmadClosureCapabilityId,
        output: &RawModelOutput,
    ) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
        let value: Value = serde_json::from_str(&output.payload_json)
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)?;
        parse_capability_result(&value)
    }
}

/// Deterministic fixture: one honest, clearly labeled result of the
/// capability's archetype, used only in the deterministic composition.
pub(crate) fn deterministic_fixture_for(binding: &CapabilityBinding) -> String {
    match binding.output_schema_id {
        BMAD_GOVERNED_CHANGE_SET_SCHEMA => serde_json::json!({
            "resultKind": "governed_change_set",
            "governedChangeSet": {
                "schemaVersion": BMAD_GOVERNED_CHANGE_SET_SCHEMA,
                "summary": format!(
                    "Deterministic candidate change set for {} (no external model involved).",
                    binding.id
                ),
                "changes": [{
                    "operation": "create",
                    "path": "bmad/proposals/deterministic-preview.md",
                    "content": "# Deterministic capability preview\n\nThis candidate was produced by the deterministic composition; it requires governed review before any effect.\n",
                }],
            },
        })
        .to_string(),
        _ => serde_json::json!({
            "resultKind": "document_artifact",
            "documentArtifact": {
                "schemaVersion": BMAD_DOCUMENT_ARTIFACT_SCHEMA,
                "title": format!("Deterministic preview: {}", binding.id),
                "sections": [{
                    "heading": "Deterministic composition",
                    "body": "This artifact was produced locally by the deterministic composition; no external model was contacted.",
                }],
                "evidenceRefs": [],
                "openQuestions": [],
            },
        })
        .to_string(),
    }
}

/// Offline transport: production posture until the deployed round trip
/// is activated (readiness Task 9); never degrades silently.
pub(crate) struct OfflineCapabilityTransport;

impl BmadCapabilityTransport for OfflineCapabilityTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        _deterministic_fixture: &str,
        _now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        desktop_cloud::OfflineModelTransport.send(request)
    }
}

#[cfg(feature = "deterministic-help")]
pub(crate) struct DeterministicCapabilityTransport;

#[cfg(feature = "deterministic-help")]
impl BmadCapabilityTransport for DeterministicCapabilityTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        desktop_cloud::DeterministicModelTransport.send_fixture(
            request,
            deterministic_fixture.to_owned(),
            now,
        )
    }
}

pub(crate) fn active_capability_transport() -> Box<dyn BmadCapabilityTransport> {
    #[cfg(feature = "deterministic-help")]
    {
        Box::new(DeterministicCapabilityTransport)
    }
    #[cfg(not(feature = "deterministic-help"))]
    {
        Box::new(OfflineCapabilityTransport)
    }
}

#[derive(Serialize)]
struct CapabilityRunIdentity<'a> {
    schema_version: &'static str,
    capability_id: &'a str,
    workspace_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    created_at: UnixMillis,
}

#[derive(Serialize)]
struct CapabilitySessionIdentity<'a> {
    schema_version: &'static str,
    renderer_session_id: &'a ContractId,
    workspace_id: &'a ContractId,
    workspace_grant_epoch: u64,
    workspace_context_read_epoch: u64,
}

pub(crate) struct AssembleCapabilityPrepareInput<'a> {
    pub binding: &'static CapabilityBinding,
    pub instruction_bytes: &'a [u8],
    pub renderer_session_id: &'a ContractId,
    pub installation_id: &'a ContractId,
    pub project_id: &'a ContractId,
    pub workspace_id: ContractId,
    pub workspace_grant_epoch: u64,
    pub workspace_context_read_epoch: u64,
    pub candidates: Vec<ContextCandidate>,
    pub created_at: UnixMillis,
}

const CAPABILITY_MANIFEST_LIFETIME_MS: u64 = 10 * 60 * 1_000;
const MAX_CAPABILITY_CONTEXT_ITEMS: u32 = 100;
const MAX_CAPABILITY_CONTEXT_BYTES: u64 = 256 * 1024;

/// Assembles the sealed manifest, invocation binding, and coordinator
/// prepare input for one capability run.
///
/// # Errors
///
/// Fails closed when the model configuration is offline, the manifest
/// cannot seal, or any binding identity cannot be derived.
pub(crate) fn assemble_capability_prepare(
    input: AssembleCapabilityPrepareInput<'_>,
) -> Result<PrepareCapabilityRunInput, BmadCapabilityCoordinatorError> {
    let configuration = current_help_model_configuration()
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    if configuration.mode == HelpModelMode::Offline {
        return Err(BmadCapabilityCoordinatorError::Transport);
    }
    let capability_id = BmadClosureCapabilityId::new(input.binding.id)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let purpose = capability_purpose(&capability_id);
    let output_schema_contract = ContractId::new(input.binding.output_schema_id)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let output_schema_hash = sha256_bytes(input.binding.output_schema_id.as_bytes());
    let instruction_hash = sha256_bytes(input.instruction_bytes);

    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: input.installation_id.clone(),
            project_ref: input.project_id.clone(),
            run_ref: input.workspace_id.clone(),
            purpose: purpose.clone(),
            model_role: "capability".to_owned(),
            canonical_output_schema_id: output_schema_contract.clone(),
            canonical_output_schema_hash: output_schema_hash,
            provider_profile_hash: configuration.provider_profile_hash,
            // The trusted-profile binding hash covers the exact reviewed
            // model profile; capabilities bind to it as one digest.
            model_profile_hash: configuration.trusted_profile.model_binding_hash(),
            deployment_hash: configuration.deployment_hash,
            policy_hash: configuration.policy_hash,
            region: configuration.region.to_owned(),
            retention_mode: configuration.retention_mode,
            created_at: input.created_at,
            expires_at: UnixMillis(
                input
                    .created_at
                    .0
                    .checked_add(CAPABILITY_MANIFEST_LIFETIME_MS)
                    .ok_or(BmadCapabilityCoordinatorError::Integrity)?,
            ),
            limits: EgressLimits {
                maximum_context_items: MAX_CAPABILITY_CONTEXT_ITEMS,
                maximum_context_bytes: MAX_CAPABILITY_CONTEXT_BYTES,
                maximum_token_estimate: MAX_CAPABILITY_CONTEXT_BYTES,
            },
            candidates: input.candidates,
            exclusions: Vec::new(),
        })
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;

    let session_authority_hash = canonical_hash(
        "bmad-capability-session-authority",
        1,
        &CapabilitySessionIdentity {
            schema_version: "sapphirus.bmad-capability-session.v1",
            renderer_session_id: input.renderer_session_id,
            workspace_id: &input.workspace_id,
            workspace_grant_epoch: input.workspace_grant_epoch,
            workspace_context_read_epoch: input.workspace_context_read_epoch,
        },
    )
    .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let run_digest = canonical_hash(
        "bmad-capability-run-id",
        1,
        &CapabilityRunIdentity {
            schema_version: "sapphirus.bmad-capability-run-id.v1",
            capability_id: input.binding.id,
            workspace_id: &input.workspace_id,
            manifest_hash: manifest.manifest_hash,
            created_at: input.created_at,
        },
    )
    .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let run_id = derived_contract_id("caprun", run_digest)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let request_id = derived_contract_id("modelreq", run_digest)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let consent_disclosure_hash = sha256_bytes(
        b"Only the exact reviewed context shown here will be sent once. Redaction reduces risk but cannot prove that every secret was detected.",
    );
    let invocation_binding = ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id,
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: input.installation_id.clone(),
        session_authority_hash,
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
        consent_disclosure_hash,
    }
    .seal()
    .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;

    Ok(PrepareCapabilityRunInput {
        capability_id,
        workspace_id: input.workspace_id,
        workspace_grant_epoch: input.workspace_grant_epoch,
        workspace_context_read_epoch: input.workspace_context_read_epoch,
        run_id,
        instruction_hash,
        output_schema_id: input.binding.output_schema_id.to_owned(),
        manifest,
        invocation_binding,
        deterministic_fixture: deterministic_fixture_for(input.binding),
        created_at: input.created_at,
    })
}

pub(crate) fn approve_input(
    capability_id: &str,
    manifest_hash: Sha256Digest,
    approved_at: UnixMillis,
) -> Result<ApproveCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(ApproveCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        manifest_hash,
        approved_at,
    })
}

pub(crate) fn cancel_input(
    capability_id: &str,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    cancelled_at: UnixMillis,
) -> Result<CancelCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(CancelCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        manifest_hash,
        decision_id,
        cancelled_at,
    })
}

pub(crate) fn submit_input(
    capability_id: &str,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    submitted_at: UnixMillis,
) -> Result<SubmitCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(SubmitCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        manifest_hash,
        decision_id,
        submitted_at,
    })
}

/// Builds bounded context candidates from governed workspace reads.
pub(crate) fn context_candidate(
    relative_path: RelativeWorkspacePath,
    content: String,
    index: usize,
) -> Result<ContextCandidate, BmadCapabilityCoordinatorError> {
    Ok(ContextCandidate {
        client_item_id: ContractId::new(format!("capctx_{index:04}"))
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        relative_label: relative_path,
        semantic_role: "capability_context".to_owned(),
        language: None,
        classification: ContextClassification::Internal,
        content,
    })
}
