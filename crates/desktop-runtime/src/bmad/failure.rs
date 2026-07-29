//! Cancellation, failure, retry, and repair (ENG-13).
//!
//! Without this module an interrupted Method or Builder step has no typed
//! account of what happened: nothing distinguishes a step that is safe to
//! retry from one that needs a human to supply new input or a fresh grant,
//! and nothing stops a "transient" failure from quietly discarding a user's
//! draft or accepted answers. [`build_problem`] is the single place a
//! [`BmadProblem`] gets built, and it enforces three things no caller can
//! opt out of:
//!
//! - `retryIdentity` hashes the *attempted operation* — session, step, and
//!   input — never the attempt number or a timestamp, so a downstream
//!   effect/artifact/eval/install store can recognise attempt two of the
//!   same operation as a retry of attempt one and refuse to double-apply it.
//! - Retry eligibility is derived from `failureClass` alone. A caller cannot
//!   mark `source`, `input`, `validation`, or `authority` retryable, because
//!   retrying cannot fix a bad declaration or a missing grant; those need
//!   `supply_input`, `reauthorize`, or `repair` instead. `cancel` is never
//!   retryable.
//! - A transient failure that claims to have discarded a draft or an
//!   accepted input is rejected before a [`BmadProblem`] is ever produced —
//!   that combination is the exact silent-rewind defect this module exists
//!   to prevent.

use serde_json::json;

use crate::generated_contracts::{
    BmadProblem, BmadProblemCancellation, BmadProblemCode, BmadProblemDetail,
    BmadProblemFailureClass, BmadProblemPreservedState, BmadProblemRecoveryAction,
    BmadProblemRecoveryActionAction, BmadProblemRecoveryActionReason, BmadProblemRetry,
    CommonContractId, CommonSha256,
};
use crate::{canonical_hash, Sha256Digest};

/// The contract's shared bound for `attempt`, `maxAttempts`, and both
/// cancellation sequence numbers.
const MAX_ATTEMPT_BOUND: i64 = 64;
const MAX_SEQUENCE_BOUND: i64 = 1_048_576;

/// Identity of the attempted operation for retry-identity hashing.
///
/// Deliberately excludes the attempt number and any timestamp: two attempts
/// of the same session/step/input must hash identically so a retry can be
/// recognised downstream, and a different operation must hash differently.
#[derive(Clone, Debug)]
pub struct BmadFailureOperation {
    pub session_id: String,
    pub step_id: String,
    pub input_hash: Sha256Digest,
}

/// A cancel request and its (possibly still-pending) acknowledgement.
#[derive(Clone, Copy, Debug)]
pub struct BmadFailureCancellation {
    pub requested_at_sequence: i64,
    pub acknowledged: bool,
    pub acknowledged_at_sequence: Option<i64>,
}

/// Everything needed to construct one durable [`BmadProblem`].
#[derive(Clone, Debug)]
pub struct BmadFailureRequest {
    pub session_id: String,
    pub problem_id: String,
    pub code: String,
    pub detail: Option<String>,
    pub failure_class: BmadProblemFailureClass,
    pub operation: BmadFailureOperation,
    pub attempt: i64,
    pub max_attempts: i64,
    pub drafts_preserved: bool,
    pub accepted_inputs_preserved: bool,
    pub state_hash: Sha256Digest,
    pub cancellation: Option<BmadFailureCancellation>,
}

/// Why a [`BmadProblem`] could not be constructed. Construction is pure and
/// either succeeds completely or leaves nothing behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadFailureError {
    /// The input violated its bounded contract.
    BoundsViolation,
    /// A session/problem identity did not match the contract's id pattern.
    InvalidIdentity,
    /// `code` did not match the contract's lowercase-snake pattern.
    InvalidCode,
    /// `detail` (or a derived recovery-action reason) was empty or exceeded
    /// its contract length bound.
    InvalidDetail,
    /// A transient failure claimed to have discarded a draft or an accepted
    /// input.
    TransientStateDiscarded,
    /// A `cancel`-class problem carried no cancellation record, or a
    /// non-cancel class carried one.
    CancellationMismatch,
    /// An acknowledgement sequence preceded its own request sequence.
    AcknowledgementBeforeRequest,
    /// Canonical hashing failed for an otherwise well-formed value.
    Hash,
}

