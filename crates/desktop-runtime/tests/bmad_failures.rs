//! Behavioral coverage for cancellation, failure, retry, and repair
//! (ENG-13): `crates/desktop-runtime/src/bmad/failure.rs`.

use desktop_runtime::{
    build_problem,
    generated_contracts::{BmadProblemFailureClass, BmadProblemRecoveryActionAction},
    sha256_bytes, BmadFailureCancellation, BmadFailureError, BmadFailureOperation,
    BmadFailureRequest,
};

const SESSION_ID: &str = "bsn_0123456789ABCDEFGHJKMNPQ";
const PROBLEM_ID: &str = "bpr_0123456789ABCDEFGHJKMNPQ";

fn operation(step_id: &str, input_seed: u8) -> BmadFailureOperation {
    BmadFailureOperation {
        session_id: SESSION_ID.to_owned(),
        step_id: step_id.to_owned(),
        input_hash: sha256_bytes(&[input_seed]),
    }
}

fn request(class: BmadProblemFailureClass) -> BmadFailureRequest {
    let transient = matches!(
        class,
        BmadProblemFailureClass::Model
            | BmadProblemFailureClass::Tool
            | BmadProblemFailureClass::Store
            | BmadProblemFailureClass::External
    );
    BmadFailureRequest {
        session_id: SESSION_ID.to_owned(),
        problem_id: PROBLEM_ID.to_owned(),
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

// Timeout: a `tool` failure is transient and, within budget, offers exactly
// `retry`, with the same retry identity on a second attempt so a downstream
// store recognises the redelivery instead of double-applying it.
#[test]
fn tool_timeout_is_retry_eligible_and_shares_identity_across_attempts() {
    let mut second_attempt = request(BmadProblemFailureClass::Tool);
    second_attempt.attempt = 2;

    let first = build_problem(&request(BmadProblemFailureClass::Tool)).expect("attempt 1");
    let second = build_problem(&second_attempt).expect("attempt 2");

    assert!(first.retry.eligible);
    assert_eq!(first.retry.retry_identity, second.retry.retry_identity);
    assert_eq!(first.recovery_actions.len(), 1);
    assert_eq!(
        first.recovery_actions[0].action,
        BmadProblemRecoveryActionAction::Retry
    );
}

// Store conflict: `store` is transient, behaves like `tool`.
#[test]
fn store_conflict_is_retry_eligible() {
    let problem = build_problem(&request(BmadProblemFailureClass::Store)).expect("builds");
    assert!(problem.retry.eligible);
}

// Process kill: modelled as a transient failure whose attempt has already
// exhausted its budget. Retry must flip ineligible and abandon/rollback must
// be offered in its place.
#[test]
fn process_kill_style_budget_exhaustion_offers_abandon_and_rollback() {
    let mut exhausted = request(BmadProblemFailureClass::Tool);
    exhausted.attempt = 3;
    exhausted.max_attempts = 3;

    let problem = build_problem(&exhausted).expect("builds");

    assert!(!problem.retry.eligible);
    let actions: Vec<_> = problem
        .recovery_actions
        .iter()
        .map(|action| action.action)
        .collect();
    assert!(actions.contains(&BmadProblemRecoveryActionAction::Abandon));
    assert!(actions.contains(&BmadProblemRecoveryActionAction::Rollback));
}

// Invalid output: `validation` is never retry-eligible and offers exactly
// `repair`.
#[test]
fn invalid_output_offers_repair_only() {
    let problem = build_problem(&request(BmadProblemFailureClass::Validation)).expect("builds");
    assert!(!problem.retry.eligible);
    assert_eq!(problem.recovery_actions.len(), 1);
    assert_eq!(
        problem.recovery_actions[0].action,
        BmadProblemRecoveryActionAction::Repair
    );
}

// Stale authority: `authority` is never retry-eligible and offers exactly
// `reauthorize`.
#[test]
fn stale_authority_offers_reauthorize_only() {
    let problem = build_problem(&request(BmadProblemFailureClass::Authority)).expect("builds");
    assert!(!problem.retry.eligible);
    assert_eq!(problem.recovery_actions.len(), 1);
    assert_eq!(
        problem.recovery_actions[0].action,
        BmadProblemRecoveryActionAction::Reauthorize
    );
}

// Cancel: never retry-eligible, offers resume while unacknowledged.
#[test]
fn cancel_is_never_retry_eligible_and_offers_resume() {
    let problem = build_problem(&request(BmadProblemFailureClass::Cancel)).expect("builds");
    assert!(!problem.retry.eligible);
    assert!(problem
        .recovery_actions
        .iter()
        .all(|action| action.action != BmadProblemRecoveryActionAction::Retry));
    assert!(problem.recovery_actions.iter().any(|action| action.action
        == BmadProblemRecoveryActionAction::Resume
        && action.available));
}

// Acceptance 2: the same operation across two attempts yields the same
// retry identity; a different operation differs.
#[test]
fn retry_identity_is_stable_per_operation_and_differs_across_operations() {
    let same_operation_attempt_two = {
        let mut request = request(BmadProblemFailureClass::External);
        request.attempt = 2;
        request
    };
    let different_step = {
        let mut request = request(BmadProblemFailureClass::External);
        request.operation = operation("step_two", 1);
        request
    };

    let baseline = build_problem(&request(BmadProblemFailureClass::External)).expect("baseline");
    let retried = build_problem(&same_operation_attempt_two).expect("retried");
    let different = build_problem(&different_step).expect("different operation");

    assert_eq!(baseline.retry.retry_identity, retried.retry.retry_identity);
    assert_ne!(
        baseline.retry.retry_identity,
        different.retry.retry_identity
    );
}

// Acceptance 2 (eligibility side): every one of the nine failure classes has
// the correct retry eligibility.
#[test]
fn retry_eligibility_is_correct_for_all_nine_failure_classes() {
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
        assert!(
            build_problem(&request(class))
                .expect("builds")
                .retry
                .eligible
        );
    }
    for class in not_transient {
        assert!(
            !build_problem(&request(class))
                .expect("builds")
                .retry
                .eligible
        );
    }
}

// Acceptance 1: a transient problem claiming to have discarded drafts or
// accepted inputs is rejected rather than silently rewinding.
#[test]
fn transient_problem_discarding_preserved_state_is_rejected() {
    let mut discarded_drafts = request(BmadProblemFailureClass::Model);
    discarded_drafts.drafts_preserved = false;
    assert_eq!(
        build_problem(&discarded_drafts).unwrap_err(),
        BmadFailureError::TransientStateDiscarded
    );

    let mut discarded_inputs = request(BmadProblemFailureClass::Model);
    discarded_inputs.accepted_inputs_preserved = false;
    assert_eq!(
        build_problem(&discarded_inputs).unwrap_err(),
        BmadFailureError::TransientStateDiscarded
    );
}

// An acknowledgement sequence earlier than its request sequence is rejected.
#[test]
fn acknowledgement_before_request_is_rejected() {
    let mut cancelled = request(BmadProblemFailureClass::Cancel);
    cancelled.cancellation = Some(BmadFailureCancellation {
        requested_at_sequence: 10,
        acknowledged: true,
        acknowledged_at_sequence: Some(9),
    });
    assert_eq!(
        build_problem(&cancelled).unwrap_err(),
        BmadFailureError::AcknowledgementBeforeRequest
    );
}

// Acceptance 3: the recovery-action mapping is exhaustive and specific
// across all nine classes, never a fixed menu.
#[test]
fn recovery_action_mapping_is_exhaustive_and_specific() {
    let expectations: &[(BmadProblemFailureClass, BmadProblemRecoveryActionAction)] = &[
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
        (
            BmadProblemFailureClass::Model,
            BmadProblemRecoveryActionAction::Retry,
        ),
        (
            BmadProblemFailureClass::Tool,
            BmadProblemRecoveryActionAction::Retry,
        ),
        (
            BmadProblemFailureClass::Store,
            BmadProblemRecoveryActionAction::Retry,
        ),
        (
            BmadProblemFailureClass::External,
            BmadProblemRecoveryActionAction::Retry,
        ),
    ];
    for (class, expected) in expectations {
        let problem = build_problem(&request(*class)).expect("builds");
        assert!(problem
            .recovery_actions
            .iter()
            .any(|action| action.action == *expected && action.available));
    }

    let source = build_problem(&request(BmadProblemFailureClass::Source)).expect("builds");
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

    let cancel = build_problem(&request(BmadProblemFailureClass::Cancel)).expect("builds");
    let cancel_actions: Vec<_> = cancel
        .recovery_actions
        .iter()
        .map(|action| action.action)
        .collect();
    assert_eq!(
        cancel_actions,
        vec![
            BmadProblemRecoveryActionAction::Resume,
            BmadProblemRecoveryActionAction::Abandon,
        ]
    );
}

// Exceeding `maxAttempts` flips `eligible` false and offers abandon/rollback
// instead of retry.
#[test]
fn exceeding_max_attempts_flips_eligible_false() {
    let mut exhausted = request(BmadProblemFailureClass::External);
    exhausted.attempt = 3;
    exhausted.max_attempts = 3;
    let problem = build_problem(&exhausted).expect("builds");
    assert!(!problem.retry.eligible);
    let retry = problem
        .recovery_actions
        .iter()
        .find(|action| action.action == BmadProblemRecoveryActionAction::Retry)
        .expect("retry still listed, unavailable");
    assert!(!retry.available);
    assert!(retry.reason.is_some());
}

// Round trip: a problem must survive a JSON encode/decode without losing the
// fields that matter for replay and recovery.
#[test]
fn problem_round_trips_through_json() {
    let problem = build_problem(&request(BmadProblemFailureClass::Tool)).expect("builds");
    let value = serde_json::to_value(&problem).expect("serializes");
    let restored: desktop_runtime::generated_contracts::BmadProblem =
        serde_json::from_value(value).expect("deserializes");

    assert_eq!(restored.retry.retry_identity, problem.retry.retry_identity);
    assert_eq!(restored.retry.eligible, problem.retry.eligible);
    assert_eq!(restored.failure_class, problem.failure_class);
    assert_eq!(
        restored.preserved_state.drafts_preserved,
        problem.preserved_state.drafts_preserved
    );
}
