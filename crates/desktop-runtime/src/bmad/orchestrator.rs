//! Deterministic step orchestration (ENG-08).
//!
//! Without this module, turning a declared BMAD step into an action would
//! mean interpreting its text at the point of execution — exactly where a
//! skill's own words could talk their way into more authority than they
//! should have. This module has no execution port at all: it takes
//! [`BmadSourceStep`] declarations and returns [`BmadActionPlan`] values,
//! nothing else. A step whose text asks for a write, a run, an install, or a
//! schedule becomes a `*_proposal`/`*_plan` kind carrying `requires_approval`
//! and a hashed [`BmadActionPlanEffect`] target — never an executed effect,
//! because there is no port here through which one could be executed. That
//! absence, not a runtime check, is what makes "skill text cannot directly
//! write/run/install/schedule" true.

use serde_json::json;
use thiserror::Error;

use crate::canonical_hash;
use crate::generated_contracts::{
    BmadActionPlan, BmadActionPlanBudget, BmadActionPlanEffect, BmadActionPlanEffectEffectClass,
    BmadActionPlanKind, BmadActionPlanModelCall, BmadActionPlanModelCallTrustMode,
};

use super::action_plan::{build_plan, BmadActionPlanError, BmadActionPlanRequest};

/// One declared BMAD source step. Orchestration is a pure function of these:
/// no clock, no filesystem, no chat transcript, nothing but the declared
/// text and the identifiers naming where it came from.
#[derive(Clone, Debug)]
pub struct BmadSourceStep {
    pub plan_id: String,
    pub source_id: String,
    pub step_index: i64,
    /// `"<verb>:<target>"`, e.g. `"write:notes/plan.md"` or `"read:spec.md"`.
    /// An unrecognized or missing verb degrades to an inert `artifact_plan`
    /// rather than ever being interpreted as an effect.
    pub text: String,
}

/// Why a source step's plan could not be built.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadOrchestratorError {
    /// A source step requested an effect it has no authority to perform.
    #[error("source step requested an effect it has no authority to perform")]
    UnauthorizedEffect,
    /// The step's plan failed action-plan construction.
    #[error("source step plan is invalid")]
    InvalidPlan,
}

/// The verb a step's text declares, classified without any execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepVerb {
    Read,
    Write,
    Run,
    Install,
    Schedule,
    Model,
    Note,
}

impl StepVerb {
    fn parse(verb: &str) -> Self {
        match verb {
            "read" => Self::Read,
            "write" => Self::Write,
            "run" | "exec" | "script" => Self::Run,
            "install" => Self::Install,
            "schedule" => Self::Schedule,
            "model" => Self::Model,
            _ => Self::Note,
        }
    }
}

/// Turns declared source steps into a bounded, ordered list of typed plans.
///
/// Every step yields exactly one plan. Plans never carry an executed effect
/// field — only [`BmadActionPlanEffect`], a hashed proposal an approver must
/// still authorize elsewhere.
///
/// # Errors
///
/// Returns [`BmadOrchestratorError::InvalidPlan`] when a step's declared
/// content fails action-plan construction (invalid identity, unsafe path, an
/// out-of-bounds budget, or an unhashable target), and
/// [`BmadOrchestratorError::UnauthorizedEffect`] if a constructed plan's
/// effect or authority does not match what its kind demands.
pub fn plan_source_steps(
    capability_id: &str,
    session_id: &str,
    budget: &BmadActionPlanBudget,
    steps: &[BmadSourceStep],
) -> Result<Vec<BmadActionPlan>, BmadOrchestratorError> {
    steps
        .iter()
        .map(|step| plan_one_step(capability_id, session_id, budget, step))
        .collect()
}

fn plan_one_step(
    capability_id: &str,
    session_id: &str,
    budget: &BmadActionPlanBudget,
    step: &BmadSourceStep,
) -> Result<BmadActionPlan, BmadOrchestratorError> {
    let step_hash = canonical_hash(
        "bmad-source-step",
        1,
        &json!({
            "sourceId": step.source_id,
            "stepIndex": step.step_index,
            "text": step.text,
        }),
    )
    .map_err(|_| BmadOrchestratorError::InvalidPlan)?;

    let (verb, target) = split_declaration(&step.text);
    let request = request_for(
        capability_id,
        session_id,
        step,
        step_hash,
        budget,
        verb,
        target,
    )?;
    build_plan(&request).map_err(map_build_error)
}

