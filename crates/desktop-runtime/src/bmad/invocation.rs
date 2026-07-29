//! Interactive and headless invocation parity (ENG-06).
//!
//! Without this module a source-declared action could be run twice — once
//! from a human answering prompts, once from an unattended pipeline — and
//! silently diverge: the headless path would either invent a value for a
//! question nobody answered, or the two paths would resolve equivalent
//! inputs to different artifacts because each mode grew its own ad hoc
//! resolution order. This module gives both modes exactly one resolution
//! path (`resolve`) so the only permitted differences are the ones the
//! contract names: headless clamps `maxPromptedInputs` to zero and can
//! never accept a prompted answer, while interactive may leave a required
//! input unresolved as an outstanding question for a later turn. Precedence
//! is fixed (argument > answer > customization > config > `declared_default`)
//! so a restart that resupplies the same origins reproduces the same
//! `resolvedInputsHash`, which is what makes interactive resume safe: the
//! caller persists the contract, not a bespoke session type.

use serde_json::Value;
use thiserror::Error;

use crate::canonical_hash;
use crate::generated_contracts::{
    BmadInvocation, BmadInvocationBudget, BmadInvocationDeclaredAction,
    BmadInvocationDeclaredInput, BmadInvocationDeclaredInputValueKind,
    BmadInvocationInputRequirement, BmadInvocationInputRequirementAllowedValuesItem,
    BmadInvocationInputRequirementCode, BmadInvocationInputRequirementName,
    BmadInvocationInputRequirementPrompt, BmadInvocationInputRequirementValueKind,
    BmadInvocationMode, BmadInvocationResolvedInput, BmadInvocationResolvedInputName,
    BmadInvocationResolvedInputOrigin, CommonContractId,
};

/// Fixed precedence for resolving one declared input from many origins.
///
/// Argument beats a live answer beats a customization overlay beats config
/// beats the source's declared default. This order is contract-visible
/// (the `origin` recorded on every resolved input), so it must never be
/// reordered without a schema change.
const ORIGIN_PRECEDENCE: [BmadInvocationResolvedInputOrigin; 5] = [
    BmadInvocationResolvedInputOrigin::Argument,
    BmadInvocationResolvedInputOrigin::Answer,
    BmadInvocationResolvedInputOrigin::Customization,
    BmadInvocationResolvedInputOrigin::Config,
    BmadInvocationResolvedInputOrigin::DeclaredDefault,
];

const MAX_DECLARED_INPUTS: usize = 256;
const MAX_ALLOWED_VALUES: usize = 64;
const MAX_MODEL_CALLS: i64 = 1024;
const MAX_RETRIES: i64 = 64;
const MAX_PROMPTED_INPUTS: i64 = 256;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadInvocationError {
    /// The declared action exceeded its bounded input count, or a declared
    /// input's allowed-value list exceeded its bounded count.
    #[error("invocation declared inputs exceed the bounded count")]
    BoundsViolation,
    /// A headless caller supplied a value with `origin: answer`. Headless
    /// never prompts, so an answer origin can only mean a caller tried to
    /// smuggle a guessed or replayed value in through the back door.
    #[error("headless invocations cannot accept a prompted answer")]
    HeadlessAnswerRejected,
    /// The requested budget falls outside the contract's bounded range.
    #[error("invocation budget is outside its bounded range")]
    InvalidBudget,
    /// A declared input or its supplied value failed contract-shape
    /// validation (name pattern, prompt length, allowed-value length).
    #[error("a declared or resolved value could not be validated against the contract")]
    InvalidValue,
    /// The resolved-input hash could not be computed.
    #[error("invocation hash could not be computed")]
    Hash,
}

#[must_use]
pub const fn is_headless(mode: BmadInvocationMode) -> bool {
    matches!(mode, BmadInvocationMode::Headless)
}

/// One supplied candidate value for a declared input, tagged with the
/// origin it came from. The resolver never infers an origin; every value
/// the caller wants considered must be labelled with the origin that
/// produced it.
pub type BmadInvocationSuppliedValue = (String, BmadInvocationResolvedInputOrigin, Value);

/// The pure engine both invocation modes run through. It performs no I/O
/// and has no side effects: given a declared action, a set of supplied
/// values, a mode, and a requested budget, it deterministically produces
/// the same `BmadInvocation` contract every time.
pub struct BmadInvocationResolver;

