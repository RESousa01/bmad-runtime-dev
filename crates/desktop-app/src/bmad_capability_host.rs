//! Host composition for the ADR-0005 capability runs (readiness Task 7).
//!
//! Maps every reviewed menu capability to its sealed instruction
//! projection and output archetype, assembles the exact reviewed context
//! through governed workspace reads, and drives the generic
//! [`BmadCapabilityCoordinator`] lifecycle. Model output is parsed from
//! untrusted JSON into the sealed archetype constructors and rejected on
//! any drift.

use std::collections::BTreeSet;

use desktop_cloud::{AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput};
use desktop_egress::{
    ContextCandidate, ContextClassification, ContextPreparer, EgressLimits,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput,
};
use desktop_ipc::{BmadProjectionError, BmadRuntimeMenuCapabilityBinding};
use desktop_runtime::{
    canonical_hash, deserialize_strict_json, sha256_bytes, BmadAgentRoster, BmadBuilderDraftFile,
    BmadBuilderDraftKind, BmadCandidateChange, BmadCapabilityOutput, BmadClosureCapabilityId,
    BmadDocumentArtifact, BmadDocumentSection, BmadGovernedChangeSet, BmadInactiveBuilderDraft,
    ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis, BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    BMAD_GOVERNED_CHANGE_SET_SCHEMA, BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
};
use serde::Serialize;
use serde_json::Value;

use crate::bmad_model::capability_coordinator::{
    capability_purpose, ApproveCapabilityRunInput, BmadCapabilityCoordinatorError,
    BmadCapabilityOutputVerifier, BmadCapabilityTransport, CancelCapabilityRunInput,
    PrepareCapabilityRunInput, SubmitCapabilityRunInput,
};
use crate::bmad_model::config::{HelpModelConfiguration, HelpModelMode};
use crate::bmad_model::context::derived_contract_id;

/// One reviewed menu capability binding: identity, archetype, projection.
pub(crate) struct CapabilityBinding {
    pub id: &'static str,
    pub output_schema_id: &'static str,
    pub projection_path: &'static str,
}

/// The 38 reviewed capabilities: 24 unique menu capabilities (ADR-0005),
/// the five Builder authoring operations (ADR-0006) whose outputs are
/// inactive drafts, and nine core skills adopted verbatim (ADR-0008).
pub(crate) const CAPABILITY_TABLE: [CapabilityBinding; 38] = [
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
    CapabilityBinding {
        id: "builder:agent.analyze",
        output_schema_id: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        projection_path: "runtime/builder/2.1.0/agent-analyze.instructions.md",
    },
    CapabilityBinding {
        id: "builder:agent.create_rebuild",
        output_schema_id: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        projection_path: "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
    },
    CapabilityBinding {
        id: "builder:agent.edit",
        output_schema_id: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        projection_path: "runtime/builder/2.1.0/agent-edit.instructions.md",
    },
    CapabilityBinding {
        id: "builder:workflow.analyze",
        output_schema_id: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        projection_path: "runtime/builder/2.1.0/workflow-analyze.instructions.md",
    },
    CapabilityBinding {
        id: "builder:workflow.build_edit",
        output_schema_id: BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
        projection_path: "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
    },
    // ADR-0008 core skills: the upstream SKILL.md is the instruction, adopted
    // verbatim. They carry the `bmm:` product namespace like every other
    // Method capability — `bmm:bmad-brainstorming` is a core skill upstream
    // too, and the module a capability came from is recorded in the package
    // descriptor, not in this id.
    CapabilityBinding {
        id: "bmm:bmad-spec",
        output_schema_id: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        projection_path: "source/method/6.10.0/src/core-skills/bmad-spec/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-advanced-elicitation",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "source/method/6.10.0/src/core-skills/bmad-advanced-elicitation/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-review-adversarial-general",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path:
            "source/method/6.10.0/src/core-skills/bmad-review-adversarial-general/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-review-edge-case-hunter",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path:
            "source/method/6.10.0/src/core-skills/bmad-review-edge-case-hunter/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-review-verification-gap",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path:
            "source/method/6.10.0/src/core-skills/bmad-review-verification-gap/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-editorial-review-prose",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path:
            "source/method/6.10.0/src/core-skills/bmad-editorial-review-prose/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-editorial-review-structure",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path:
            "source/method/6.10.0/src/core-skills/bmad-editorial-review-structure/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-shard-doc",
        output_schema_id: BMAD_GOVERNED_CHANGE_SET_SCHEMA,
        projection_path: "source/method/6.10.0/src/core-skills/bmad-shard-doc/SKILL.md",
    },
    CapabilityBinding {
        id: "bmm:bmad-index-docs",
        output_schema_id: BMAD_DOCUMENT_ARTIFACT_SCHEMA,
        projection_path: "source/method/6.10.0/src/core-skills/bmad-index-docs/SKILL.md",
    },
];