/// Builds the durable, replay-safe [`BmadProblem`] for one failed attempt.
///
/// # Errors
///
/// Returns [`BmadFailureError`] when an identity, code, or detail fails its
/// contract pattern; when `attempt`/`maxAttempts`/a cancellation sequence
/// falls outside its bound; when a transient failure claims to have
/// discarded preserved state; when cancellation presence disagrees with
/// `failureClass`; when an acknowledgement precedes its own request; or when
/// canonical hashing fails.
pub fn build_problem(request: &BmadFailureRequest) -> Result<BmadProblem, BmadFailureError> {
    if !(0..=MAX_ATTEMPT_BOUND).contains(&request.attempt)
        || !(1..=MAX_ATTEMPT_BOUND).contains(&request.max_attempts)
    {
        return Err(BmadFailureError::BoundsViolation);
    }

    let session_id = parse_contract_id(&request.session_id)?;
    let problem_id = parse_contract_id(&request.problem_id)?;
    let code = request
        .code
        .parse::<BmadProblemCode>()
        .map_err(|_| BmadFailureError::InvalidCode)?;
    let detail = request
        .detail
        .as_deref()
        .map(str::parse::<BmadProblemDetail>)
        .transpose()
        .map_err(|_| BmadFailureError::InvalidDetail)?;

    let is_cancel_class = request.failure_class == BmadProblemFailureClass::Cancel;
    let cancellation = match (&request.cancellation, is_cancel_class) {
        (Some(record), true) => Some(build_cancellation(record)?),
        (None, false) => None,
        _ => return Err(BmadFailureError::CancellationMismatch),
    };

    let transient = class_is_transient(request.failure_class);
    if transient && !(request.drafts_preserved && request.accepted_inputs_preserved) {
        return Err(BmadFailureError::TransientStateDiscarded);
    }

    let budget_available = request.attempt < request.max_attempts;
    let eligible = transient && budget_available;
    let recovery_actions =
        recovery_actions(request.failure_class, eligible, cancellation.as_ref())?;

    Ok(BmadProblem {
        cancellation,
        code,
        detail,
        failure_class: request.failure_class,
        preserved_state: BmadProblemPreservedState {
            accepted_inputs_preserved: request.accepted_inputs_preserved,
            drafts_preserved: request.drafts_preserved,
            state_hash: sha256_contract(request.state_hash)?,
        },
        problem_id,
        recovery_actions,
        retry: BmadProblemRetry {
            attempt: request.attempt,
            eligible,
            max_attempts: request.max_attempts,
            retry_identity: operation_hash(&request.operation)?,
        },
        schema_version: json!("sapphirus.bmad-problem.v1"),
        session_id,
    })
}

/// Transient classes (`model`, `tool`, `store`, `external`) are the only
/// ones a retry can plausibly fix: the same call against the same model,
/// tool, store, or external system may simply succeed next time.
/// `source`/`input`/`validation`/`authority` failures describe a bad
/// declaration or a missing grant that retrying cannot change, and `cancel`
/// is a deliberate stop, never a fault to retry past.
const fn class_is_transient(class: BmadProblemFailureClass) -> bool {
    matches!(
        class,
        BmadProblemFailureClass::Model
            | BmadProblemFailureClass::Tool
            | BmadProblemFailureClass::Store
            | BmadProblemFailureClass::External
    )
}

fn build_cancellation(
    record: &BmadFailureCancellation,
) -> Result<BmadProblemCancellation, BmadFailureError> {
    if !(0..=MAX_SEQUENCE_BOUND).contains(&record.requested_at_sequence) {
        return Err(BmadFailureError::BoundsViolation);
    }
    if let Some(acknowledged_at) = record.acknowledged_at_sequence {
        if !(0..=MAX_SEQUENCE_BOUND).contains(&acknowledged_at) {
            return Err(BmadFailureError::BoundsViolation);
        }
        if acknowledged_at < record.requested_at_sequence {
            return Err(BmadFailureError::AcknowledgementBeforeRequest);
        }
    }
    Ok(BmadProblemCancellation {
        acknowledged: record.acknowledged,
        acknowledged_at_sequence: record.acknowledged_at_sequence,
        requested_at_sequence: record.requested_at_sequence,
    })
}

