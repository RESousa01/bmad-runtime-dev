#![allow(clippy::expect_used)]

//! Integration coverage for the BMAD artifact lifecycle through its public
//! host-facing port (`BmadRuntimePort`), rather than the crate-internal unit
//! tests in `bmad::artifacts`. This is what a host composition root actually
//! calls, so a break here is a break a caller would hit.

use desktop_runtime::generated_contracts::{
    BmadArtifact, BmadArtifactCompletionHookRunHook, BmadArtifactCompletionHookRunStatus,
    BmadArtifactState,
};
use desktop_runtime::{
    BmadArtifactIntent, BmadArtifactTransitionError, BmadDomainRuntime, BmadRuntimePort,
};
use serde_json::json;

fn draft_artifact() -> BmadArtifact {
    serde_json::from_value(json!({
        "artifactId": "artifact_01J00000000000000000000000",
        "contentHash": format!("sha256:{}", "1".repeat(64)),
        "evidenceHash": format!("sha256:{}", "2".repeat(64)),
        "kind": "document",
        "lineage": {
            "sourceId": "source_01J00000000000000000000000",
            "templateHash": format!("sha256:{}", "3".repeat(64)),
            "inputsHash": format!("sha256:{}", "4".repeat(64))
        },
        "revision": 1,
        "schemaVersion": "sapphirus.bmad-artifact.v1",
        "sessionId": "session_01J00000000000000000000000",
        "state": "draft",
        "validation": {
            "declaredChecks": ["schema_ok", "links_resolve"],
            "passedChecks": [],
            "status": "not_run"
        }
    }))
    .expect("generated artifact")
}

/// Acceptance criterion 2: narrative prose cannot mark completion. Neither a
/// bare `Approve`/`Write` on an unvalidated artifact, nor completion-claiming
/// text anywhere in the contract, can substitute for declared checks
/// actually passing through `Validate`.
#[test]
fn prose_and_skipped_steps_cannot_reach_written_without_declared_checks_passing() {
    let runtime = BmadDomainRuntime;

    // Skipping straight to Approve/Write without validating is refused.
    let mut skipping = draft_artifact();
    let approve_result = runtime.apply_artifact_intent(&mut skipping, BmadArtifactIntent::Approve);
    assert!(matches!(
        approve_result,
        Err(BmadArtifactTransitionError::InvalidTransition { .. })
    ));
    assert_eq!(skipping.state, BmadArtifactState::Draft);

    // Prose asserting completion in the one free-text surface on the
    // contract does not help: validation still requires the exact declared
    // checks to be supplied as evidence.
    let mut lying = draft_artifact();
    lying.media_type = "note: all checks passed, this is complete".parse().ok();
    let validate_result = runtime.apply_artifact_intent(
        &mut lying,
        BmadArtifactIntent::Validate {
            passed_checks: vec![],
        },
    );
    assert_eq!(
        validate_result,
        Err(BmadArtifactTransitionError::ValidationIncomplete)
    );
    assert_eq!(lying.state, BmadArtifactState::Draft);
}

/// The full governed path: validate with exact evidence, approve (which
/// re-checks rather than trusting the prior step), write, then record a
/// completion hook — refused before `written`, idempotent afterward.
#[test]
fn full_lifecycle_reaches_written_and_records_an_idempotent_completion_hook() {
    let runtime = BmadDomainRuntime;
    let mut artifact = draft_artifact();

    // A completion hook cannot be recorded before a governed write exists.
    let too_early = runtime.apply_artifact_intent(
        &mut artifact,
        BmadArtifactIntent::RecordCompletionHook {
            hook: BmadArtifactCompletionHookRunHook::EmitReceipt,
            status: BmadArtifactCompletionHookRunStatus::Succeeded,
            receipt_hash: None,
        },
    );
    assert_eq!(
        too_early,
        Err(BmadArtifactTransitionError::ArtifactNotWritten)
    );

    runtime
        .apply_artifact_intent(
            &mut artifact,
            BmadArtifactIntent::Validate {
                passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
            },
        )
        .expect("declared checks fully covered");
    assert_eq!(artifact.state, BmadArtifactState::Validated);

    runtime
        .apply_artifact_intent(&mut artifact, BmadArtifactIntent::Approve)
        .expect("approval re-verifies the validation block and accepts it");
    assert_eq!(artifact.state, BmadArtifactState::Approved);

    runtime
        .apply_artifact_intent(&mut artifact, BmadArtifactIntent::Write)
        .expect("approved artifacts may be written");
    assert_eq!(artifact.state, BmadArtifactState::Written);

    for status in [
        BmadArtifactCompletionHookRunStatus::Pending,
        BmadArtifactCompletionHookRunStatus::Succeeded,
    ] {
        runtime
            .apply_artifact_intent(
                &mut artifact,
                BmadArtifactIntent::RecordCompletionHook {
                    hook: BmadArtifactCompletionHookRunHook::EmitReceipt,
                    status,
                    receipt_hash: None,
                },
            )
            .expect("hook recording is accepted once written");
    }
    // Idempotent: the redelivered hook replaced the earlier run rather than
    // appending a duplicate.
    assert_eq!(artifact.completion_hooks.len(), 1);
    assert_eq!(
        artifact.completion_hooks[0].status,
        BmadArtifactCompletionHookRunStatus::Succeeded
    );
}

/// Terminal states accept nothing further, and a refused intent leaves the
/// artifact byte-identical to before the call.
#[test]
fn terminal_states_refuse_every_further_intent_without_mutating_the_artifact() {
    let runtime = BmadDomainRuntime;
    let mut artifact = draft_artifact();
    runtime
        .apply_artifact_intent(&mut artifact, BmadArtifactIntent::Reject)
        .expect("draft may be rejected");
    assert_eq!(artifact.state, BmadArtifactState::Rejected);

    let before = serde_json::to_value(&artifact).expect("serialize");
    let refused = runtime.apply_artifact_intent(&mut artifact, BmadArtifactIntent::Approve);
    assert!(matches!(
        refused,
        Err(BmadArtifactTransitionError::InvalidTransition { .. })
    ));
    let after = serde_json::to_value(&artifact).expect("serialize");
    assert_eq!(
        before, after,
        "a refused intent must not mutate the artifact"
    );
}