/// The monotonic ADR-0005 Method denominator. Shared capabilities such as
/// document-project and implementation-readiness remain distinct menu paths.
pub(crate) const METHOD_MENU_CAPABILITY_TABLE: [BmadRuntimeMenuCapabilityBinding<'static>; 26] = [
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
        menu_code: "CA",
        capability_id: "bmm:bmad-architecture",
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

pub(crate) fn capability_binding(capability_id: &str) -> Option<&'static CapabilityBinding> {
    CAPABILITY_TABLE
        .iter()
        .find(|binding| binding.id == capability_id)
}

/// The runtime-enabled Method menu paths.
///
/// Architect/CA was held back while the runtime had no ordered lane: its
/// sealed record is the foundation's only `entrypointKind: "step_jit"`
/// projection, and offering it through the one-shot lane would have
/// contradicted its own guidance. The ordered progression in
/// [`crate::bmad_architecture`] is that lane, so all 26 reviewed paths are
/// now enabled.
pub(crate) fn reviewed_method_menu_bindings(
    roster: &BmadAgentRoster,
) -> Result<Vec<BmadRuntimeMenuCapabilityBinding<'static>>, BmadProjectionError> {
    validate_method_menu_capability_bindings(roster, &METHOD_MENU_CAPABILITY_TABLE)?;
    Ok(METHOD_MENU_CAPABILITY_TABLE.to_vec())
}

/// ADR-0008 core skills are reachable directly rather than through an agent
/// menu: upstream invokes them from inside other skills, so they have no menu
/// path of their own. Their enablement is therefore the adopted-source
/// binding itself.
#[must_use]
pub(crate) fn is_core_skill_binding(binding: &CapabilityBinding) -> bool {
    binding
        .projection_path
        .starts_with("source/method/6.10.0/src/core-skills/")
}

pub(crate) fn runtime_capability_enabled(
    roster: &BmadAgentRoster,
    binding: &CapabilityBinding,
) -> Result<bool, BmadProjectionError> {
    if binding.id.starts_with("builder:") && binding.projection_path.starts_with("runtime/builder/")
    {
        return Ok(true);
    }
    if is_core_skill_binding(binding) {
        return Ok(true);
    }
    Ok(reviewed_method_menu_bindings(roster)?
        .iter()
        .any(|candidate| candidate.capability_id == binding.id))
}

/// The host-owned boundary a verbatim upstream instruction is read under.
///
/// ADR-0008 adopts instruction *bodies* unchanged, but ADR-0003's executable
/// exclusion still stands: those bodies routinely tell the model to run
/// scripts, read files, spawn subagents, or fetch the web. None of that is
/// available here. Rather than rewrite the body — which is what lost the
/// method's wording in the first place — the host states the boundary
/// alongside it, as one more reviewed context item.
pub(crate) fn adopted_instruction_preamble(
) -> Result<ContextCandidate, BmadCapabilityCoordinatorError> {
    Ok(ContextCandidate {
        client_item_id: ContractId::new("capctx_adopted_boundary")
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        relative_label: RelativeWorkspacePath::new(
            "review/bmad-capability/adopted-instruction-boundary.md",
        )
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        semantic_role: "adopted_instruction_boundary".to_owned(),
        language: Some("markdown".to_owned()),
        classification: ContextClassification::Internal,
        content: ADOPTED_INSTRUCTION_BOUNDARY.to_owned(),
    })
}