/// One caller's request to resolve a declared action.
///
/// The identities and the requested budget travel together because a resolved
/// invocation is only meaningful bound to the session and capability it was
/// resolved for; passing them separately invited callers to pair the wrong
/// ones.
#[derive(Clone, Debug)]
pub struct BmadInvocationRequest<'a> {
    pub invocation_id: CommonContractId,
    pub capability_id: CommonContractId,
    pub session_id: CommonContractId,
    pub mode: BmadInvocationMode,
    pub action: &'a BmadInvocationDeclaredAction,
    pub supplied: &'a [BmadInvocationSuppliedValue],
    pub max_model_calls: i64,
    pub max_retries: i64,
    /// Ignored in headless mode, which always resolves to zero.
    pub requested_max_prompted_inputs: i64,
}

impl BmadInvocationResolver {
    /// Resolves one declared action's inputs for either invocation mode.
    ///
    /// # Errors
    ///
    /// Returns [`BmadInvocationError`] when the declared action exceeds its
    /// bounded input count, a headless caller supplies a prompted answer,
    /// the requested budget is outside its contract range, a declared or
    /// resolved value fails contract validation, or the resolved-input hash
    /// cannot be computed.
    pub fn resolve(
        request: &BmadInvocationRequest<'_>,
    ) -> Result<BmadInvocation, BmadInvocationError> {
        let action = request.action;
        let mode = request.mode;
        if action.declared_inputs.len() > MAX_DECLARED_INPUTS {
            return Err(BmadInvocationError::BoundsViolation);
        }
        if is_headless(mode)
            && request
                .supplied
                .iter()
                .any(|(_, origin, _)| is_answer(*origin))
        {
            return Err(BmadInvocationError::HeadlessAnswerRejected);
        }

        // Headless mode cannot prompt at all, so its budget is clamped rather
        // than merely validated: a caller asking for prompts gets zero, not an
        // error, because the same call site serves both modes.
        let max_prompted_inputs = if is_headless(mode) {
            0
        } else {
            request.requested_max_prompted_inputs
        };
        validate_budget(
            request.max_model_calls,
            request.max_retries,
            max_prompted_inputs,
        )?;

        let (resolved_inputs, missing_inputs) = resolve_declared_inputs(action, request.supplied)?;
        let ready = missing_inputs.is_empty();
        let resolved_inputs_hash = canonical_hash("bmad-invocation-inputs", 1, &resolved_inputs)
            .map_err(|_| BmadInvocationError::Hash)?
            .to_string()
            .parse()
            .map_err(|_| BmadInvocationError::Hash)?;

        Ok(BmadInvocation {
            budget: BmadInvocationBudget {
                max_model_calls: request.max_model_calls,
                max_prompted_inputs,
                max_retries: request.max_retries,
            },
            capability_id: request.capability_id.clone(),
            declared_action: action.clone(),
            invocation_id: request.invocation_id.clone(),
            missing_inputs,
            mode,
            ready: Some(ready),
            resolved_inputs,
            resolved_inputs_hash,
            schema_version: Value::String("sapphirus.bmad-invocation.v1".to_owned()),
            session_id: request.session_id.clone(),
        })
    }
}

const fn is_answer(origin: BmadInvocationResolvedInputOrigin) -> bool {
    matches!(origin, BmadInvocationResolvedInputOrigin::Answer)
}

fn validate_budget(
    max_model_calls: i64,
    max_retries: i64,
    max_prompted_inputs: i64,
) -> Result<(), BmadInvocationError> {
    let in_range = (0..=MAX_MODEL_CALLS).contains(&max_model_calls)
        && (0..=MAX_RETRIES).contains(&max_retries)
        && (0..=MAX_PROMPTED_INPUTS).contains(&max_prompted_inputs);
    if in_range {
        Ok(())
    } else {
        Err(BmadInvocationError::InvalidBudget)
    }
}

#[allow(clippy::type_complexity)]
fn resolve_declared_inputs(
    action: &BmadInvocationDeclaredAction,
    supplied: &[BmadInvocationSuppliedValue],
) -> Result<
    (
        Vec<BmadInvocationResolvedInput>,
        Vec<BmadInvocationInputRequirement>,
    ),
    BmadInvocationError,
