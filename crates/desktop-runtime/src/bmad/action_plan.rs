//! Typed, tamper-evident BMAD action plans (ENG-08).
//!
//! Without this module a skill step's text would decide, on its own, whether
//! it gets to write a file, run a command, or merely read one. Here `kind`
//! is the only thing that ever chooses `authorityClass`: `build_plan` derives
//! the class itself and never accepts one from a caller, so skill-authored
//! text cannot raise it. `validate_plan` re-derives the same class and
//! recomputes `planHash` over the plan's own content, so a plan edited after
//! construction — by a bug, a replay, or an adversarial round trip through
//! JSON — is caught rather than silently trusted.

use serde_json::Value;
use thiserror::Error;

use crate::generated_contracts::{
    BmadActionPlan, BmadActionPlanAuthorityClass, BmadActionPlanBudget, BmadActionPlanEffect,
    BmadActionPlanKind, BmadActionPlanModelCall, BmadActionPlanProvenance, BmadActionPlanReadsItem,
    CommonContractId, CommonSha256,
};
use crate::{canonical_hash_without_field, Sha256Digest};

/// Contract-declared bounds a plan's numeric budget fields must stay inside.
const MAX_MODEL_CALLS: i64 = 1024;
const MAX_TOKENS: i64 = 4_194_304;
const MAX_RETRIES: i64 = 64;
const MAX_REPAIRS: i64 = 64;
const MAX_READS: usize = 256;

/// Everything the orchestrator supplies to construct one plan.
///
/// `authority_class` is deliberately absent: [`build_plan`] derives it from
/// `kind`, which is the structural guarantee that a source step's text can
/// never grant itself more authority than its declared kind allows.
#[derive(Clone, Debug)]
pub struct BmadActionPlanRequest {
    pub capability_id: String,
    pub session_id: String,
    pub plan_id: String,
    pub step_index: i64,
    pub kind: BmadActionPlanKind,
    pub source_id: String,
    pub step_hash: Sha256Digest,
    pub customization_hash: Option<Sha256Digest>,
    pub budget: BmadActionPlanBudget,
    pub model_call: Option<BmadActionPlanModelCall>,
    pub reads: Vec<String>,
    pub effect: Option<BmadActionPlanEffect>,
}

/// Why a plan could not be built or failed post-construction validation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadActionPlanError {
    /// A budget field fell outside the contract's declared bounds.
    #[error("plan budget exceeds its contract bounds")]
    BudgetExceeded,
    /// An identifier did not match the contract's identity pattern.
    #[error("plan identity is invalid")]
    InvalidIdentity,
    /// A `reads` entry was absolute, escaped its root, or used a backslash.
    #[error("plan read path is unsafe")]
    UnsafePath,
    /// `authorityClass` does not match the class its `kind` requires.
    #[error("plan authority class does not match its kind")]
    AuthorityMismatch,
    /// A kind that must carry an effect was built or found without one.
    #[error("plan requires an effect its kind demands")]
    EffectRequired,
    /// A kind that must not carry an effect was built or found with one.
    #[error("plan carries an effect its kind forbids")]
    EffectForbidden,
    /// Only `model_call` plans may carry a `modelCall` block.
    #[error("plan model call is required or forbidden by its kind")]
    ModelCallMismatch,
    /// Too many `reads` entries for the contract's bounded array.
    #[error("plan reads exceed the bounded array size")]
    TooManyReads,
    /// The plan content could not be canonically hashed.
    #[error("plan hash could not be computed")]
    Hash,
    /// A previously built plan's content no longer matches its `planHash`.
    #[error("plan hash does not match its content")]
    TamperDetected,
}

/// The `authorityClass` a `kind` is permitted to carry. This is the single
/// place authority is decided; every other function in this module and in
/// `orchestrator.rs` routes through it rather than trusting a caller value.
#[must_use]
pub const fn required_authority_class(kind: BmadActionPlanKind) -> BmadActionPlanAuthorityClass {
    match kind {
        BmadActionPlanKind::Read => BmadActionPlanAuthorityClass::ReadOnly,
        BmadActionPlanKind::ModelCall | BmadActionPlanKind::ArtifactPlan => {
            BmadActionPlanAuthorityClass::None
        }
        BmadActionPlanKind::WriteProposal
        | BmadActionPlanKind::CommandProposal
        | BmadActionPlanKind::ScriptPlan
        | BmadActionPlanKind::SchedulePlan => BmadActionPlanAuthorityClass::RequiresApproval,
    }
}

/// True for the kinds whose authority is only meaningful alongside a
/// declared, hashed effect target.
#[must_use]
const fn effect_required(kind: BmadActionPlanKind) -> bool {
    matches!(
        kind,
        BmadActionPlanKind::WriteProposal
            | BmadActionPlanKind::CommandProposal
            | BmadActionPlanKind::ScriptPlan
            | BmadActionPlanKind::SchedulePlan
    )
}