/// Derives the offered recovery actions from `failureClass` alone, so the
/// set a caller sees is never a fixed menu: `input` only ever offers
/// `supply_input`, `authority` only ever offers `reauthorize`, and so on.
/// Every action carries `available`, and an unavailable one always carries
/// a `reason`.
fn recovery_actions(
    class: BmadProblemFailureClass,
    eligible: bool,
    cancellation: Option<&BmadProblemCancellation>,
) -> Result<Vec<BmadProblemRecoveryAction>, BmadFailureError> {
    use BmadProblemRecoveryActionAction::{Abandon, Reauthorize, Repair, Resume, Retry, Rollback};

    match class {
        BmadProblemFailureClass::Source => Ok(vec![available(Repair), available(Abandon)]),
        BmadProblemFailureClass::Input => Ok(vec![available(
            BmadProblemRecoveryActionAction::SupplyInput,
        )]),
        BmadProblemFailureClass::Validation => Ok(vec![available(Repair)]),
        BmadProblemFailureClass::Authority => Ok(vec![available(Reauthorize)]),
        BmadProblemFailureClass::Model
        | BmadProblemFailureClass::Tool
        | BmadProblemFailureClass::Store
        | BmadProblemFailureClass::External => {
            if eligible {
                Ok(vec![available(Retry)])
            } else {
                Ok(vec![
                    unavailable(Retry, "retry budget exhausted")?,
                    available(Abandon),
                    available(Rollback),
                ])
            }
        }
        BmadProblemFailureClass::Cancel => {
            if cancellation.is_some_and(|record| record.acknowledged) {
                Ok(vec![
                    unavailable(Resume, "cancellation already acknowledged")?,
                    available(Abandon),
                ])
            } else {
                Ok(vec![available(Resume), available(Abandon)])
            }
        }
    }
}

const fn available(action: BmadProblemRecoveryActionAction) -> BmadProblemRecoveryAction {
    BmadProblemRecoveryAction {
        action,
        available: true,
        reason: None,
    }
}

fn unavailable(
    action: BmadProblemRecoveryActionAction,
    reason: &str,
) -> Result<BmadProblemRecoveryAction, BmadFailureError> {
    let reason = reason
        .parse::<BmadProblemRecoveryActionReason>()
        .map_err(|_| BmadFailureError::InvalidDetail)?;
    Ok(BmadProblemRecoveryAction {
        action,
        available: false,
        reason: Some(reason),
    })
}

fn operation_hash(operation: &BmadFailureOperation) -> Result<CommonSha256, BmadFailureError> {
    let digest = canonical_hash(
        "bmad-retry-identity",
        1,
        &json!({
            "sessionId": operation.session_id,
            "stepId": operation.step_id,
            "inputHash": operation.input_hash.to_string(),
        }),
    )
    .map_err(|_| BmadFailureError::Hash)?;
    sha256_contract(digest)
}

fn sha256_contract(digest: Sha256Digest) -> Result<CommonSha256, BmadFailureError> {
    digest
        .to_string()
        .parse()
        .map_err(|_| BmadFailureError::Hash)
}

fn parse_contract_id(value: &str) -> Result<CommonContractId, BmadFailureError> {
    value.parse().map_err(|_| BmadFailureError::InvalidIdentity)
}

#[cfg(test)]
mod tests {
    use super::{
        build_problem, class_is_transient, BmadFailureCancellation, BmadFailureError,
        BmadFailureOperation, BmadFailureRequest,
    };
    use crate::generated_contracts::{BmadProblemFailureClass, BmadProblemRecoveryActionAction};
    use crate::sha256_bytes;

    fn operation(step: &str, input: u8) -> BmadFailureOperation {
        BmadFailureOperation {
            session_id: "bsn_0123456789ABCDEFGHJKMNPQ".to_owned(),
            step_id: step.to_owned(),
            input_hash: sha256_bytes(&[input]),
        }
    }

