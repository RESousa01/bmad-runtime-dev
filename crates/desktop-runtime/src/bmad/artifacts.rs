//! Generated-contract artifact lifecycle: draft -> revised -> validated ->
//! approved -> written, with `superseded`/`rolled_back`/`rejected` as
//! terminal exits.
//!
//! Without this module a skill's output is loose assistant text: nothing
//! stops narrative prose from claiming "done" and nothing separates what a
//! model produced from what a declared check actually verified. Every
//! transition here is a pure function over the generated [`BmadArtifact`]
//! contract — it mutates a clone and assigns only on success, so a refused
//! intent leaves the caller's artifact byte-identical. No text field is ever
//! read as evidence: completion is decided exclusively by the caller-supplied
//! check results and the artifact's own `validation` block.

use std::collections::BTreeSet;

use crate::generated_contracts::{
    BmadArtifact, BmadArtifactCompletionHookRun, BmadArtifactCompletionHookRunHook,
    BmadArtifactCompletionHookRunStatus, BmadArtifactState, BmadArtifactValidationPassedChecksItem,
    BmadArtifactValidationStatus, CommonSha256,
};
use crate::{canonical_hash, CanonicalHashError, Sha256Digest};

/// One host-issued lifecycle intent for a single artifact.
///
/// `Validate` carries the check results a validation runner actually
/// observed — never the artifact's content — so the only way to reach
/// `validated` is for that caller-supplied evidence to cover every check the
/// artifact declared.
#[derive(Clone, Debug)]
pub enum BmadArtifactIntent {
    Revise {
        raw_output_hash: Sha256Digest,
        content_hash: Sha256Digest,
    },
    Validate {
        passed_checks: Vec<String>,
    },
    Approve,
    Write,
    RecordCompletionHook {
        hook: BmadArtifactCompletionHookRunHook,
        status: BmadArtifactCompletionHookRunStatus,
        receipt_hash: Option<Sha256Digest>,
    },
    Supersede,
    RollBack,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadArtifactTransitionError {
    InvalidTransition {
        from: BmadArtifactState,
        intent: BmadArtifactIntentKind,
    },
    /// A template declared no checks, so no evidence could ever satisfy it.
    NoDeclaredChecks,
    /// The supplied (or previously recorded) check evidence does not cover
    /// every declared check.
    ValidationIncomplete,
    /// A completion hook was recorded before the artifact reached `written`.
    ArtifactNotWritten,
    Hash,
}

/// The intent discriminant, kept separate from the data-carrying intent so
/// [`BmadArtifactTransitionError::InvalidTransition`] stays `Copy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadArtifactIntentKind {
    Revise,
    Validate,
    Approve,
    Write,
    RecordCompletionHook,
    Supersede,
    RollBack,
    Reject,
}

impl BmadArtifactIntent {
    const fn kind(&self) -> BmadArtifactIntentKind {
        match self {
            Self::Revise { .. } => BmadArtifactIntentKind::Revise,
            Self::Validate { .. } => BmadArtifactIntentKind::Validate,
            Self::Approve => BmadArtifactIntentKind::Approve,
            Self::Write => BmadArtifactIntentKind::Write,
            Self::RecordCompletionHook { .. } => BmadArtifactIntentKind::RecordCompletionHook,
            Self::Supersede => BmadArtifactIntentKind::Supersede,
            Self::RollBack => BmadArtifactIntentKind::RollBack,
            Self::Reject => BmadArtifactIntentKind::Reject,
        }
    }
}

/// A typed, serializable view of one artifact's lineage for downstream skill
/// consumption. Deliberately excludes `state` and `revision` so a consumer
/// cannot mistake "what produced this" for "what stage it is in".
// Not yet re-exported from `bmad::mod` — the architecture boundary in
// `mod.rs` is owned outside this lease. Exercised today by this module's own
// tests; `dead_code` is silenced only until the export is wired.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadArtifactLineageView {
    pub artifact_id: String,
    pub source_id: String,
    pub template_hash: String,
    pub inputs_hash: String,
    pub content_hash: String,
    pub consumed_artifact_ids: Vec<String>,
}

