//! Behavioral coverage for the canonical review and validation lenses
//! (ENG-12): `crates/desktop-runtime/src/bmad/review.rs`.

use desktop_runtime::{
    blocks_bmad_review_completion,
    generated_contracts::{
        BmadReviewResultDisposition, BmadReviewResultFindingSeverity,
        BmadReviewResultFindingVerificationState, BmadReviewResultTargetKind,
    },
    review_bmad, BmadReviewError, BmadReviewFindingInput, BmadReviewRequest, Sha256Digest,
};

const REVIEW_ID: &str = "review_01J00000000000000000000000";

fn content_hash(seed: u8) -> Sha256Digest {
    Sha256Digest::from_bytes([seed; 32])
}

fn finding(
    code: &str,
    severity: BmadReviewResultFindingSeverity,
    locator: &str,
) -> BmadReviewFindingInput {
    BmadReviewFindingInput {
        code: code.to_owned(),
        severity,
        verification_state: BmadReviewResultFindingVerificationState::Verified,
        evidence_locator: locator.to_owned(),
        evidence_content_hash: content_hash(1),
        evidence_line: Some(12),
        summary: None,
    }
}

fn request(lens_alias: &str) -> BmadReviewRequest {
    BmadReviewRequest {
        review_id: REVIEW_ID.to_owned(),
        lens_alias: lens_alias.to_owned(),
        target_kind: BmadReviewResultTargetKind::CodeChange,
        target_hash: content_hash(2),
        deterministic_findings: Vec::new(),
        model_findings: Vec::new(),
        required_evidence: Vec::new(),
        gaps_block_completion: false,
    }
}

// Break caught: a model finding claiming the same (code, locator) as a
// deterministic finding could otherwise overwrite or soften the fact a
// deterministic check already established.
#[test]
fn model_finding_cannot_overwrite_colliding_deterministic_finding() {
    let mut req = request("security");
    req.deterministic_findings.push(finding(
        "hardcoded_secret",
        BmadReviewResultFindingSeverity::Critical,
        "src/config.rs:10",
    ));
    let mut model_dupe = finding(
        "hardcoded_secret",
        BmadReviewResultFindingSeverity::Info,
        "src/config.rs:10",
    );
    model_dupe.verification_state = BmadReviewResultFindingVerificationState::Refuted;
    req.model_findings.push(model_dupe);

    let result = review_bmad(&req).expect("review builds");

    assert_eq!(result.deterministic_findings.len(), 1);
    assert!(
        result.model_findings.is_empty(),
        "model duplicate must be dropped, not merged"
    );
    assert_eq!(
        result.deterministic_findings[0].severity,
        BmadReviewResultFindingSeverity::Critical,
        "deterministic severity must not be softened by the model's colliding finding"
    );
    assert_eq!(
        result.deterministic_findings[0].verification_state,
        BmadReviewResultFindingVerificationState::Verified,
        "deterministic verification state must not be overwritten"
    );
}

// Break caught: two legacy names for the same review used to run different
// code paths and could silently diverge in output for identical input.
#[test]
fn two_legacy_aliases_for_one_lens_produce_byte_identical_results() {
    let mut req_a = request("sec");
    req_a.deterministic_findings.push(finding(
        "sql_injection_risk",
        BmadReviewResultFindingSeverity::High,
        "src/db.rs:42",
    ));
    let mut req_b = req_a.clone();
    req_b.lens_alias = "security-review".to_owned();

    let result_a = review_bmad(&req_a).expect("review builds");
    let result_b = review_bmad(&req_b).expect("review builds");

    let json_a = serde_json::to_value(&result_a).expect("serializes");
    let json_b = serde_json::to_value(&result_b).expect("serializes");
    assert_eq!(
        json_a, json_b,
        "alias convergence must produce identical results"
    );
    assert_eq!(result_a.result_hash, result_b.result_hash);
}

// Break caught: a finding with no evidence location would let a reviewer
// assert a defect with nothing a reader could go inspect.
#[test]
fn finding_without_evidence_locator_is_rejected() {
    let mut req = request("quality");
    req.deterministic_findings.push(BmadReviewFindingInput {
        code: "missing_test_coverage".to_owned(),
        severity: BmadReviewResultFindingSeverity::Medium,
        verification_state: BmadReviewResultFindingVerificationState::Unverified,
        evidence_locator: String::new(),
        evidence_content_hash: content_hash(3),
        evidence_line: None,
        summary: None,
    });

    assert!(matches!(
        review_bmad(&req),
        Err(BmadReviewError::MissingEvidence)
    ));
}