/// Builds one contract-conformant plan with `authorityClass` and `planHash`
/// both derived, never accepted, from the rest of the content.
///
/// # Errors
///
/// Returns [`BmadActionPlanError`] for an invalid identity, an out-of-bounds
/// budget, an unsafe `reads` entry, a `kind`/effect mismatch, a
/// `kind`/model-call mismatch, or a hashing failure.
pub fn build_plan(request: &BmadActionPlanRequest) -> Result<BmadActionPlan, BmadActionPlanError> {
    let capability_id = parse_contract_id(&request.capability_id)?;
    let session_id = parse_contract_id(&request.session_id)?;
    let plan_id = parse_contract_id(&request.plan_id)?;
    let source_id = parse_contract_id(&request.source_id)?;
    check_budget(&request.budget)?;
    check_effect_shape(request.kind, request.effect.is_some())?;
    check_model_call_shape(request.kind, request.model_call.is_some())?;
    let reads = build_reads(&request.reads)?;

    let provenance = BmadActionPlanProvenance {
        source_id,
        step_hash: sha256_contract(request.step_hash)?,
        customization_hash: request
            .customization_hash
            .map(sha256_contract)
            .transpose()?,
    };

    let placeholder_hash = sha256_contract(Sha256Digest::from_bytes([0; 32]))?;
    let mut plan = BmadActionPlan {
        authority_class: required_authority_class(request.kind),
        budget: request.budget.clone(),
        capability_id,
        effect: request.effect.clone(),
        kind: request.kind,
        model_call: request.model_call.clone(),
        plan_hash: placeholder_hash,
        plan_id,
        provenance,
        reads,
        schema_version: Value::String("sapphirus.bmad-action-plan.v1".to_owned()),
        session_id,
        step_index: request.step_index,
    };
    plan.plan_hash = sha256_contract(content_hash(&plan)?)?;
    Ok(plan)
}

/// Re-derives `authorityClass` and `planHash` from a plan's own content and
/// rejects any drift between what the plan claims and what its content
/// requires.
///
/// # Errors
///
/// Returns [`BmadActionPlanError::AuthorityMismatch`] when `authorityClass`
/// does not match the class `kind` requires,
/// [`BmadActionPlanError::EffectRequired`] or
/// [`BmadActionPlanError::EffectForbidden`] when the effect does not match
/// what `kind` demands, [`BmadActionPlanError::UnsafePath`] for an unsafe
/// `reads` entry, and [`BmadActionPlanError::TamperDetected`] when the
/// content no longer matches `planHash`.
pub fn validate_plan(plan: &BmadActionPlan) -> Result<(), BmadActionPlanError> {
    if plan.authority_class != required_authority_class(plan.kind) {
        return Err(BmadActionPlanError::AuthorityMismatch);
    }
    check_effect_shape(plan.kind, plan.effect.is_some())?;
    check_model_call_shape(plan.kind, plan.model_call.is_some())?;
    for entry in &plan.reads {
        if !path_is_safe(entry) {
            return Err(BmadActionPlanError::UnsafePath);
        }
    }
    let expected = content_hash(plan)?;
    let actual =
        Sha256Digest::parse(plan.plan_hash.as_str()).map_err(|_| BmadActionPlanError::Hash)?;
    if expected != actual {
        return Err(BmadActionPlanError::TamperDetected);
    }
    Ok(())
}

/// Hashes a plan's content, excluding its own `planHash` field.
fn content_hash(plan: &BmadActionPlan) -> Result<Sha256Digest, BmadActionPlanError> {
    let value = serde_json::to_value(plan).map_err(|_| BmadActionPlanError::Hash)?;
    canonical_hash_without_field("bmad-action-plan", 1, &value, "planHash")
        .map_err(|_| BmadActionPlanError::Hash)
}

fn check_budget(budget: &BmadActionPlanBudget) -> Result<(), BmadActionPlanError> {
    let in_bounds = (0..=MAX_MODEL_CALLS).contains(&budget.max_model_calls)
        && (0..=MAX_TOKENS).contains(&budget.max_tokens)
        && (0..=MAX_RETRIES).contains(&budget.max_retries)
        && (0..=MAX_REPAIRS).contains(&budget.max_repairs);
    if in_bounds {
        Ok(())
    } else {
        Err(BmadActionPlanError::BudgetExceeded)
    }
}

fn check_effect_shape(
    kind: BmadActionPlanKind,
    has_effect: bool,
) -> Result<(), BmadActionPlanError> {
    match (effect_required(kind), has_effect) {
        (true, false) => Err(BmadActionPlanError::EffectRequired),
        (false, true) => Err(BmadActionPlanError::EffectForbidden),
        _ => Ok(()),
    }
}