fn split_declaration(text: &str) -> (StepVerb, &str) {
    text.split_once(':')
        .map_or((StepVerb::Note, ""), |(verb, target)| {
            (StepVerb::parse(verb.trim()), target.trim())
        })
}

#[allow(clippy::too_many_arguments)]
fn request_for(
    capability_id: &str,
    session_id: &str,
    step: &BmadSourceStep,
    step_hash: crate::Sha256Digest,
    budget: &BmadActionPlanBudget,
    verb: StepVerb,
    target: &str,
) -> Result<BmadActionPlanRequest, BmadOrchestratorError> {
    let (kind, reads, effect, model_call) = match verb {
        StepVerb::Read => (
            BmadActionPlanKind::Read,
            if target.is_empty() {
                Vec::new()
            } else {
                vec![target.to_owned()]
            },
            None,
            None,
        ),
        StepVerb::Write => (
            BmadActionPlanKind::WriteProposal,
            Vec::new(),
            Some(effect_for(
                BmadActionPlanEffectEffectClass::Filesystem,
                target,
                true,
            )?),
            None,
        ),
        StepVerb::Run => (
            BmadActionPlanKind::CommandProposal,
            Vec::new(),
            Some(effect_for(
                BmadActionPlanEffectEffectClass::Process,
                target,
                false,
            )?),
            None,
        ),
        StepVerb::Install => (
            BmadActionPlanKind::ScriptPlan,
            Vec::new(),
            Some(effect_for(
                BmadActionPlanEffectEffectClass::Installation,
                target,
                false,
            )?),
            None,
        ),
        StepVerb::Schedule => (
            BmadActionPlanKind::SchedulePlan,
            Vec::new(),
            Some(effect_for(
                BmadActionPlanEffectEffectClass::Schedule,
                target,
                true,
            )?),
            None,
        ),
        StepVerb::Model => (
            BmadActionPlanKind::ModelCall,
            Vec::new(),
            None,
            Some(model_call_for(target)?),
        ),
        StepVerb::Note => (BmadActionPlanKind::ArtifactPlan, Vec::new(), None, None),
    };

    Ok(BmadActionPlanRequest {
        capability_id: capability_id.to_owned(),
        session_id: session_id.to_owned(),
        plan_id: step.plan_id.clone(),
        step_index: step.step_index,
        kind,
        source_id: step.source_id.clone(),
        step_hash,
        customization_hash: None,
        budget: budget.clone(),
        model_call,
        reads,
        effect,
    })
}

fn effect_for(
    effect_class: BmadActionPlanEffectEffectClass,
    target: &str,
    reversible: bool,
) -> Result<BmadActionPlanEffect, BmadOrchestratorError> {
    let target_hash = canonical_hash(
        "bmad-action-plan-effect",
        1,
        &json!({ "effectClass": effect_class, "target": target }),
    )
    .map_err(|_| BmadOrchestratorError::InvalidPlan)?;
    let target_hash = target_hash
        .to_string()
        .parse()
        .map_err(|_| BmadOrchestratorError::InvalidPlan)?;
    Ok(BmadActionPlanEffect {
        effect_class,
        reversible,
        target_hash,
    })
}

fn model_call_for(target: &str) -> Result<BmadActionPlanModelCall, BmadOrchestratorError> {
    let context_hash = canonical_hash(
        "bmad-action-plan-model-context",
        1,
        &json!({ "target": target }),
    )
    .map_err(|_| BmadOrchestratorError::InvalidPlan)?;
    let context_hash = context_hash
        .to_string()
        .parse()
        .map_err(|_| BmadOrchestratorError::InvalidPlan)?;
    let output_schema_id = "sapphirus.bmad-model-note.v1"
        .parse()
        .map_err(|_| BmadOrchestratorError::InvalidPlan)?;
    Ok(BmadActionPlanModelCall {
        context_hash,
        output_schema_id,
        redacted_prompt_hash: None,
        trust_mode: BmadActionPlanModelCallTrustMode::ReviewedAdvisory,
    })
}

fn map_build_error(error: BmadActionPlanError) -> BmadOrchestratorError {
    match error {
        BmadActionPlanError::AuthorityMismatch
        | BmadActionPlanError::EffectRequired
        | BmadActionPlanError::EffectForbidden
        | BmadActionPlanError::ModelCallMismatch => BmadOrchestratorError::UnauthorizedEffect,
        _ => BmadOrchestratorError::InvalidPlan,
    }
}