    fn base_request(class: BmadProblemFailureClass) -> BmadFailureRequest {
        let transient = class_is_transient(class);
        BmadFailureRequest {
            session_id: "bsn_0123456789ABCDEFGHJKMNPQ".to_owned(),
            problem_id: "bpr_0123456789ABCDEFGHJKMNPQ".to_owned(),
            code: "step_failed".to_owned(),
            detail: None,
            failure_class: class,
            operation: operation("step_one", 1),
            attempt: 1,
            max_attempts: 3,
            drafts_preserved: transient,
            accepted_inputs_preserved: transient,
            state_hash: sha256_bytes(b"state"),
            cancellation: (class == BmadProblemFailureClass::Cancel).then_some(
                BmadFailureCancellation {
                    requested_at_sequence: 5,
                    acknowledged: false,
                    acknowledged_at_sequence: None,
                },
            ),
        }
    }

    // Break caught: a naive retry identity keyed on the attempt number would
    // give every retry a fresh identity, so a downstream store could never
    // recognise attempt two as a repeat of attempt one and would double
    // apply its effect.
    #[test]
    fn retry_identity_matches_across_attempts_of_the_same_operation() {
        let mut first = base_request(BmadProblemFailureClass::Model);
        first.attempt = 1;
        let mut second = base_request(BmadProblemFailureClass::Model);
        second.attempt = 2;

        let first = build_problem(&first).expect("attempt one builds");
        let second = build_problem(&second).expect("attempt two builds");

        assert_eq!(first.retry.retry_identity, second.retry.retry_identity);
    }

    // Break caught: if the identity did not bind session/step/input, two
    // unrelated operations could collide and a store would refuse to apply
    // the second one, believing it a duplicate of the first.
    #[test]
    fn retry_identity_differs_for_a_different_operation() {
        let same_step_other_input = {
            let mut request = base_request(BmadProblemFailureClass::Tool);
            request.operation = operation("step_one", 9);
            request
        };
        let other_step = {
            let mut request = base_request(BmadProblemFailureClass::Tool);
            request.operation = operation("step_two", 1);
            request
        };

        let baseline =
            build_problem(&base_request(BmadProblemFailureClass::Tool)).expect("baseline builds");
        let other_input = build_problem(&same_step_other_input).expect("other input builds");
        let other_step = build_problem(&other_step).expect("other step builds");

        assert_ne!(
            baseline.retry.retry_identity,
            other_input.retry.retry_identity
        );
        assert_ne!(
            baseline.retry.retry_identity,
            other_step.retry.retry_identity
        );
    }

    // Break caught: retry eligibility decided per-caller (rather than per
    // failure class) could let a caller mark a bad declaration "retryable"
    // when only a human editing the source can actually fix it.
    #[test]
    fn retry_eligibility_is_correct_for_every_failure_class() {
        let transient = [
            BmadProblemFailureClass::Model,
            BmadProblemFailureClass::Tool,
            BmadProblemFailureClass::Store,
            BmadProblemFailureClass::External,
        ];
        let not_transient = [
            BmadProblemFailureClass::Source,
            BmadProblemFailureClass::Input,
            BmadProblemFailureClass::Validation,
            BmadProblemFailureClass::Authority,
            BmadProblemFailureClass::Cancel,
        ];

        for class in transient {
            let problem = build_problem(&base_request(class)).expect("transient problem builds");
            assert!(problem.retry.eligible, "{class} must be retry-eligible");
        }
        for class in not_transient {
            let problem =
                build_problem(&base_request(class)).expect("non-transient problem builds");
            assert!(
                !problem.retry.eligible,
                "{class} must not be retry-eligible"
            );
        }
    }

    // Break caught: a transient problem that discards accepted input is the
    // exact silent-rewind defect acceptance criterion 1 exists to prevent.
    #[test]
    fn transient_problem_claiming_discarded_drafts_is_rejected() {
        let mut request = base_request(BmadProblemFailureClass::Store);
        request.drafts_preserved = false;
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::TransientStateDiscarded
        );