fn check_model_call_shape(
    kind: BmadActionPlanKind,
    has_model_call: bool,
) -> Result<(), BmadActionPlanError> {
    let wants = kind == BmadActionPlanKind::ModelCall;
    if wants == has_model_call {
        Ok(())
    } else {
        Err(BmadActionPlanError::ModelCallMismatch)
    }
}

fn build_reads(paths: &[String]) -> Result<Vec<BmadActionPlanReadsItem>, BmadActionPlanError> {
    if paths.len() > MAX_READS {
        return Err(BmadActionPlanError::TooManyReads);
    }
    paths
        .iter()
        .map(|path| {
            if !path_is_safe(path) {
                return Err(BmadActionPlanError::UnsafePath);
            }
            path.parse().map_err(|_| BmadActionPlanError::UnsafePath)
        })
        .collect()
}

/// Rejects an absolute path, a `..` traversal segment, or a backslash. The
/// generated schema already carries this pattern; it is enforced again here
/// so a hand-built plan can never rely on the schema alone.
fn path_is_safe(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    !path
        .split('/')
        .any(|segment| segment == ".." || segment == ".")
}

fn parse_contract_id(value: &str) -> Result<CommonContractId, BmadActionPlanError> {
    value
        .parse()
        .map_err(|_| BmadActionPlanError::InvalidIdentity)
}

fn sha256_contract(digest: Sha256Digest) -> Result<CommonSha256, BmadActionPlanError> {
    digest
        .to_string()
        .parse()
        .map_err(|_| BmadActionPlanError::Hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step_hash() -> Sha256Digest {
        Sha256Digest::from_bytes([7; 32])
    }

    fn request(
        kind: BmadActionPlanKind,
        effect: Option<BmadActionPlanEffect>,
    ) -> BmadActionPlanRequest {
        BmadActionPlanRequest {
            capability_id: "capability_01J00000000000000000000000".to_owned(),
            session_id: "session_01J00000000000000000000000".to_owned(),
            plan_id: "plan_01J00000000000000000000000".to_owned(),
            step_index: 0,
            kind,
            source_id: "source_01J00000000000000000000000".to_owned(),
            step_hash: step_hash(),
            customization_hash: None,
            budget: BmadActionPlanBudget {
                max_model_calls: 1,
                max_repairs: 0,
                max_retries: 0,
                max_tokens: 100,
            },
            model_call: None,
            reads: Vec::new(),
            effect: effect.clone(),
        }
    }

    fn effect() -> BmadActionPlanEffect {
        BmadActionPlanEffect {
            effect_class: crate::generated_contracts::BmadActionPlanEffectEffectClass::Filesystem,
            reversible: false,
            target_hash: Sha256Digest::from_bytes([9; 32])
                .to_string()
                .parse()
                .unwrap(),
        }
    }

    // Break caught: a write proposal without an effect could otherwise carry
    // requires_approval authority with nothing an approver can inspect.
    #[test]
    fn write_proposal_without_effect_is_rejected() {
        let request = request(BmadActionPlanKind::WriteProposal, None);
        assert!(matches!(
            build_plan(&request),
            Err(BmadActionPlanError::EffectRequired)
        ));
    }

    // Break caught: a read plan carrying an effect would let a read silently
    // smuggle a side effect past the reviewer.
    #[test]
    fn read_plan_with_effect_is_rejected() {
        let request = request(BmadActionPlanKind::Read, Some(effect()));
        assert!(matches!(
            build_plan(&request),
            Err(BmadActionPlanError::EffectForbidden)
        ));
    }

    // Break caught: a tampered plan whose authorityClass was hand-lowered to
    // `none` after construction must still fail closed on validation.
    #[test]
    fn declared_authority_lower_than_kind_requires_fails_closed() {
        let mut plan = build_plan(&request(
            BmadActionPlanKind::CommandProposal,
            Some(effect()),
        ))
        .expect("plan builds");
        plan.authority_class = BmadActionPlanAuthorityClass::None;
        assert_eq!(
            validate_plan(&plan),
            Err(BmadActionPlanError::AuthorityMismatch)
        );
    }

    // Break caught: editing plan content after construction without
    // recomputing planHash must be detectable, not silently accepted.
    #[test]
    fn edited_plan_content_fails_hash_validation() {
        let mut plan = build_plan(&request(BmadActionPlanKind::Read, None)).expect("plan builds");
        plan.step_index = 99;
        assert_eq!(
            validate_plan(&plan),
            Err(BmadActionPlanError::TamperDetected)
        );
    }

    #[test]
    fn absolute_and_traversal_and_backslash_paths_are_unsafe() {
        assert!(!path_is_safe("/etc/passwd"));
        assert!(!path_is_safe("../secret"));
        assert!(!path_is_safe("a/../b"));
        assert!(!path_is_safe("a\\b"));
        assert!(path_is_safe("docs/plan.md"));
    }
}