// Every finding that does carry evidence identifies both its lens (via the
// result) and its evidence location (via the finding).
#[test]
fn every_finding_identifies_its_lens_and_evidence_location() {
    let mut req = request("architecture");
    req.deterministic_findings.push(finding(
        "layering_violation",
        BmadReviewResultFindingSeverity::Low,
        "src/bmad/review.rs:100",
    ));

    let result = review_bmad(&req).expect("review builds");

    assert_eq!(
        result.lens,
        desktop_runtime::generated_contracts::BmadReviewResultLens::Architecture
    );
    let evidence = &result.deterministic_findings[0].evidence;
    assert_eq!(evidence.locator.to_string(), "src/bmad/review.rs:100");
}

// Break caught: a required evidence code with no supporting finding could
// otherwise pass review silently, letting completion proceed on a gap.
#[test]
fn missing_required_evidence_with_gaps_blocking_yields_blocked_and_refuses_completion() {
    let mut req = request("evidence_gap");
    req.required_evidence.push("test_run_recorded".to_owned());
    req.gaps_block_completion = true;

    let result = review_bmad(&req).expect("review builds");

    assert_eq!(result.disposition, BmadReviewResultDisposition::Blocked);
    assert!(result.verification.present_evidence.is_empty());
    assert!(
        blocks_bmad_review_completion(&result),
        "completion predicate must refuse"
    );
}

// Break caught: satisfied required evidence with a non-blocking configuration
// must not be treated as blocked just because findings exist.
#[test]
fn satisfied_required_evidence_does_not_block_completion() {
    let mut req = request("evidence_gap");
    req.required_evidence.push("test_run_recorded".to_owned());
    req.gaps_block_completion = true;
    req.deterministic_findings.push(finding(
        "test_run_recorded",
        BmadReviewResultFindingSeverity::Info,
        "ci/run-42.log",
    ));

    let result = review_bmad(&req).expect("review builds");

    assert_eq!(result.verification.present_evidence.len(), 1);
    assert_ne!(result.disposition, BmadReviewResultDisposition::Blocked);
}

// Break caught: a naive incrementing id scheme would make findingId depend
// on call order rather than content, so a rerun over identical input would
// silently renumber every finding and duplicates would never collapse.
#[test]
fn finding_ids_are_stable_across_reruns_and_duplicates_dedup() {
    let mut req = request("quality");
    req.deterministic_findings.push(finding(
        "unused_import",
        BmadReviewResultFindingSeverity::Low,
        "src/lib.rs:3",
    ));
    req.deterministic_findings.push(finding(
        "unused_import",
        BmadReviewResultFindingSeverity::Low,
        "src/lib.rs:3",
    ));

    let first = review_bmad(&req).expect("review builds");
    let second = review_bmad(&req).expect("review builds");

    assert_eq!(
        first.deterministic_findings.len(),
        1,
        "duplicate finding must collapse"
    );
    assert_eq!(
        first.deterministic_findings[0].finding_id, second.deterministic_findings[0].finding_id,
        "finding id must be a stable function of (lens, code, locator)"
    );
}

// All six target kinds the contract declares must be accepted by the engine.
#[test]
fn all_six_target_kinds_are_accepted() {
    let kinds = [
        BmadReviewResultTargetKind::Artifact,
        BmadReviewResultTargetKind::Skill,
        BmadReviewResultTargetKind::Module,
        BmadReviewResultTargetKind::Workflow,
        BmadReviewResultTargetKind::CodeChange,
        BmadReviewResultTargetKind::CompletionEvidence,
    ];
    for kind in kinds {
        let mut req = request("completeness");
        req.target_kind = kind;
        let result = review_bmad(&req).expect("review builds for every target kind");
        assert_eq!(result.target_kind, kind);
    }
}

// Round-trips one result through serde_json to confirm the contract's
// generated (de)serialization matches what the engine actually produces.
#[test]
fn result_round_trips_through_serde_json_value() {
    let mut req = request("eval_readiness");
    req.deterministic_findings.push(finding(
        "flaky_eval_seed",
        BmadReviewResultFindingSeverity::Medium,
        "evals/seed.rs:1",
    ));
    let result = review_bmad(&req).expect("review builds");

    let value = serde_json::to_value(&result).expect("serializes to value");
    let round_tripped: desktop_runtime::generated_contracts::BmadReviewResult =
        serde_json::from_value(value).expect("deserializes back from value");

    assert_eq!(round_tripped.result_hash, result.result_hash);
    assert_eq!(round_tripped.deterministic_findings.len(), 1);
}

// Break caught: an unrecognized alias silently mapping to a default lens
// would let a typo produce a review under the wrong lens instead of failing.
#[test]
fn unknown_lens_alias_is_rejected() {
    let req = request("not-a-real-lens");
    assert!(matches!(
        review_bmad(&req),
        Err(BmadReviewError::UnknownLensAlias)
    ));
}