        let mut request = base_request(BmadProblemFailureClass::Store);
        request.accepted_inputs_preserved = false;
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::TransientStateDiscarded
        );
    }

    // Break caught: an acknowledgement stamped before its own request would
    // describe an acknowledgement that predates the thing it acknowledges.
    #[test]
    fn acknowledgement_before_its_request_is_rejected() {
        let mut request = base_request(BmadProblemFailureClass::Cancel);
        request.cancellation = Some(BmadFailureCancellation {
            requested_at_sequence: 10,
            acknowledged: true,
            acknowledged_at_sequence: Some(9),
        });
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::AcknowledgementBeforeRequest
        );
    }

    // Cancellation timeout scenario: a request that has not yet been
    // acknowledged offers resume/abandon, never retry, and stays ineligible.
    #[test]
    fn cancel_offers_resume_and_abandon_never_retry() {
        let problem =
            build_problem(&base_request(BmadProblemFailureClass::Cancel)).expect("cancel builds");
        assert!(!problem.retry.eligible);
        assert_eq!(problem.recovery_actions.len(), 2);
        assert!(problem
            .recovery_actions
            .iter()
            .all(|action| action.action != BmadProblemRecoveryActionAction::Retry));
        let resume = problem
            .recovery_actions
            .iter()
            .find(|action| action.action == BmadProblemRecoveryActionAction::Resume)
            .expect("resume offered");
        assert!(resume.available);
    }

    // Cancellation, acknowledged: resume is no longer offered as available.
    #[test]
    fn acknowledged_cancel_marks_resume_unavailable_with_a_reason() {
        let mut request = base_request(BmadProblemFailureClass::Cancel);
        request.cancellation = Some(BmadFailureCancellation {
            requested_at_sequence: 5,
            acknowledged: true,
            acknowledged_at_sequence: Some(6),
        });
        let problem = build_problem(&request).expect("acknowledged cancel builds");
        let resume = problem
            .recovery_actions
            .iter()
            .find(|action| action.action == BmadProblemRecoveryActionAction::Resume)
            .expect("resume offered");
        assert!(!resume.available);
        assert!(resume.reason.is_some());
    }

    // Timeout scenario: a `tool` failure (e.g. a step that timed out) is
    // transient and, within budget, offers exactly `retry`.
    #[test]
    fn timeout_style_tool_failure_offers_retry_within_budget() {
        let problem =
            build_problem(&base_request(BmadProblemFailureClass::Tool)).expect("tool builds");
        assert!(problem.retry.eligible);
        assert_eq!(problem.recovery_actions.len(), 1);
        assert_eq!(
            problem.recovery_actions[0].action,
            BmadProblemRecoveryActionAction::Retry
        );
        assert!(problem.recovery_actions[0].available);
    }

    // Store conflict scenario: a `store` failure behaves like any other
    // transient class.
    #[test]
    fn store_conflict_is_retry_eligible_within_budget() {
        let problem =
            build_problem(&base_request(BmadProblemFailureClass::Store)).expect("store builds");
        assert!(problem.retry.eligible);
    }

    // Process-kill scenario: modelled as a `tool` failure whose attempt has
    // already exhausted its budget; retry must flip ineligible and abandon
    // or rollback must be offered instead.
    #[test]
    fn exceeding_max_attempts_flips_eligible_false_and_offers_abandon_or_rollback() {
        let mut request = base_request(BmadProblemFailureClass::Tool);
        request.attempt = 3;
        request.max_attempts = 3;
        let problem = build_problem(&request).expect("exhausted-budget problem builds");

        assert!(!problem.retry.eligible);
        let actions: Vec<_> = problem
            .recovery_actions
            .iter()
            .map(|action| action.action)
            .collect();
        assert!(actions.contains(&BmadProblemRecoveryActionAction::Abandon));
        assert!(actions.contains(&BmadProblemRecoveryActionAction::Rollback));
        let retry = problem
            .recovery_actions
            .iter()
            .find(|action| action.action == BmadProblemRecoveryActionAction::Retry)
            .expect("retry still listed as unavailable");
        assert!(!retry.available);
        assert!(retry.reason.is_some());
    }

    // Invalid-output scenario: a `validation` failure is never retryable and
    // offers exactly `repair`, derived from validated evidence.
    #[test]
    fn invalid_output_validation_failure_offers_repair_only() {
        let problem = build_problem(&base_request(BmadProblemFailureClass::Validation))
            .expect("validation builds");
        assert!(!problem.retry.eligible);
        assert_eq!(problem.recovery_actions.len(), 1);
        assert_eq!(
            problem.recovery_actions[0].action,
            BmadProblemRecoveryActionAction::Repair
        );
    }

    // Stale-authority scenario: an `authority` failure is never retryable
    // and offers exactly `reauthorize`.
    #[test]
    fn stale_authority_failure_offers_reauthorize_only() {
        let problem = build_problem(&base_request(BmadProblemFailureClass::Authority))
            .expect("authority builds");
        assert!(!problem.retry.eligible);
        assert_eq!(problem.recovery_actions.len(), 1);
        assert_eq!(
            problem.recovery_actions[0].action,
            BmadProblemRecoveryActionAction::Reauthorize
        );
    }

    // Break caught: a fixed recovery-action menu (e.g. always offering
    // retry) would tell the UI to retry a bad declaration it can never fix.
    #[test]
    fn recovery_action_mapping_is_specific_for_every_failure_class() {
        let cases: &[(BmadProblemFailureClass, BmadProblemRecoveryActionAction)] = &[
            (
                BmadProblemFailureClass::Input,
                BmadProblemRecoveryActionAction::SupplyInput,
            ),
            (
                BmadProblemFailureClass::Authority,
                BmadProblemRecoveryActionAction::Reauthorize,
            ),
            (
                BmadProblemFailureClass::Validation,
                BmadProblemRecoveryActionAction::Repair,
            ),
        ];
        for (class, expected) in cases {
            let problem = build_problem(&base_request(*class)).expect("class-specific build");
            assert_eq!(problem.recovery_actions.len(), 1);
            assert_eq!(problem.recovery_actions[0].action, *expected);
            assert!(problem.recovery_actions[0].available);
        }

        let source =
            build_problem(&base_request(BmadProblemFailureClass::Source)).expect("source builds");
        let source_actions: Vec<_> = source
            .recovery_actions
            .iter()
            .map(|action| action.action)
            .collect();
        assert_eq!(
            source_actions,
            vec![
                BmadProblemRecoveryActionAction::Repair,
                BmadProblemRecoveryActionAction::Abandon,
            ]
        );
    }

    // Round trip: a problem must survive a JSON encode/decode unchanged in
    // the fields that matter for replay.
    #[test]
    fn problem_round_trips_through_json() {
        let problem =
            build_problem(&base_request(BmadProblemFailureClass::External)).expect("builds");
        let value = serde_json::to_value(&problem).expect("serializes");
        let restored: crate::generated_contracts::BmadProblem =
            serde_json::from_value(value).expect("deserializes");

        assert_eq!(restored.retry.retry_identity, problem.retry.retry_identity);
        assert_eq!(restored.retry.eligible, problem.retry.eligible);
        assert_eq!(restored.failure_class, problem.failure_class);
        assert_eq!(
            restored.preserved_state.drafts_preserved,
            problem.preserved_state.drafts_preserved
        );
        assert_eq!(
            restored.preserved_state.accepted_inputs_preserved,
            problem.preserved_state.accepted_inputs_preserved
        );
    }

    #[test]
    fn bounds_violation_rejects_out_of_range_attempts() {
        let mut request = base_request(BmadProblemFailureClass::Tool);
        request.attempt = 65;
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::BoundsViolation
        );
    }

    #[test]
    fn cancellation_record_required_only_for_cancel_class() {
        let mut request = base_request(BmadProblemFailureClass::Tool);
        request.cancellation = Some(BmadFailureCancellation {
            requested_at_sequence: 1,
            acknowledged: false,
            acknowledged_at_sequence: None,
        });
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::CancellationMismatch
        );

        let mut request = base_request(BmadProblemFailureClass::Cancel);
        request.cancellation = None;
        assert_eq!(
            build_problem(&request).unwrap_err(),
            BmadFailureError::CancellationMismatch
        );
    }
}