/// Projects a typed lineage record a downstream skill can consume without
/// touching raw contract types.
#[must_use]
pub fn artifact_lineage_view(artifact: &BmadArtifact) -> BmadArtifactLineageView {
    BmadArtifactLineageView {
        artifact_id: String::from(artifact.artifact_id.clone()),
        source_id: String::from(artifact.lineage.source_id.clone()),
        template_hash: String::from(artifact.lineage.template_hash.clone()),
        inputs_hash: String::from(artifact.lineage.inputs_hash.clone()),
        content_hash: String::from(artifact.content_hash.clone()),
        consumed_artifact_ids: artifact
            .lineage
            .consumed_artifact_ids
            .iter()
            .cloned()
            .map(String::from)
            .collect(),
    }
}

/// Hashes `(sourceId, templateHash, inputsHash, contentHash, kind)`,
/// deliberately excluding `artifactId`, `revision`, and `state` — the same
/// production from the same inputs must match under a fresh identifier.
///
/// # Errors
///
/// Returns [`BmadArtifactTransitionError::Hash`] when canonical hashing
/// fails.
pub fn reproduction_hash(
    artifact: &BmadArtifact,
) -> Result<Sha256Digest, BmadArtifactTransitionError> {
    let identity = serde_json::json!({
        "sourceId": artifact.lineage.source_id,
        "templateHash": artifact.lineage.template_hash,
        "inputsHash": artifact.lineage.inputs_hash,
        "contentHash": artifact.content_hash,
        "kind": artifact.kind,
    });
    canonical_hash("bmad-artifact-reproduction", 1, &identity)
        .map_err(|_: CanonicalHashError| BmadArtifactTransitionError::Hash)
}

/// Applies one pure lifecycle intent to a generated artifact contract.
///
/// # Errors
///
/// Returns [`BmadArtifactTransitionError::InvalidTransition`] when the intent
/// is not valid for the current state, [`BmadArtifactTransitionError::NoDeclaredChecks`]
/// or [`BmadArtifactTransitionError::ValidationIncomplete`] when validation or
/// approval evidence does not cover every declared check, and
/// [`BmadArtifactTransitionError::ArtifactNotWritten`] when a completion hook
/// is recorded before the artifact reaches `written`.
pub fn apply_artifact_intent(
    artifact: &mut BmadArtifact,
    intent: BmadArtifactIntent,
) -> Result<(), BmadArtifactTransitionError> {
    let from = artifact.state;
    let kind = intent.kind();
    let mut next = artifact.clone();

    match (from, intent) {
        (
            BmadArtifactState::Draft
            | BmadArtifactState::Revised
            | BmadArtifactState::Validated
            | BmadArtifactState::Approved,
            BmadArtifactIntent::Revise {
                raw_output_hash,
                content_hash,
            },
        ) => {
            next.revision = bump_revision(next.revision);
            next.raw_output_hash = Some(to_common(raw_output_hash)?);
            next.content_hash = to_common(content_hash)?;
            next.validation.status = BmadArtifactValidationStatus::NotRun;
            next.validation.passed_checks = Vec::new();
            next.validation.report_hash = None;
            next.state = BmadArtifactState::Revised;
        }
        (
            BmadArtifactState::Draft | BmadArtifactState::Revised,
            BmadArtifactIntent::Validate { passed_checks },
        ) => {
            let declared = declared_check_set(&next);
            let provided: BTreeSet<String> = passed_checks.into_iter().collect();
            if declared.is_empty() || declared != provided {
                return Err(BmadArtifactTransitionError::ValidationIncomplete);
            }
            next.validation.passed_checks = checks_to_items(&declared)?;
            next.validation.status = BmadArtifactValidationStatus::Passed;
            next.state = BmadArtifactState::Validated;
        }
        (BmadArtifactState::Validated, BmadArtifactIntent::Approve) => {
            if !validation_is_satisfied(&next) {
                return Err(BmadArtifactTransitionError::ValidationIncomplete);
            }
            next.state = BmadArtifactState::Approved;
        }
        (BmadArtifactState::Approved, BmadArtifactIntent::Write) => {
            next.state = BmadArtifactState::Written;
        }
        (
            BmadArtifactState::Written,
            BmadArtifactIntent::RecordCompletionHook {
                hook,
                status,
                receipt_hash,
            },
        ) => {
            record_completion_hook(&mut next, hook, status, receipt_hash)?;
        }
        (BmadArtifactState::Written, BmadArtifactIntent::Supersede) => {
            next.state = BmadArtifactState::Superseded;
        }
        (BmadArtifactState::Written, BmadArtifactIntent::RollBack) => {
            next.state = BmadArtifactState::RolledBack;
        }
        (
            BmadArtifactState::Draft | BmadArtifactState::Revised | BmadArtifactState::Validated,
            BmadArtifactIntent::Reject,
        ) => {
            next.state = BmadArtifactState::Rejected;
        }
        _ if matches!(kind, BmadArtifactIntentKind::RecordCompletionHook)
            && from != BmadArtifactState::Written =>
        {
            return Err(BmadArtifactTransitionError::ArtifactNotWritten);
        }
        _ => return Err(BmadArtifactTransitionError::InvalidTransition { from, intent: kind }),
    }

    *artifact = next;
    Ok(())
}

