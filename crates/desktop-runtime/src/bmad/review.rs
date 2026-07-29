//! Canonical review and validation lenses (ENG-12).
//!
//! Before this module, "review" meant whatever prompt text a skill happened
//! to paste: a model could invent a finding, restate a deterministic check
//! in softer language, or quietly drop a severity a linter had already
//! proven. Here every review — Method's, a Builder analysis, a deterministic
//! validator, an evidence-gap check — funnels through one engine. Legacy
//! alias names converge on the same canonical lens, deterministic checks run
//! first and are folded in before any model finding is considered, and a
//! model finding that collides with a deterministic one on `(code, locator)`
//! is dropped rather than allowed to overwrite or soften it. Pure, no I/O:
//! the caller supplies findings already computed; this module only merges,
//! dedupes, and hashes them.

use std::{collections::BTreeSet, num::NonZeroU64};

use serde_json::Value;
use thiserror::Error;

use crate::generated_contracts::{
    BmadReviewResult, BmadReviewResultDisposition, BmadReviewResultEvidenceLocation,
    BmadReviewResultFinding, BmadReviewResultFindingOrigin, BmadReviewResultFindingSeverity,
    BmadReviewResultFindingVerificationState, BmadReviewResultLens, BmadReviewResultTargetKind,
    BmadReviewResultVerification, CommonContractId, CommonSha256,
};
use crate::{canonical_hash_without_field, sha256_bytes, Sha256Digest};

/// Contract-declared bound on each finding array.
const MAX_FINDINGS: usize = 512;
/// Contract-declared bound on the evidence-code arrays.
const MAX_EVIDENCE_CODES: usize = 64;
/// Contract-declared bound on an evidence location's line number.
const MAX_LINE: u64 = 16_777_216;

/// One finding as a caller (a validator, Method, or a model) reports it,
/// before it has a stable id or a place in a result.
#[derive(Clone, Debug)]
pub struct BmadReviewFindingInput {
    pub code: String,
    pub severity: BmadReviewResultFindingSeverity,
    pub verification_state: BmadReviewResultFindingVerificationState,
    pub evidence_locator: String,
    pub evidence_content_hash: Sha256Digest,
    pub evidence_line: Option<u64>,
    pub summary: Option<String>,
}

/// Everything one review run needs. No clock, no filesystem, no model port:
/// findings arrive pre-computed and this module only merges and hashes them.
#[derive(Clone, Debug)]
pub struct BmadReviewRequest {
    pub review_id: String,
    pub lens_alias: String,
    pub target_kind: BmadReviewResultTargetKind,
    pub target_hash: Sha256Digest,
    pub deterministic_findings: Vec<BmadReviewFindingInput>,
    pub model_findings: Vec<BmadReviewFindingInput>,
    pub required_evidence: Vec<String>,
    pub gaps_block_completion: bool,
}

/// Why a review request could not be turned into a result.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadReviewError {
    /// A findings or evidence-code array exceeded its contract bound.
    #[error("review request exceeds its bounded array size")]
    BoundsViolation,
    /// `review_id` or `target_hash` did not match its contract identity.
    #[error("review identity is invalid")]
    InvalidIdentity,
    /// `lens_alias` matched none of the closed set of known aliases.
    #[error("review lens alias is not recognized")]
    UnknownLensAlias,
    /// A finding's `code` did not match the contract's code pattern.
    #[error("review finding code is invalid")]
    InvalidCode,
    /// A finding carried no evidence location, or its locator was unsafe.
    #[error("review finding is missing a valid evidence location")]
    MissingEvidence,
    /// A finding's result content could not be canonically hashed.
    #[error("review result hash could not be computed")]
    Hash,
}

/// Resolves a legacy review alias onto its canonical lens. Two aliases for
/// the same lens must resolve identically so alias convergence (acceptance
/// 1) holds regardless of which spelling a caller used.
fn resolve_lens(alias: &str) -> Result<BmadReviewResultLens, BmadReviewError> {
    match alias.to_ascii_lowercase().as_str() {
        "security" | "sec" | "security-review" | "secreview" => Ok(BmadReviewResultLens::Security),
        "quality" | "qa" | "quality-review" | "qareview" => Ok(BmadReviewResultLens::Quality),
        "architecture" | "arch" | "architecture-review" | "archreview" => {
            Ok(BmadReviewResultLens::Architecture)
        }
        "completeness" | "complete" | "completeness-review" => {
            Ok(BmadReviewResultLens::Completeness)
        }
        "evidence_gap" | "evidence-gap" | "evidencegap" | "evidence-gap-review" => {
            Ok(BmadReviewResultLens::EvidenceGap)
        }
        "eval_readiness" | "eval-readiness" | "evalreadiness" | "eval-readiness-review" => {
            Ok(BmadReviewResultLens::EvalReadiness)
        }
        _ => Err(BmadReviewError::UnknownLensAlias),
    }
}