const ADOPTED_INSTRUCTION_BOUNDARY: &str = "# How to read the skill instruction

The instruction that follows is the upstream BMAD skill, carried unchanged. It was written for a runtime with tools this one does not provide. In this runtime you have **no** ability to run a script or command, read or write a file, spawn a subagent, fetch a URL, or hand off to an external system.

So:

- Ignore every instruction to run a script, resolve config from disk, write a   file, or invoke another skill. Those steps are performed by the host, not by   you, and their results are already in the reviewed context if they apply.
- Never claim to have run something, read something, or written something. If   a step needs an input you were not given, record it as an open question.
- Work only from the reviewed context supplied with this request.
- Return exactly the JSON result shape you were asked for, and nothing else.

Everything else in the instruction — its intent, its structure, its quality bar, and its vocabulary — applies in full.
";

fn validate_method_menu_capability_bindings(
    roster: &BmadAgentRoster,
    bindings: &[BmadRuntimeMenuCapabilityBinding<'_>],
) -> Result<(), BmadProjectionError> {
    let roster_paths = roster
        .agents
        .iter()
        .flat_map(|agent| {
            agent
                .menus
                .iter()
                .map(move |menu| (agent.agent_code.as_str(), menu.code.as_str()))
        })
        .collect::<BTreeSet<_>>();
    let canonical_paths = METHOD_MENU_CAPABILITY_TABLE
        .iter()
        .map(|binding| (binding.agent_code, binding.menu_code))
        .collect::<BTreeSet<_>>();
    if canonical_paths.len() != METHOD_MENU_CAPABILITY_TABLE.len()
        || canonical_paths != roster_paths
    {
        return Err(BmadProjectionError::Unavailable);
    }
    let mut reviewed_paths = BTreeSet::new();
    for binding in bindings {
        let Some(capability) = capability_binding(binding.capability_id) else {
            return Err(BmadProjectionError::Unavailable);
        };
        let path = (binding.agent_code, binding.menu_code);
        let exact_capability = METHOD_MENU_CAPABILITY_TABLE.iter().any(|candidate| {
            candidate.agent_code == binding.agent_code
                && candidate.menu_code == binding.menu_code
                && candidate.capability_id == binding.capability_id
        });
        if !capability.projection_path.starts_with("runtime/method/")
            || !exact_capability
            || !reviewed_paths.insert(path)
            || !roster_paths.contains(&path)
        {
            return Err(BmadProjectionError::Unavailable);
        }
    }
    if reviewed_paths.len() != canonical_paths.len() {
        return Err(BmadProjectionError::Unavailable);
    }
    Ok(())
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
        "document_artifact" | "governed_change_set" | "inactive_builder_draft"
            if object.len() != 2 =>
        {
            Err(BmadCapabilityCoordinatorError::OutputRejected)
        }
        "document_artifact" => parse_document_artifact(
            object
                .get("documentArtifact")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?,
        ),
        "governed_change_set" => parse_governed_change_set(
            object
                .get("governedChangeSet")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?,
        ),
        "inactive_builder_draft" => parse_inactive_builder_draft(
            object
                .get("inactiveBuilderDraft")
                .and_then(Value::as_object)
                .ok_or(BmadCapabilityCoordinatorError::OutputRejected)?,
        ),
        _ => Err(BmadCapabilityCoordinatorError::OutputRejected),
    }
}

fn parse_document_artifact(
    artifact: &serde_json::Map<String, Value>,
) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
    require_exact_keys(
        artifact,
        &[
            "schemaVersion",
            "title",
            "sections",
            "evidenceRefs",
            "openQuestions",
        ],
        &["mermaidText"],
    )?;
    {
        {
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
                    require_exact_keys(section, &["heading", "body"], &[])?;
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
    }
}

fn parse_governed_change_set(
    change_set: &serde_json::Map<String, Value>,
) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
    require_exact_keys(change_set, &["schemaVersion", "summary", "changes"], &[])?;
    {
        {
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
    }
}