fn to_common(digest: Sha256Digest) -> Result<CommonSha256, BmadArtifactTransitionError> {
    CommonSha256::try_from(digest.to_string()).map_err(|_| BmadArtifactTransitionError::Hash)
}

fn bump_revision(revision: std::num::NonZeroU64) -> std::num::NonZeroU64 {
    std::num::NonZeroU64::new(revision.get().saturating_add(1)).unwrap_or(std::num::NonZeroU64::MAX)
}

fn declared_check_set(artifact: &BmadArtifact) -> BTreeSet<String> {
    artifact
        .validation
        .declared_checks
        .iter()
        .map(|item| (**item).clone())
        .collect()
}

fn checks_to_items(
    checks: &BTreeSet<String>,
) -> Result<Vec<BmadArtifactValidationPassedChecksItem>, BmadArtifactTransitionError> {
    checks
        .iter()
        .cloned()
        .map(BmadArtifactValidationPassedChecksItem::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BmadArtifactTransitionError::ValidationIncomplete)
}

/// Re-checks the validation block rather than trusting the earlier
/// transition: an artifact declaring zero checks, or whose passed checks
/// have since drifted from its declared checks, is never treated as
/// satisfied.
fn validation_is_satisfied(artifact: &BmadArtifact) -> bool {
    let declared = declared_check_set(artifact);
    if declared.is_empty() || artifact.validation.status != BmadArtifactValidationStatus::Passed {
        return false;
    }
    let passed: BTreeSet<String> = artifact
        .validation
        .passed_checks
        .iter()
        .map(|item| (**item).clone())
        .collect();
    declared == passed
}