/// Runs one review: resolves the lens, folds in deterministic findings
/// first, then merges model findings without letting any of them overwrite
/// or soften a deterministic collision, and derives verification and
/// disposition from the merged set.
///
/// # Errors
///
/// Returns [`BmadReviewError`] for an unrecognized lens alias, an invalid
/// `review_id`/`target_hash`, a bounds violation, an invalid finding code,
/// a finding missing a valid evidence location, or a hashing failure.
pub fn review(request: &BmadReviewRequest) -> Result<BmadReviewResult, BmadReviewError> {
    if request.deterministic_findings.len() > MAX_FINDINGS
        || request.model_findings.len() > MAX_FINDINGS
        || request.required_evidence.len() > MAX_EVIDENCE_CODES
    {
        return Err(BmadReviewError::BoundsViolation);
    }
    let lens = resolve_lens(&request.lens_alias)?;
    let review_id: CommonContractId = request
        .review_id
        .parse()
        .map_err(|_| BmadReviewError::InvalidIdentity)?;
    let target_hash = sha256_contract(request.target_hash)?;

    let mut claimed: BTreeSet<(String, String)> = BTreeSet::new();
    let deterministic_findings = fold_findings(
        lens,
        BmadReviewResultFindingOrigin::Deterministic,
        &request.deterministic_findings,
        &mut claimed,
    )?;
    let model_findings = fold_findings(
        lens,
        BmadReviewResultFindingOrigin::Model,
        &request.model_findings,
        &mut claimed,
    )?;

    let all_codes: BTreeSet<&str> = deterministic_findings
        .iter()
        .chain(&model_findings)
        .map(|finding| finding.code.as_str())
        .collect();
    let verification = build_verification(
        &request.required_evidence,
        &all_codes,
        request.gaps_block_completion,
    )?;
    let disposition = derive_disposition(&deterministic_findings, &all_codes, &verification);

    let mut result = BmadReviewResult {
        deterministic_findings,
        disposition,
        lens,
        model_findings,
        result_hash: sha256_contract(Sha256Digest::from_bytes([0; 32]))?,
        review_id,
        schema_version: Value::String("sapphirus.bmad-review-result.v1".to_owned()),
        target_hash,
        target_kind: request.target_kind,
        verification,
    };
    result.result_hash = sha256_contract(content_hash(&result)?)?;
    Ok(result)
}

/// A caller refuses completion when this returns `true`. The predicate
/// exists so "does this review block completion" is decided in one place
/// rather than re-derived, and possibly re-derived wrong, at every call
/// site.
#[must_use]
pub fn blocks_completion(result: &BmadReviewResult) -> bool {
    result.disposition == BmadReviewResultDisposition::Blocked
}

/// Builds each input into a contract finding, skipping (never overwriting)
/// any whose `(code, locator)` was already claimed by an earlier fold —
/// deterministic findings are folded before model findings, so this is the
/// mechanism that keeps a model finding from overwriting or downgrading a
/// deterministic one, and that dedupes repeats within either origin.
fn fold_findings(
    lens: BmadReviewResultLens,
    origin: BmadReviewResultFindingOrigin,
    inputs: &[BmadReviewFindingInput],
    claimed: &mut BTreeSet<(String, String)>,
) -> Result<Vec<BmadReviewResultFinding>, BmadReviewError> {
    let mut findings = Vec::with_capacity(inputs.len());
    for input in inputs {
        let key = (input.code.clone(), input.evidence_locator.clone());
        if claimed.contains(&key) {
            continue;
        }
        let finding = build_finding(lens, origin, input)?;
        claimed.insert(key);
        findings.push(finding);
    }
    Ok(findings)
}