fn parse_inactive_builder_draft(
    draft: &serde_json::Map<String, Value>,
) -> Result<BmadCapabilityOutput, BmadCapabilityCoordinatorError> {
    require_exact_keys(
        draft,
        &[
            "schemaVersion",
            "draftKind",
            "title",
            "revisionNote",
            "files",
        ],
        &[],
    )?;
    {
        {
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
                    require_exact_keys(file, &["path", "content"], &[])?;
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
    }
}

/// Rejects any key outside the closed reviewed set: an unexpected field is
/// treated as an authority-smuggling attempt, never ignored.
fn require_exact_keys(
    object: &serde_json::Map<String, Value>,
    required: &[&str],
    optional: &[&str],
) -> Result<(), BmadCapabilityCoordinatorError> {
    for key in object.keys() {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(BmadCapabilityCoordinatorError::OutputRejected);
        }
    }
    for key in required {
        if !object.contains_key(*key) {
            return Err(BmadCapabilityCoordinatorError::OutputRejected);
        }
    }
    Ok(())
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
    match change.get("operation").and_then(Value::as_str) {
        Some("create") => {
            require_exact_keys(change, &["operation", "path", "content"], &[])?;
            Ok(BmadCandidateChange::Create {
                path: required_relative_path(change.get("path"))?,
                content: required_string(change.get("content"))?,
            })
        }
        Some("replace") => {
            require_exact_keys(
                change,
                &["operation", "path", "content", "preimageSha256"],
                &[],
            )?;
            Ok(BmadCandidateChange::Replace {
                path: required_relative_path(change.get("path"))?,
                content: required_string(change.get("content"))?,
                preimage_sha256: required_digest(change.get("preimageSha256"))?,
            })
        }
        Some("delete") => {
            require_exact_keys(change, &["operation", "path", "preimageSha256"], &[])?;
            Ok(BmadCandidateChange::Delete {
                path: required_relative_path(change.get("path"))?,
                preimage_sha256: required_digest(change.get("preimageSha256"))?,
            })
        }
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
        let value: Value = deserialize_strict_json(output.payload_json.as_bytes())
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)?;
        parse_capability_result(&value)
    }
}

/// Deterministic fixture: one honest, clearly labeled result of the
/// capability's archetype, used only in the deterministic composition.
pub(crate) fn deterministic_fixture_for(binding: &CapabilityBinding) -> String {
    match binding.output_schema_id {
        BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA => serde_json::json!({
            "resultKind": "inactive_builder_draft",
            "inactiveBuilderDraft": {
                "schemaVersion": BMAD_INACTIVE_BUILDER_DRAFT_SCHEMA,
                "draftKind": if binding.id.starts_with("builder:workflow") { "workflow" } else { "agent" },
                "title": format!("Deterministic draft: {}", binding.id),
                "revisionNote": "Produced by the deterministic composition; inactive by design.",
                "files": [{
                    "path": "draft.instructions.md",
                    "content": "# Deterministic inactive draft

This draft cannot install, register, or execute.
",
                }],
            },
        })
        .to_string(),
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
                    "path": "deterministic-preview.proposal.md",
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

#[cfg(feature = "direct-model-providers")]
impl BmadCapabilityTransport for crate::bmad_model::transport::DirectModelTransport {
    fn send(
        &self,
        request: AuthorizedModelRequest,
        _deterministic_fixture: &str,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        self.send_authorized(request, now)
    }
}

pub(crate) fn active_capability_transport(
    state: &crate::state::HostState,
) -> Box<dyn BmadCapabilityTransport> {
    #[cfg(any(feature = "direct-model-providers", feature = "deterministic-help"))]
    let mode = state.model_configuration().ok().map(|value| value.mode);
    #[cfg(not(any(feature = "direct-model-providers", feature = "deterministic-help")))]
    let _ = state;
    #[cfg(feature = "direct-model-providers")]
    if mode == Some(HelpModelMode::DirectProviderDevelopment) {
        if let Some(config) = state.direct_provider_config() {
            if let Ok(transport) = crate::bmad_model::transport::DirectModelTransport::new(config) {
                return Box::new(transport);
            }
        }
        return Box::new(OfflineCapabilityTransport);
    }
    #[cfg(feature = "deterministic-help")]
    if mode == Some(HelpModelMode::DeterministicDevelopment) {
        return Box::new(DeterministicCapabilityTransport);
    }
    Box::new(OfflineCapabilityTransport)
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
    pub configuration: &'a HelpModelConfiguration,
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
/// The declared ceiling has to cover the host-injected candidates as well as
/// the user's attachments, otherwise a full attachment set plus the host's own
/// context overflows and surfaces as an opaque integrity failure rather than
/// "attach fewer files":
///
/// * up to `MAX_CAPABILITY_CONTEXT_PATHS` (100) user attachments,
/// * 1 sealed instruction candidate,
/// * 2 architecture progression candidates (selected step + decision log),
/// * 1 adopted-instruction boundary preamble for core-skill capabilities.
///
/// Must stay at or under `desktop-egress`'s `HARD_MAX_CONTEXT_ITEMS` (128).
const MAX_CAPABILITY_CONTEXT_ITEMS: u32 = 104;
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
    let configuration = input.configuration;
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
            region: configuration.region.clone(),
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
            candidates: {
                let mut candidates = Vec::with_capacity(input.candidates.len() + 1);
                candidates.push(sealed_instruction_candidate(input.instruction_bytes)?);
                candidates.extend(input.candidates);
                candidates
            },
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
    let invocation_binding = seal_capability_binding(SealCapabilityBinding {
        request_id,
        installation_id: input.installation_id,
        session_authority_hash,
        purpose,
        manifest: &manifest,
    })?;

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
    })
}