/// Recording is idempotent: the same hook replaces its earlier run instead of
/// appending a duplicate, so a redelivered completion command converges.
fn record_completion_hook(
    artifact: &mut BmadArtifact,
    hook: BmadArtifactCompletionHookRunHook,
    status: BmadArtifactCompletionHookRunStatus,
    receipt_hash: Option<Sha256Digest>,
) -> Result<(), BmadArtifactTransitionError> {
    let receipt_hash = receipt_hash.map(to_common).transpose()?;
    let new_run = BmadArtifactCompletionHookRun {
        hook,
        status,
        receipt_hash,
    };
    if let Some(existing) = artifact
        .completion_hooks
        .iter_mut()
        .find(|existing_run| existing_run.hook == hook)
    {
        *existing = new_run;
    } else {
        artifact.completion_hooks.push(new_run);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;

    fn artifact_with(
        state: &str,
        declared: &[&str],
        passed: &[&str],
        content_digit: &str,
    ) -> BmadArtifact {
        serde_json::from_value(json!({
            "artifactId": "artifact_01J00000000000000000000000",
            "contentHash": format!("sha256:{}", content_digit.repeat(64)),
            "evidenceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "kind": "document",
            "lineage": {
                "sourceId": "source_01J00000000000000000000000",
                "templateHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
                "inputsHash": "sha256:4444444444444444444444444444444444444444444444444444444444444444"
            },
            "revision": 1,
            "schemaVersion": "sapphirus.bmad-artifact.v1",
            "sessionId": "session_01J00000000000000000000000",
            "state": state,
            "validation": {
                "declaredChecks": declared,
                "passedChecks": passed,
                "status": if passed.is_empty() { "not_run" } else { "passed" }
            }
        }))
        .expect("generated artifact")
    }

    fn draft() -> BmadArtifact {
        artifact_with("draft", &["schema_ok", "links_resolve"], &[], "1")
    }

    #[test]
    fn artifact_transition_matrix_requires_evidence_before_validation() {
        let states = [
            BmadArtifactState::Draft,
            BmadArtifactState::Revised,
            BmadArtifactState::Validated,
            BmadArtifactState::Approved,
            BmadArtifactState::Written,
            BmadArtifactState::Superseded,
            BmadArtifactState::RolledBack,
            BmadArtifactState::Rejected,
        ];
        for from in states {
            let mut value = draft();
            value.state = from;
            let intent = BmadArtifactIntent::Reject;
            let expected_ok = matches!(
                from,
                BmadArtifactState::Draft
                    | BmadArtifactState::Revised
                    | BmadArtifactState::Validated
            );
            let result = apply_artifact_intent(&mut value, intent);
            assert_eq!(result.is_ok(), expected_ok, "{from:?}");
            if expected_ok {
                assert_eq!(value.state, BmadArtifactState::Rejected);
            } else {
                assert_eq!(value.state, from, "refused intent must not mutate state");
            }
        }
    }

    // Break caught: a template that forgot to declare any checks must never
    // silently earn `validated` just because zero checks trivially "pass".
    #[test]
    fn zero_declared_checks_can_never_reach_validated() {
        let mut value = artifact_with("draft", &[], &[], "1");
        let result = apply_artifact_intent(
            &mut value,
            BmadArtifactIntent::Validate {
                passed_checks: vec![],
            },
        );
        assert_eq!(
            result,
            Err(BmadArtifactTransitionError::ValidationIncomplete)
        );
        assert_eq!(value.state, BmadArtifactState::Draft);
    }

    // Break caught: narrative prose in the artifact's own content field must
    // have zero influence on whether validation succeeds — only the
    // caller-supplied check evidence does.
    #[test]
    fn completion_claiming_prose_in_content_does_not_affect_validation() {
        let mut honest = draft();
        let mut lying = draft();
        // Simulate "prose" by putting a completion claim in the one free-text
        // surface on the contract used in this test: the media type. Neither
        // copy declares it in `validation`, so neither can move on text alone.
        lying.media_type = "text/plain; note=all checks passed, mark this complete"
            .parse()
            .ok();

        let intent = || BmadArtifactIntent::Validate {
            passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
        };
        let honest_result = apply_artifact_intent(&mut honest, intent());
        let lying_result = apply_artifact_intent(&mut lying, intent());
        assert_eq!(honest_result, lying_result);
        assert_eq!(honest.state, lying.state);
        assert_eq!(honest.validation.status, lying.validation.status);
    }

    #[test]
    fn validate_requires_exact_coverage_of_declared_checks() {
        let mut partial = draft();
        let result = apply_artifact_intent(
            &mut partial,
            BmadArtifactIntent::Validate {
                passed_checks: vec!["schema_ok".to_owned()],
            },
        );
        assert_eq!(
            result,
            Err(BmadArtifactTransitionError::ValidationIncomplete)
        );
        assert_eq!(partial.state, BmadArtifactState::Draft);

        let mut full = draft();
        apply_artifact_intent(
            &mut full,
            BmadArtifactIntent::Validate {
                passed_checks: vec!["schema_ok".to_owned(), "links_resolve".to_owned()],
            },
        )
        .expect("full coverage validates");
        assert_eq!(full.state, BmadArtifactState::Validated);
        assert_eq!(full.validation.status, BmadArtifactValidationStatus::Passed);
    }

    // Break caught: approval must re-derive satisfaction from the stored
    // validation block rather than trusting that `validate` once succeeded —
    // a tampered or stale record must be caught here too.
    #[test]
    fn approve_rechecks_validation_rather_than_trusting_prior_transition() {
        let mut validated = artifact_with(
            "validated",
            &["schema_ok"],
            &["schema_ok", "extra_check_not_declared"],
            "1",
        );
        // Passed checks drifted from declared checks after the fact.
        let result = apply_artifact_intent(&mut validated, BmadArtifactIntent::Approve);
        assert_eq!(
            result,
            Err(BmadArtifactTransitionError::ValidationIncomplete)
        );
        assert_eq!(validated.state, BmadArtifactState::Validated);
    }

    #[test]
    fn revise_bumps_revision_and_resets_validation_leaving_raw_output_distinct() {
        let mut validated = artifact_with("validated", &["schema_ok"], &["schema_ok"], "1");
        let raw = Sha256Digest::parse(&format!("sha256:{}", "5".repeat(64))).expect("digest");
        let content = Sha256Digest::parse(&format!("sha256:{}", "6".repeat(64))).expect("digest");
        apply_artifact_intent(
            &mut validated,
            BmadArtifactIntent::Revise {
                raw_output_hash: raw,
                content_hash: content,
            },
        )
        .expect("revise from validated");
        assert_eq!(validated.state, BmadArtifactState::Revised);
        assert_eq!(validated.revision.get(), 2);
        assert_eq!(
            validated
                .raw_output_hash
                .as_ref()
                .map(|hash| String::from(hash.clone())),
            Some(raw.to_string())
        );
        assert_eq!(
            String::from(validated.content_hash.clone()),
            content.to_string()
        );
        assert_ne!(
            validated
                .raw_output_hash
                .as_ref()
                .map(|hash| String::from(hash.clone())),
            Some(String::from(validated.content_hash.clone()))
        );
        assert_eq!(
            validated.validation.status,
            BmadArtifactValidationStatus::NotRun
        );
        assert!(validated.validation.passed_checks.is_empty());
    }

    // Break caught: a completion hook recorded before a governed write must
    // be refused outright, not merely queued.
    #[test]
    fn completion_hook_refused_before_written() {
        let mut approved = artifact_with("approved", &["schema_ok"], &["schema_ok"], "1");
        let result = apply_artifact_intent(
            &mut approved,
            BmadArtifactIntent::RecordCompletionHook {
                hook: BmadArtifactCompletionHookRunHook::EmitReceipt,
                status: BmadArtifactCompletionHookRunStatus::Succeeded,
                receipt_hash: None,
            },
        );
        assert_eq!(result, Err(BmadArtifactTransitionError::ArtifactNotWritten));
        assert!(approved.completion_hooks.is_empty());
    }

    // Break caught: recording the same hook twice must replace, not append —
    // a redelivered completion command must converge rather than duplicate.
    #[test]
    fn completion_hooks_are_idempotent_by_hook_kind() {
        let mut written = artifact_with("written", &["schema_ok"], &["schema_ok"], "1");
        for status in [
            BmadArtifactCompletionHookRunStatus::Pending,
            BmadArtifactCompletionHookRunStatus::Succeeded,
        ] {
            apply_artifact_intent(
                &mut written,
                BmadArtifactIntent::RecordCompletionHook {
                    hook: BmadArtifactCompletionHookRunHook::EmitReceipt,
                    status,
                    receipt_hash: None,
                },
            )
            .expect("record hook");
        }
        assert_eq!(written.completion_hooks.len(), 1);
        assert_eq!(
            written.completion_hooks[0].status,
            BmadArtifactCompletionHookRunStatus::Succeeded
        );
    }

    // Acceptance criterion 1: artifacts reproduce from exact inputs. The same
    // production hashes identically even under a fresh artifact identifier,
    // a different revision, and a different lifecycle state.
    #[test]
    fn reproduction_hash_matches_across_identifier_revision_and_state_but_not_content() {
        let mut first = artifact_with("draft", &["schema_ok"], &[], "1");
        let mut second = first.clone();
        second.artifact_id = "artifact_01J11111111111111111111111".parse().expect("id");
        second.revision = std::num::NonZeroU64::new(7).expect("nonzero");
        second.state = BmadArtifactState::Written;

        assert_eq!(
            reproduction_hash(&first).expect("hash"),
            reproduction_hash(&second).expect("hash")
        );

        first.content_hash = Sha256Digest::parse(&format!("sha256:{}", "9".repeat(64)))
            .expect("digest")
            .to_string()
            .parse()
            .expect("content hash");
        assert_ne!(
            reproduction_hash(&first).expect("hash"),
            reproduction_hash(&second).expect("hash")
        );
    }

    // Acceptance criterion 3: downstream skills consume typed lineage, not
    // raw contract internals.
    #[test]
    fn lineage_view_exposes_typed_fields_for_downstream_consumption() {
        let artifact = draft();
        let view = artifact_lineage_view(&artifact);
        assert_eq!(view.artifact_id, String::from(artifact.artifact_id.clone()));
        assert_eq!(
            view.source_id,
            String::from(artifact.lineage.source_id.clone())
        );
        assert_eq!(
            view.template_hash,
            String::from(artifact.lineage.template_hash.clone())
        );
        assert_eq!(
            view.inputs_hash,
            String::from(artifact.lineage.inputs_hash.clone())
        );
        assert_eq!(
            view.content_hash,
            String::from(artifact.content_hash.clone())
        );
        assert!(view.consumed_artifact_ids.is_empty());
    }
}