/// Builds one contract finding, deriving its `findingId` as a stable
/// function of `(lens, code, locator)` rather than a counter, so a rerun of
/// the same review over the same input produces the same id.
fn build_finding(
    lens: BmadReviewResultLens,
    origin: BmadReviewResultFindingOrigin,
    input: &BmadReviewFindingInput,
) -> Result<BmadReviewResultFinding, BmadReviewError> {
    if input.evidence_locator.is_empty() {
        return Err(BmadReviewError::MissingEvidence);
    }
    let locator = input
        .evidence_locator
        .parse()
        .map_err(|_| BmadReviewError::MissingEvidence)?;
    let code = input
        .code
        .parse()
        .map_err(|_| BmadReviewError::InvalidCode)?;
    let line = match input.evidence_line {
        None => None,
        Some(0) => return Err(BmadReviewError::BoundsViolation),
        Some(value) if value > MAX_LINE => return Err(BmadReviewError::BoundsViolation),
        Some(value) => NonZeroU64::new(value),
    };
    let summary = match input.summary.as_deref() {
        None | Some("") => None,
        Some(text) => Some(text.parse().map_err(|_| BmadReviewError::BoundsViolation)?),
    };
    let seed = format!("{lens}|{}|{}", input.code, input.evidence_locator);
    let finding_id = format!("f_{}", &sha256_bytes(seed.as_bytes()).hex_value()[..24])
        .parse()
        .map_err(|_| BmadReviewError::Hash)?;
    Ok(BmadReviewResultFinding {
        code,
        evidence: BmadReviewResultEvidenceLocation {
            content_hash: sha256_contract(input.evidence_content_hash)?,
            line,
            locator,
        },
        finding_id,
        origin,
        severity: input.severity,
        summary,
        verification_state: input.verification_state,
    })
}

/// Computes which required evidence codes are present among the merged
/// findings, preserving the caller's declared `required_evidence` order.
fn build_verification(
    required_evidence: &[String],
    present_codes: &BTreeSet<&str>,
    gaps_block_completion: bool,
) -> Result<BmadReviewResultVerification, BmadReviewError> {
    let mut required = Vec::with_capacity(required_evidence.len());
    let mut present = Vec::new();
    for code in required_evidence {
        required.push(code.parse().map_err(|_| BmadReviewError::InvalidCode)?);
        if present_codes.contains(code.as_str()) {
            present.push(code.parse().map_err(|_| BmadReviewError::InvalidCode)?);
        }
    }
    Ok(BmadReviewResultVerification {
        gaps_block_completion,
        present_evidence: present,
        required_evidence: required,
    })
}

/// `blocked` when a required-evidence gap must block completion, or an
/// unresolved (not refuted) high/critical deterministic finding remains;
/// `advisory` when only lesser findings or a non-blocking gap exist;
/// `passed` when the merged set carries nothing that needs attention.
fn derive_disposition(
    deterministic_findings: &[BmadReviewResultFinding],
    all_codes: &BTreeSet<&str>,
    verification: &BmadReviewResultVerification,
) -> BmadReviewResultDisposition {
    let missing_required = verification
        .required_evidence
        .iter()
        .any(|code| !all_codes.contains(code.as_str()));
    let blocking_gap = missing_required && verification.gaps_block_completion;
    let unresolved_critical = deterministic_findings.iter().any(|finding| {
        matches!(
            finding.severity,
            BmadReviewResultFindingSeverity::High | BmadReviewResultFindingSeverity::Critical
        ) && finding.verification_state != BmadReviewResultFindingVerificationState::Refuted
    });
    if blocking_gap || unresolved_critical {
        BmadReviewResultDisposition::Blocked
    } else if missing_required || !deterministic_findings.is_empty() || !all_codes.is_empty() {
        BmadReviewResultDisposition::Advisory
    } else {
        BmadReviewResultDisposition::Passed
    }
}

/// Hashes a result's content, excluding its own `resultHash` field.
fn content_hash(result: &BmadReviewResult) -> Result<Sha256Digest, BmadReviewError> {
    let value = serde_json::to_value(result).map_err(|_| BmadReviewError::Hash)?;
    canonical_hash_without_field("bmad-review-result", 1, &value, "resultHash")
        .map_err(|_| BmadReviewError::Hash)
}

fn sha256_contract(digest: Sha256Digest) -> Result<CommonSha256, BmadReviewError> {
    digest
        .to_string()
        .parse()
        .map_err(|_| BmadReviewError::Hash)
}