struct SealCapabilityBinding<'a> {
    request_id: ContractId,
    installation_id: &'a ContractId,
    session_authority_hash: Sha256Digest,
    purpose: String,
    manifest: &'a desktop_egress::ContextEgressManifest,
}

fn seal_capability_binding(
    input: SealCapabilityBinding<'_>,
) -> Result<desktop_egress::ModelInvocationBinding, BmadCapabilityCoordinatorError> {
    let SealCapabilityBinding {
        request_id,
        installation_id,
        session_authority_hash,
        purpose,
        manifest,
    } = input;
    let consent_disclosure_hash = sha256_bytes(
        b"Only the exact reviewed context shown here will be sent once. Redaction reduces risk but cannot prove that every secret was detected.",
    );
    ModelInvocationBindingDraft {
        schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
        request_id,
        tenant_ref: manifest.draft.tenant_ref.clone(),
        project_ref: manifest.draft.project_ref.clone(),
        run_ref: manifest.draft.run_ref.clone(),
        installation_id: installation_id.clone(),
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
    .map_err(|_| BmadCapabilityCoordinatorError::Integrity)
}

pub(crate) struct WorkspaceBindingInput {
    pub id: ContractId,
    pub grant_epoch: u64,
    pub context_read_epoch: u64,
}

pub(crate) fn approve_input(
    capability_id: &str,
    workspace: WorkspaceBindingInput,
    manifest_hash: Sha256Digest,
    approved_at: UnixMillis,
) -> Result<ApproveCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(ApproveCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        workspace_id: workspace.id,
        workspace_grant_epoch: workspace.grant_epoch,
        workspace_context_read_epoch: workspace.context_read_epoch,
        manifest_hash,
        approved_at,
    })
}

pub(crate) fn cancel_input(
    capability_id: &str,
    workspace: WorkspaceBindingInput,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    cancelled_at: UnixMillis,
) -> Result<CancelCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(CancelCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        workspace_id: workspace.id,
        workspace_grant_epoch: workspace.grant_epoch,
        workspace_context_read_epoch: workspace.context_read_epoch,
        manifest_hash,
        decision_id,
        cancelled_at,
    })
}