> {
    let mut resolved_inputs = Vec::with_capacity(action.declared_inputs.len());
    let mut missing_inputs = Vec::new();
    for declared in &action.declared_inputs {
        match winning_supplied(declared.name.as_str(), supplied) {
            Some((origin, value)) => {
                resolved_inputs.push(resolved_input(declared, origin, value)?);
            }
            None if declared.required => {
                missing_inputs.push(missing_requirement(declared)?);
            }
            None => {}
        }
    }
    if resolved_inputs.len() > MAX_DECLARED_INPUTS || missing_inputs.len() > MAX_DECLARED_INPUTS {
        return Err(BmadInvocationError::BoundsViolation);
    }
    Ok((resolved_inputs, missing_inputs))
}

/// Picks the highest-precedence supplied value for one declared input name.
/// Headless never guesses: an input absent from `supplied` is left
/// unresolved here and turned into a `missingInputs` entry by the caller,
/// never silently filled from a heuristic or an empty value.
fn winning_supplied<'a>(
    name: &str,
    supplied: &'a [BmadInvocationSuppliedValue],
) -> Option<(BmadInvocationResolvedInputOrigin, &'a Value)> {
    ORIGIN_PRECEDENCE.into_iter().find_map(|origin| {
        supplied
            .iter()
            .find(|(candidate_name, candidate_origin, _)| {
                candidate_name == name && *candidate_origin == origin
            })
            .map(|(_, _, value)| (origin, value))
    })
}

fn resolved_input(
    declared: &BmadInvocationDeclaredInput,
    origin: BmadInvocationResolvedInputOrigin,
    value: &Value,
) -> Result<BmadInvocationResolvedInput, BmadInvocationError> {
    let name: BmadInvocationResolvedInputName = declared
        .name
        .as_str()
        .try_into()
        .map_err(|_| BmadInvocationError::InvalidValue)?;
    let value_hash = canonical_hash("bmad-invocation-input-value", 1, value)
        .map_err(|_| BmadInvocationError::Hash)?
        .to_string()
        .parse()
        .map_err(|_| BmadInvocationError::Hash)?;
    Ok(BmadInvocationResolvedInput {
        name,
        origin,
        value_hash,
    })
}

fn missing_requirement(
    declared: &BmadInvocationDeclaredInput,
) -> Result<BmadInvocationInputRequirement, BmadInvocationError> {
    if declared.allowed_values.len() > MAX_ALLOWED_VALUES {
        return Err(BmadInvocationError::BoundsViolation);
    }
    let name: BmadInvocationInputRequirementName = declared
        .name
        .as_str()
        .try_into()
        .map_err(|_| BmadInvocationError::InvalidValue)?;
    let code: BmadInvocationInputRequirementCode = "missing_required_input"
        .try_into()
        .map_err(|_| BmadInvocationError::InvalidValue)?;
    let prompt = declared
        .prompt
        .as_ref()
        .map(|prompt| {
            BmadInvocationInputRequirementPrompt::try_from(prompt.as_str())
                .map_err(|_| BmadInvocationError::InvalidValue)
        })
        .transpose()?;
    let allowed_values = declared
        .allowed_values
        .iter()
        .map(|item| {
            BmadInvocationInputRequirementAllowedValuesItem::try_from(item.as_str())
                .map_err(|_| BmadInvocationError::InvalidValue)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BmadInvocationInputRequirement {
        allowed_values,
        code,
        name,
        prompt,
        value_kind: convert_value_kind(declared.value_kind),
    })
}

const fn convert_value_kind(
    value_kind: BmadInvocationDeclaredInputValueKind,
) -> BmadInvocationInputRequirementValueKind {
    match value_kind {
        BmadInvocationDeclaredInputValueKind::String => {
            BmadInvocationInputRequirementValueKind::String
        }
        BmadInvocationDeclaredInputValueKind::Integer => {
            BmadInvocationInputRequirementValueKind::Integer
        }
        BmadInvocationDeclaredInputValueKind::Boolean => {
            BmadInvocationInputRequirementValueKind::Boolean
        }
        BmadInvocationDeclaredInputValueKind::Path => BmadInvocationInputRequirementValueKind::Path,
        BmadInvocationDeclaredInputValueKind::Enum => BmadInvocationInputRequirementValueKind::Enum,
        BmadInvocationDeclaredInputValueKind::Reference => {
            BmadInvocationInputRequirementValueKind::Reference
        }
    }
}
