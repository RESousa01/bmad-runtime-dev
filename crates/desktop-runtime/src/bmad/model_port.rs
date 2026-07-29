//! The governed model port (ENG-08).
//!
//! Without this module a model response could be interpreted as anything the
//! caller hoped for and could, by construction, come back carrying more
//! authority than the plan that asked for it. Every response here crosses
//! exactly one boundary: [`accept_model_response`]. It only accepts a
//! response that (1) answers a `model_call` plan whose declared
//! `authorityClass` is `none`, (2) validates against the plan's declared
//! closed `outputSchemaId` by deserializing into that schema's own Rust type
//! with unknown fields rejected, and (3) stays inside the plan's budget. The
//! function returns the typed, schema-bound value only — never the plan, and
//! never a value with an authority field of its own — so accepting a
//! response can never widen what its plan was allowed to do.

use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::generated_contracts::{
    BmadActionPlan, BmadActionPlanAuthorityClass, BmadActionPlanKind,
};

/// Why a raw model response, retry, or repair was refused.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadModelPortError {
    /// The response failed its closed output schema.
    #[error("model response failed its closed output schema")]
    SchemaRejected,
    /// The plan is not a `model_call` plan, or does not carry `none`
    /// authority, so no response may be accepted against it at all.
    #[error("plan is not an eligible model_call target")]
    NotEligible,
    /// The plan's declared `outputSchemaId` does not match the schema the
    /// caller is trying to validate against.
    #[error("response does not target the plan's declared output schema")]
    SchemaMismatch,
    /// A call, token, retry, or repair budget was exhausted.
    #[error("model call budget exhausted")]
    BudgetExceeded,
}

/// Running counters checked against one plan's [`BmadActionPlanBudget`].
///
/// The caller owns the value across a plan's lifetime; every accepted call,
/// retry, or repair mutates it in place so a second attempt is checked
/// against the same running total, not a fresh one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BmadModelCallUsage {
    pub calls: i64,
    pub tokens: i64,
    pub retries: i64,
    pub repairs: i64,
}

/// A closed output schema a `model_call` plan may declare.
///
/// Implementors must reject unknown fields (`#[serde(deny_unknown_fields)]`)
/// so a response cannot smuggle extra, unvalidated content through as an
/// accepted value.
pub trait BmadClosedOutputSchema: DeserializeOwned {
    /// The exact `outputSchemaId` string this type accepts.
    fn schema_id() -> &'static str;
}

/// Validates and accepts one raw model response against a plan.
///
/// This is the only place a model response crosses into typed data. It
/// never returns the plan and never returns anything carrying its own
/// authority field, so a caller holding only the returned `T` cannot infer,
/// let alone exercise, more authority than the plan declared.
///
/// # Errors
///
/// Returns [`BmadModelPortError::NotEligible`] when the plan is not a
/// `model_call` plan with `none` authority,
/// [`BmadModelPortError::SchemaMismatch`] when `T::schema_id()` does not
/// match the plan's declared `outputSchemaId`,
/// [`BmadModelPortError::BudgetExceeded`] when accepting this call would
/// exceed the plan's call or token budget, and
/// [`BmadModelPortError::SchemaRejected`] when the raw response does not
/// parse as `T`.
pub fn accept_model_response<T: BmadClosedOutputSchema>(
    plan: &BmadActionPlan,
    raw_response: &Value,
    tokens_used: i64,
    usage: &mut BmadModelCallUsage,
) -> Result<T, BmadModelPortError> {
    let model_call = eligible_model_call(plan)?;
    if model_call.output_schema_id.as_str() != T::schema_id() {
        return Err(BmadModelPortError::SchemaMismatch);
    }
    check_call_budget(plan, usage, tokens_used)?;
    let accepted: T = serde_json::from_value(raw_response.clone())
        .map_err(|_| BmadModelPortError::SchemaRejected)?;
    usage.calls += 1;
    usage.tokens += tokens_used;
    Ok(accepted)
}

/// Records one retry against the plan's `maxRetries` budget.
///
/// # Errors
///
/// Returns [`BmadModelPortError::NotEligible`] for a non-eligible plan and
/// [`BmadModelPortError::BudgetExceeded`] once `maxRetries` is reached.
pub fn record_retry(
    plan: &BmadActionPlan,
    usage: &mut BmadModelCallUsage,
) -> Result<(), BmadModelPortError> {
    eligible_model_call(plan)?;
    if usage.retries >= plan.budget.max_retries {
        return Err(BmadModelPortError::BudgetExceeded);
    }
    usage.retries += 1;
    Ok(())
}

/// Records one repair attempt against the plan's `maxRepairs` budget.
///
/// # Errors
///
/// Returns [`BmadModelPortError::NotEligible`] for a non-eligible plan and
/// [`BmadModelPortError::BudgetExceeded`] once `maxRepairs` is reached.
pub fn record_repair(
    plan: &BmadActionPlan,
    usage: &mut BmadModelCallUsage,
) -> Result<(), BmadModelPortError> {
    eligible_model_call(plan)?;
    if usage.repairs >= plan.budget.max_repairs {
        return Err(BmadModelPortError::BudgetExceeded);
    }
    usage.repairs += 1;
    Ok(())
}

/// A plan is eligible to accept a model response only when its kind and
/// authority both say `model_call`/`none`. `authorityClass` is re-checked
/// here rather than trusted, so a response can never be accepted against a
/// plan whose authority was raised or corrupted after construction.
fn eligible_model_call(
    plan: &BmadActionPlan,
) -> Result<&crate::generated_contracts::BmadActionPlanModelCall, BmadModelPortError> {
    if plan.kind != BmadActionPlanKind::ModelCall
        || plan.authority_class != BmadActionPlanAuthorityClass::None
    {
        return Err(BmadModelPortError::NotEligible);
    }
    plan.model_call
        .as_ref()
        .ok_or(BmadModelPortError::NotEligible)
}

fn check_call_budget(
    plan: &BmadActionPlan,
    usage: &BmadModelCallUsage,
    tokens_used: i64,
) -> Result<(), BmadModelPortError> {
    if usage.calls + 1 > plan.budget.max_model_calls
        || usage.tokens + tokens_used > plan.budget.max_tokens
    {
        return Err(BmadModelPortError::BudgetExceeded);
    }
    Ok(())
}