pub(crate) fn submit_input(
    capability_id: &str,
    workspace: WorkspaceBindingInput,
    manifest_hash: Sha256Digest,
    decision_id: ContractId,
    submitted_at: UnixMillis,
) -> Result<SubmitCapabilityRunInput, BmadCapabilityCoordinatorError> {
    Ok(SubmitCapabilityRunInput {
        capability_id: BmadClosureCapabilityId::new(capability_id)
            .map_err(|_| BmadCapabilityCoordinatorError::CapabilityBindingMismatch)?,
        workspace_id: workspace.id,
        workspace_grant_epoch: workspace.grant_epoch,
        workspace_context_read_epoch: workspace.context_read_epoch,
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

pub(crate) fn sealed_instruction_candidate(
    instruction_bytes: &[u8],
) -> Result<ContextCandidate, BmadCapabilityCoordinatorError> {
    let content = std::str::from_utf8(instruction_bytes)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?
        .to_owned();
    Ok(ContextCandidate {
        client_item_id: ContractId::new("capctx_sealed_instruction")
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        relative_label: RelativeWorkspacePath::new("review/bmad-capability/sealed-instruction.md")
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        semantic_role: "sealed_instruction".to_owned(),
        language: Some("markdown".to_owned()),
        classification: ContextClassification::Internal,
        content,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use super::{
        capability_binding, parse_capability_result, reviewed_method_menu_bindings,
        sealed_instruction_candidate, validate_method_menu_capability_bindings,
        WireCapabilityOutputVerifier, METHOD_MENU_CAPABILITY_TABLE,
    };
    use crate::bmad_foundation::load_bmad_foundation;
    use crate::bmad_model::capability_coordinator::BmadCapabilityOutputVerifier;
    use desktop_cloud::{ModelAccessReceipt, ModelReceiptStatus, RawModelOutput};
    use desktop_egress::RetentionMode;
    use desktop_ipc::BmadProjectionError;
    use desktop_runtime::{
        sha256_bytes, BmadClosureCapabilityId, ContractId, UnixMillis,
        BMAD_DOCUMENT_ARTIFACT_SCHEMA,
    };

    fn foundation_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
    }

    fn raw_capability_output(payload_json: &str) -> RawModelOutput {
        let payload_json = payload_json.to_owned();
        let payload_hash = sha256_bytes(payload_json.as_bytes());
        let request_id = ContractId::new("request_duplicate_keys").expect("request id");
        RawModelOutput {
            request_id: request_id.clone(),
            output_schema_id: ContractId::new(BMAD_DOCUMENT_ARTIFACT_SCHEMA)
                .expect("output schema id"),
            payload_json,
            payload_hash,
            receipt: ModelAccessReceipt {
                schema_version: "sapphirus.model-access-receipt.v1".to_owned(),
                receipt_id: ContractId::new("receipt_duplicate_keys").expect("receipt id"),
                request_id,
                request_hash: sha256_bytes(b"request"),
                result_hash: payload_hash,
                manifest_hash: sha256_bytes(b"manifest"),
                binding_hash: sha256_bytes(b"binding"),
                consumption_hash: sha256_bytes(b"consumption"),
                consent_disclosure_hash: sha256_bytes(b"disclosure"),
                provider_profile_hash: sha256_bytes(b"provider"),
                model_profile_hash: sha256_bytes(b"model"),
                deployment_hash: sha256_bytes(b"deployment"),
                retention_mode: RetentionMode::TransientNoStore,
                region: "westeurope".to_owned(),
                input_bytes: 1,
                output_bytes: 1,
                started_at: UnixMillis(1),
                completed_at: UnixMillis(2),
                status: ModelReceiptStatus::Succeeded,
                proof: "test-only".to_owned(),
            },
        }
    }

    #[test]
    fn wire_capability_output_rejects_duplicate_keys_at_every_nested_shape() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let cases = [
            (
                "root",
                r#"{"resultKind":"document_artifact","resultKind":"document_artifact","documentArtifact":{"schemaVersion":"sapphirus.bmad-document-artifact.v1","title":"Title","sections":[{"heading":"Summary","body":"Body"}],"evidenceRefs":[],"openQuestions":[]}}"#.to_owned(),
            ),
            (
                "document section",
                r#"{"resultKind":"document_artifact","documentArtifact":{"schemaVersion":"sapphirus.bmad-document-artifact.v1","title":"Title","sections":[{"heading":"Summary","body":"Body","body":"Body"}],"evidenceRefs":[],"openQuestions":[]}}"#.to_owned(),
            ),
            (
                "Builder file",
                r#"{"resultKind":"inactive_builder_draft","inactiveBuilderDraft":{"schemaVersion":"sapphirus.bmad-inactive-builder-draft.v1","draftKind":"agent","title":"Draft","revisionNote":"Reviewed","files":[{"path":"agent.md","content":"Draft","content":"Draft"}]}}"#.to_owned(),
            ),
            (
                "governed change",
                r#"{"resultKind":"governed_change_set","governedChangeSet":{"schemaVersion":"sapphirus.bmad-governed-change-set.v1","summary":"Change","changes":[{"operation":"create","operation":"create","path":"new.md","content":"New"}]}}"#.to_owned(),
            ),
            (
                "governed content",
                r#"{"resultKind":"governed_change_set","governedChangeSet":{"schemaVersion":"sapphirus.bmad-governed-change-set.v1","summary":"Change","changes":[{"operation":"create","path":"new.md","content":"New","content":"New"}]}}"#.to_owned(),
            ),
            (
                "governed preimage",
                format!(
                    r#"{{"resultKind":"governed_change_set","governedChangeSet":{{"schemaVersion":"sapphirus.bmad-governed-change-set.v1","summary":"Change","changes":[{{"operation":"replace","path":"existing.md","content":"New","preimageSha256":"{digest}","preimageSha256":"{digest}"}}]}}}}"#
                ),
            ),
        ];
        let capability_id =
            BmadClosureCapabilityId::new("bmm:bmad-product-brief").expect("capability id");
        let accepted = cases
            .iter()
            .filter_map(|(label, payload)| {
                WireCapabilityOutputVerifier
                    .verify(&capability_id, &raw_capability_output(payload))
                    .is_ok()
                    .then_some(*label)
            })
            .collect::<Vec<_>>();

        assert!(
            accepted.is_empty(),
            "duplicate decoded keys crossed the wire verifier: {accepted:?}"
        );
    }

    #[test]
    fn reviewed_method_menu_table_closes_the_exact_runtime_roster() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let bindings = reviewed_method_menu_bindings(foundation.roster())
            .expect("reviewed Method menu capability bindings");
        let closure_paths = METHOD_MENU_CAPABILITY_TABLE
            .iter()
            .map(|binding| (binding.agent_code, binding.menu_code))
            .collect::<BTreeSet<_>>();

        assert_eq!(closure_paths.len(), 26);
        assert_eq!(bindings.len(), 26);
        assert!(METHOD_MENU_CAPABILITY_TABLE
            .iter()
            .all(|binding| capability_binding(binding.capability_id).is_some()));
        // Architect/CA is enabled by the ordered architecture progression.
        assert!(bindings.iter().any(|binding| {
            binding.agent_code == "bmad-agent-architect"
                && binding.menu_code == "CA"
                && binding.capability_id == crate::bmad_architecture::ARCHITECTURE_CAPABILITY_ID
        }));
        for menu_code in ["WD", "MG", "VD", "EC"] {
            assert!(bindings.iter().any(|binding| {
                binding.agent_code == "bmad-agent-tech-writer" && binding.menu_code == menu_code
            }));
        }
    }

    #[test]
    fn reviewed_method_menu_table_rejects_unknown_and_duplicate_bindings() {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");

        let mut unknown_capability = METHOD_MENU_CAPABILITY_TABLE;
        unknown_capability[0].capability_id = "bmm:not-reviewed";
        assert!(matches!(
            validate_method_menu_capability_bindings(foundation.roster(), &unknown_capability),
            Err(BmadProjectionError::Unavailable)
        ));

        let mut duplicate_path = METHOD_MENU_CAPABILITY_TABLE;
        duplicate_path[1].agent_code = duplicate_path[0].agent_code;
        duplicate_path[1].menu_code = duplicate_path[0].menu_code;
        assert!(matches!(
            validate_method_menu_capability_bindings(foundation.roster(), &duplicate_path),
            Err(BmadProjectionError::Unavailable)
        ));

        let mut unknown_path = METHOD_MENU_CAPABILITY_TABLE;
        unknown_path[0].menu_code = "UNKNOWN";
        assert!(matches!(
            validate_method_menu_capability_bindings(foundation.roster(), &unknown_path),
            Err(BmadProjectionError::Unavailable)
        ));

        let mut swapped_capabilities = METHOD_MENU_CAPABILITY_TABLE;
        let first_capability = swapped_capabilities[1].capability_id;
        swapped_capabilities[1].capability_id = swapped_capabilities[2].capability_id;
        swapped_capabilities[2].capability_id = first_capability;
        assert!(matches!(
            validate_method_menu_capability_bindings(foundation.roster(), &swapped_capabilities),
            Err(BmadProjectionError::Unavailable)
        ));
    }

    #[test]
    fn sealed_capability_instruction_is_part_of_reviewed_context() {
        let candidate = sealed_instruction_candidate(b"Follow the sealed BMAD method.")
            .expect("sealed instruction candidate");

        assert_eq!(candidate.semantic_role, "sealed_instruction");
        assert_eq!(
            candidate.relative_label.as_str(),
            "review/bmad-capability/sealed-instruction.md"
        );
        assert_eq!(candidate.content, "Follow the sealed BMAD method.");
    }

    fn draft_value(extra_draft_keys: &str, extra_top_keys: &str) -> serde_json::Value {
        let json = format!(
            r##"{{
              "resultKind": "inactive_builder_draft"{extra_top_keys},
              "inactiveBuilderDraft": {{
                "schemaVersion": "sapphirus.bmad-inactive-builder-draft.v1",
                "draftKind": "agent",
                "title": "Draft reviewer",
                "revisionNote": "First draft.",
                "files": [{{"path": "agent.instructions.md", "content": "# Draft"}}]{extra_draft_keys}
              }}
            }}"##
        );
        serde_json::from_str(&json).expect("test JSON")
    }

    #[test]
    fn builder_drafts_parse_only_as_inactive_data() {
        assert!(parse_capability_result(&draft_value("", "")).is_ok());
    }

    #[test]
    fn authority_bearing_fields_fail_closed_everywhere() {
        // Draft-level authority claims are rejected, never ignored.
        for smuggled in [
            r#", "activate": true"#,
            r#", "registration": {"install": true}"#,
            r#", "command": "pwsh""#,
            r#", "hooks": []"#,
            r#", "network": {"allow": "*"}"#,
        ] {
            assert!(
                parse_capability_result(&draft_value(smuggled, "")).is_err(),
                "accepted draft authority field: {smuggled}"
            );
        }
        // Top-level envelope smuggling is rejected too.
        assert!(parse_capability_result(&draft_value("", r#", "approved": true"#)).is_err());
    }

    #[test]
    fn document_sections_reject_extra_nested_keys() {
        let value = serde_json::json!({
            "resultKind": "document_artifact",
            "documentArtifact": {
                "schemaVersion": "sapphirus.bmad-document-artifact.v1",
                "title": "Reviewed document",
                "sections": [{
                    "heading": "Summary",
                    "body": "Reviewed content.",
                    "approved": true,
                }],
                "evidenceRefs": [],
                "openQuestions": [],
            },
        });

        assert!(parse_capability_result(&value).is_err());
    }

    #[test]
    fn builder_files_reject_extra_nested_keys() {
        let mut value = draft_value("", "");
        value["inactiveBuilderDraft"]["files"][0]["activate"] = serde_json::json!(true);

        assert!(parse_capability_result(&value).is_err());
    }

    #[test]
    fn governed_changes_reject_extra_nested_keys() {
        let digest = format!("sha256:{}", "a".repeat(64));
        for change in [
            serde_json::json!({
                "operation": "create",
                "path": "docs/create.md",
                "content": "# Create",
                "approved": true,
            }),
            serde_json::json!({
                "operation": "replace",
                "path": "docs/replace.md",
                "content": "# Replace",
                "preimageSha256": digest.clone(),
                "approved": true,
            }),
            serde_json::json!({
                "operation": "delete",
                "path": "docs/delete.md",
                "preimageSha256": digest,
                "approved": true,
            }),
        ] {
            let value = serde_json::json!({
                "resultKind": "governed_change_set",
                "governedChangeSet": {
                    "schemaVersion": "sapphirus.bmad-governed-change-set.v1",
                    "summary": "Reviewed changes",
                    "changes": [change],
                },
            });

            assert!(parse_capability_result(&value).is_err());
        }
    }
}
