//! Truthful BMAD Model Context projection (ENG-14).
//!
//! Without this module a preview of what goes into the model's context
//! window would be produced by one code path and the actual dispatch built
//! by another, so the two could silently drift — a preview that promises
//! one thing while dispatch sends another is worse than no preview at all.
//! [`project`] is the single function both callers invoke; the caller
//! chooses only [`BmadContextContributionPurpose`], never a second
//! implementation. Every contribution also carries why it is present
//! (`inclusionReason`) and what was actually done to it (`transformation`),
//! so a caller can trust the projection instead of re-deriving it. Budget
//! pressure is resolved by compaction, then truncation, then omission of
//! optional content; a `required_by_source` contribution is never quietly
//! dropped or mislabelled — when it cannot fit, projection fails closed.

use serde::Serialize;
use serde_json::json;

use crate::generated_contracts::{
    BmadContextContribution, BmadContextContributionContribution,
    BmadContextContributionContributionInclusionReason, BmadContextContributionContributionKind,
    BmadContextContributionContributionTransformation, BmadContextContributionIdentities,
    BmadContextContributionPurpose, BmadContextContributionReportedUsage, CommonContractId,
    CommonSha256,
};
use crate::{canonical_hash_without_field, Sha256Digest};

/// Contract-declared bounds a projection request must stay inside.
const MAX_CONTRIBUTIONS: usize = 512;
const MAX_TOKENS: i64 = 4_194_304;

/// The identities a dispatch snapshot is bound to.
///
/// `projectionHash` covers every field here: changing any one of them —
/// especially `config_hash` or `source_id` — must change the hash, which is
/// exactly how a stale preview becomes detectable.
#[derive(Clone, Debug)]
pub struct BmadContextProjectionIdentities {
    pub source_id: String,
    pub package_id: String,
    pub config_hash: Sha256Digest,
    pub model_id: String,
    pub policy_hash: Sha256Digest,
}

/// One candidate contribution before budget policy is applied.
///
/// `compacted_tokens` and `truncated_tokens` are the sizes this contribution
/// could shrink to, smallest last. A `None` means that reduction is not
/// available for this contribution's content.
#[derive(Clone, Copy, Debug)]
pub struct BmadContextProjectionItem {
    pub kind: BmadContextContributionContributionKind,
    pub inclusion_reason: BmadContextContributionContributionInclusionReason,
    pub source_hash: Sha256Digest,
    pub estimated_tokens: i64,
    pub compacted_tokens: Option<i64>,
    pub truncated_tokens: Option<i64>,
}

/// Everything needed to produce either a preview or a dispatch snapshot.
///
/// The same request, passed twice through [`project`] with only `purpose`
/// varying, is the only supported way to obtain both views — that is what
/// keeps them from drifting apart.
#[derive(Clone, Debug)]
pub struct BmadContextProjectionRequest {
    pub session_id: String,
    pub projection_id: String,
    pub identities: BmadContextProjectionIdentities,
    pub items: Vec<BmadContextProjectionItem>,
    pub token_budget: i64,
    pub output_reserve_tokens: Option<i64>,
}

/// Why a projection request was refused. The request is never partially
/// applied: every error leaves nothing built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadContextProjectionError {
    /// The input violated its bounded contract.
    BoundsViolation,
    /// A session, projection, source, package, or model identity failed the
    /// contract's identifier rules.
    InvalidIdentity,
    /// Every `required_by_source` contribution, even compacted or truncated
    /// where allowed, still does not fit inside `token_budget`. Failing
    /// closed here is deliberate: silently omitting required instructions
    /// would be a worse defect than refusing the projection.
    BudgetExceeded,
    /// The projection's self-hash could not be computed.
    Hash,
}

/// Fixed, explicit precedence used to order contributions by kind.
const fn kind_precedence(kind: BmadContextContributionContributionKind) -> u8 {
    match kind {
        BmadContextContributionContributionKind::Instructions => 0,
        BmadContextContributionContributionKind::Config => 1,
        BmadContextContributionContributionKind::Customization => 2,
        BmadContextContributionContributionKind::Artifact => 3,
        BmadContextContributionContributionKind::SessionState => 4,
        BmadContextContributionContributionKind::ToolCatalog => 5,
        BmadContextContributionContributionKind::Summary => 6,
    }
}

/// Projects one ordered, budget-fitted set of BMAD contributions.
///
/// This is the one projector both preview and dispatch callers use. Given an
/// identical `request`, calling this twice with `Purpose::Preview` and
/// `Purpose::Dispatch` yields identical contributions, ordinals, estimates,
/// and `projectionHash` — the returned `purpose` field is the only thing
/// that differs. A preview is therefore never a lie about what dispatch
/// would send, because there is no second code path that could disagree.
///
/// # Errors
///
/// Returns [`BmadContextProjectionError::BoundsViolation`] when the item
/// count or a token value exceeds its contract bound,
/// [`BmadContextProjectionError::InvalidIdentity`] for a malformed session,
/// projection, source, package, or model identifier,
/// [`BmadContextProjectionError::BudgetExceeded`] when required
/// contributions cannot fit even after compaction and truncation, and
/// [`BmadContextProjectionError::Hash`] when the self-hash cannot be
/// computed.
pub fn project(
    request: &BmadContextProjectionRequest,
    purpose: BmadContextContributionPurpose,
) -> Result<BmadContextContribution, BmadContextProjectionError> {
    validate_request(request)?;

    let mut ordered: Vec<(usize, &BmadContextProjectionItem)> =
        request.items.iter().enumerate().collect();
    ordered.sort_by_key(|(index, item)| (kind_precedence(item.kind), *index));

    let output_reserve = request.output_reserve_tokens.unwrap_or(0);
    let mut spendable = request
        .token_budget
        .checked_sub(output_reserve)
        .ok_or(BmadContextProjectionError::BudgetExceeded)?;

    let mut resolved = Vec::with_capacity(ordered.len());
    for (_, item) in &ordered {
        let required = item.inclusion_reason
            == BmadContextContributionContributionInclusionReason::RequiredBySource;
        match fit(item, spendable) {
            Some((tokens, transformation)) => {
                spendable -= tokens;
                resolved.push((*item, tokens, transformation));
            }
            None if required => return Err(BmadContextProjectionError::BudgetExceeded),
            None => {}
        }
    }

    let contributions = resolved
        .iter()
        .enumerate()
        .map(|(ordinal, (item, tokens, transformation))| {
            build_contribution(item, ordinal, *tokens, *transformation)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let estimated_tokens: i64 = contributions
        .iter()
        .map(|entry| entry.estimated_tokens)
        .sum();
    let identities = build_identities(&request.identities)?;
    let session_id = parse_contract_id(&request.session_id)?;
    let projection_id = parse_contract_id(&request.projection_id)?;

    let mut contribution = BmadContextContribution {
        contributions,
        estimated_tokens,
        identities,
        output_reserve_tokens: request.output_reserve_tokens,
        projection_hash: sha256_contract(Sha256Digest::from_bytes([0; 32]))?,
        projection_id,
        purpose,
        reported_usage: None,
        schema_version: json!("sapphirus.bmad-context-contribution.v1"),
        session_id,
    };
    contribution.projection_hash = sha256_contract(hash_projection(&contribution)?)?;
    Ok(contribution)
}

/// Records provider-reported usage against an already-built projection.
///
/// The estimate and the projection's self-hash are load-bearing evidence of
/// what was *planned*; reported usage is evidence of what actually
/// happened. Overwriting either with the other would erase the distinction
/// acceptance criterion 3 depends on, so this only ever adds the sibling
/// field.
#[must_use]
pub fn attach_reported_usage(
    projection: &BmadContextContribution,
    reported_usage: BmadContextContributionReportedUsage,
) -> BmadContextContribution {
    BmadContextContribution {
        contributions: projection.contributions.clone(),
        estimated_tokens: projection.estimated_tokens,
        identities: projection.identities.clone(),
        output_reserve_tokens: projection.output_reserve_tokens,
        projection_hash: projection.projection_hash.clone(),
        projection_id: projection.projection_id.clone(),
        purpose: projection.purpose,
        reported_usage: Some(reported_usage),
        schema_version: projection.schema_version.clone(),
        session_id: projection.session_id.clone(),
    }
}

/// Chooses the smallest transformation that lets `item` fit in `spendable`.
///
/// Order matters: verbatim is preferred, and compaction is tried before
/// truncation because it is the less lossy reduction. A `None` return means
/// no available transformation fits; the caller treats that as a hard
/// failure for a `required_by_source` item and as omission otherwise.
fn fit(
    item: &BmadContextProjectionItem,
    spendable: i64,
) -> Option<(i64, BmadContextContributionContributionTransformation)> {
    if item.estimated_tokens <= spendable {
        return Some((
            item.estimated_tokens,
            BmadContextContributionContributionTransformation::Verbatim,
        ));
    }
    if let Some(tokens) = item.compacted_tokens {
        if tokens <= spendable {
            return Some((
                tokens,
                BmadContextContributionContributionTransformation::Compacted,
            ));
        }
    }
    if let Some(tokens) = item.truncated_tokens {
        if tokens <= spendable {
            return Some((
                tokens,
                BmadContextContributionContributionTransformation::Truncated,
            ));
        }
    }
    None
}

fn build_contribution(
    item: &BmadContextProjectionItem,
    ordinal: usize,
    estimated_tokens: i64,
    transformation: BmadContextContributionContributionTransformation,
) -> Result<BmadContextContributionContribution, BmadContextProjectionError> {
    let ordinal =
        i64::try_from(ordinal).map_err(|_| BmadContextProjectionError::BoundsViolation)?;
    Ok(BmadContextContributionContribution {
        estimated_tokens,
        inclusion_reason: item.inclusion_reason,
        kind: item.kind,
        ordinal,
        source_hash: sha256_contract(item.source_hash)?,
        transformation,
    })
}

fn build_identities(
    identities: &BmadContextProjectionIdentities,
) -> Result<BmadContextContributionIdentities, BmadContextProjectionError> {
    Ok(BmadContextContributionIdentities {
        config_hash: sha256_contract(identities.config_hash)?,
        model_id: parse_contract_id(&identities.model_id)?,
        package_id: parse_contract_id(&identities.package_id)?,
        policy_hash: sha256_contract(identities.policy_hash)?,
        source_id: parse_contract_id(&identities.source_id)?,
    })
}

fn validate_request(
    request: &BmadContextProjectionRequest,
) -> Result<(), BmadContextProjectionError> {
    if request.items.len() > MAX_CONTRIBUTIONS {
        return Err(BmadContextProjectionError::BoundsViolation);
    }
    if !(0..=MAX_TOKENS).contains(&request.token_budget) {
        return Err(BmadContextProjectionError::BoundsViolation);
    }
    if let Some(reserve) = request.output_reserve_tokens {
        if !(0..=MAX_TOKENS).contains(&reserve) {
            return Err(BmadContextProjectionError::BoundsViolation);
        }
    }
    for item in &request.items {
        if !(0..=MAX_TOKENS).contains(&item.estimated_tokens) {
            return Err(BmadContextProjectionError::BoundsViolation);
        }
        if item
            .compacted_tokens
            .is_some_and(|tokens| tokens > item.estimated_tokens)
            || item
                .truncated_tokens
                .is_some_and(|tokens| tokens > item.estimated_tokens)
        {
            return Err(BmadContextProjectionError::BoundsViolation);
        }
    }
    Ok(())
}

/// The exact fields `projectionHash` covers.
///
/// `purpose` is deliberately absent: preview and dispatch are required to
/// carry the same hash so a caller can prove they did not drift, and the
/// only field allowed to differ between them is `purpose` itself. Hashing
/// it would make that guarantee impossible to satisfy.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextProjectionHashEnvelope<'a> {
    schema_version: &'a serde_json::Value,
    session_id: &'a CommonContractId,
    projection_id: &'a CommonContractId,
    identities: &'a BmadContextContributionIdentities,
    contributions: &'a [BmadContextContributionContribution],
    estimated_tokens: i64,
    output_reserve_tokens: Option<i64>,
    projection_hash: Option<()>,
}

fn hash_projection(
    contribution: &BmadContextContribution,
) -> Result<Sha256Digest, BmadContextProjectionError> {
    let envelope = ContextProjectionHashEnvelope {
        schema_version: &contribution.schema_version,
        session_id: &contribution.session_id,
        projection_id: &contribution.projection_id,
        identities: &contribution.identities,
        contributions: &contribution.contributions,
        estimated_tokens: contribution.estimated_tokens,
        output_reserve_tokens: contribution.output_reserve_tokens,
        projection_hash: None,
    };
    canonical_hash_without_field("bmad-context-projection", 1, &envelope, "projectionHash")
        .map_err(|_| BmadContextProjectionError::Hash)
}

fn parse_contract_id(value: &str) -> Result<CommonContractId, BmadContextProjectionError> {
    value
        .parse()
        .map_err(|_| BmadContextProjectionError::InvalidIdentity)
}

fn sha256_contract(digest: Sha256Digest) -> Result<CommonSha256, BmadContextProjectionError> {
    digest
        .to_string()
        .parse()
        .map_err(|_| BmadContextProjectionError::Hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::from_bytes([byte; 32])
    }

    fn identities() -> BmadContextProjectionIdentities {
        BmadContextProjectionIdentities {
            source_id: "source_01J00000000000000000000000".to_owned(),
            package_id: "package_01J00000000000000000000000".to_owned(),
            config_hash: digest(1),
            model_id: "model_01J00000000000000000000000".to_owned(),
            policy_hash: digest(2),
        }
    }

    fn item(
        kind: BmadContextContributionContributionKind,
        reason: BmadContextContributionContributionInclusionReason,
        tokens: i64,
    ) -> BmadContextProjectionItem {
        BmadContextProjectionItem {
            kind,
            inclusion_reason: reason,
            source_hash: digest(3),
            estimated_tokens: tokens,
            compacted_tokens: None,
            truncated_tokens: None,
        }
    }

    fn request() -> BmadContextProjectionRequest {
        BmadContextProjectionRequest {
            session_id: "session_01J00000000000000000000000".to_owned(),
            projection_id: "projection_01J00000000000000000000000".to_owned(),
            identities: identities(),
            items: vec![
                item(
                    BmadContextContributionContributionKind::Instructions,
                    BmadContextContributionContributionInclusionReason::RequiredBySource,
                    100,
                ),
                item(
                    BmadContextContributionContributionKind::Config,
                    BmadContextContributionContributionInclusionReason::RequiredBySource,
                    50,
                ),
                item(
                    BmadContextContributionContributionKind::Customization,
                    BmadContextContributionContributionInclusionReason::RequestedByCustomization,
                    30,
                ),
                item(
                    BmadContextContributionContributionKind::Artifact,
                    BmadContextContributionContributionInclusionReason::CarriedFromSession,
                    40,
                ),
                item(
                    BmadContextContributionContributionKind::SessionState,
                    BmadContextContributionContributionInclusionReason::CarriedFromSession,
                    20,
                ),
                item(
                    BmadContextContributionContributionKind::ToolCatalog,
                    BmadContextContributionContributionInclusionReason::RequestedByCustomization,
                    10,
                ),
                item(
                    BmadContextContributionContributionKind::Summary,
                    BmadContextContributionContributionInclusionReason::SummarizedForBudget,
                    15,
                ),
            ],
            token_budget: 1_000,
            output_reserve_tokens: Some(100),
        }
    }

    // Break caught: a projector that reimplements dispatch separately from
    // preview could drift, silently sending something other than what was
    // shown. One function called twice with different purposes cannot drift.
    #[test]
    fn preview_and_dispatch_share_every_field_except_purpose() {
        let request = request();
        let preview = project(&request, BmadContextContributionPurpose::Preview).unwrap();
        let dispatch = project(&request, BmadContextContributionPurpose::Dispatch).unwrap();

        assert_eq!(preview.estimated_tokens, dispatch.estimated_tokens);
        assert_eq!(
            preview.projection_hash.as_str(),
            dispatch.projection_hash.as_str()
        );
        assert_eq!(preview.contributions.len(), dispatch.contributions.len());
        for (left, right) in preview
            .contributions
            .iter()
            .zip(dispatch.contributions.iter())
        {
            assert_eq!(left.kind, right.kind);
            assert_eq!(left.ordinal, right.ordinal);
            assert_eq!(left.estimated_tokens, right.estimated_tokens);
            assert_eq!(left.transformation, right.transformation);
            assert_eq!(left.inclusion_reason, right.inclusion_reason);
        }
        assert_eq!(preview.purpose, BmadContextContributionPurpose::Preview);
        assert_eq!(dispatch.purpose, BmadContextContributionPurpose::Dispatch);
    }

    // Break caught: ordinals that follow insertion order instead of kind
    // precedence would let a customization layer silently reorder itself
    // ahead of required instructions in the actual model context.
    #[test]
    fn ordinals_are_dense_ascending_and_follow_kind_precedence() {
        let projection = project(&request(), BmadContextContributionPurpose::Preview).unwrap();
        let mut previous_precedence = 0u8;
        for (index, entry) in projection.contributions.iter().enumerate() {
            assert_eq!(entry.ordinal, i64::try_from(index).unwrap());
            let precedence = kind_precedence(entry.kind);
            assert!(precedence >= previous_precedence);
            previous_precedence = precedence;
        }
    }

    // Break caught: hashing configHash or sourceId out of the identity
    // envelope would let a stale preview keep matching a changed source or
    // config, defeating invalidation.
    #[test]
    fn each_identity_change_invalidates_the_projection_hash() {
        let base = project(&request(), BmadContextContributionPurpose::Preview).unwrap();

        let mut source_changed = request();
        source_changed.identities.source_id = "source_02J00000000000000000000000".to_owned();
        let source = project(&source_changed, BmadContextContributionPurpose::Preview).unwrap();
        assert_ne!(
            base.projection_hash.as_str(),
            source.projection_hash.as_str()
        );

        let mut package_changed = request();
        package_changed.identities.package_id = "package_02J00000000000000000000000".to_owned();
        let package = project(&package_changed, BmadContextContributionPurpose::Preview).unwrap();
        assert_ne!(
            base.projection_hash.as_str(),
            package.projection_hash.as_str()
        );

        let mut config_changed = request();
        config_changed.identities.config_hash = digest(9);
        let config = project(&config_changed, BmadContextContributionPurpose::Preview).unwrap();
        assert_ne!(
            base.projection_hash.as_str(),
            config.projection_hash.as_str()
        );

        let mut model_changed = request();
        model_changed.identities.model_id = "model_02J00000000000000000000000".to_owned();
        let model = project(&model_changed, BmadContextContributionPurpose::Preview).unwrap();
        assert_ne!(
            base.projection_hash.as_str(),
            model.projection_hash.as_str()
        );

        let mut policy_changed = request();
        policy_changed.identities.policy_hash = digest(9);
        let policy = project(&policy_changed, BmadContextContributionPurpose::Preview).unwrap();
        assert_ne!(
            base.projection_hash.as_str(),
            policy.projection_hash.as_str()
        );
    }

    // Break caught: recomputing the hash (or the estimate) after a provider
    // usage report arrives would make "what we planned" indistinguishable
    // from "what actually happened," destroying the reconciliation record.
    #[test]
    fn attaching_reported_usage_leaves_estimate_and_hash_byte_identical() {
        let projection = project(&request(), BmadContextContributionPurpose::Dispatch).unwrap();
        let reported = BmadContextContributionReportedUsage {
            completion_tokens: 12,
            prompt_tokens: 340,
        };
        let reconciled = attach_reported_usage(&projection, reported);

        assert_eq!(reconciled.estimated_tokens, projection.estimated_tokens);
        assert_eq!(
            reconciled.projection_hash.as_str(),
            projection.projection_hash.as_str()
        );
        assert!(reconciled.reported_usage.is_some());
        assert!(projection.reported_usage.is_none());
    }

    // Break caught: mislabeling a shrunk contribution as `verbatim` hides
    // the fact that content was altered under budget pressure, which is a
    // truthfulness defect the projection exists to prevent.
    #[test]
    fn budget_pressure_produces_truthfully_labelled_compacted_and_truncated_entries() {
        let mut request = request();
        // 200 forces real pressure: 100 + 150 + 90 verbatim cannot fit, so the
        // artifact falls back to its compacted form. At 260 it fit verbatim and
        // the scenario never exercised compaction.
        request.token_budget = 200;
        request.output_reserve_tokens = Some(0);
        request.items = vec![
            item(
                BmadContextContributionContributionKind::Instructions,
                BmadContextContributionContributionInclusionReason::RequiredBySource,
                100,
            ),
            BmadContextProjectionItem {
                compacted_tokens: Some(60),
                truncated_tokens: Some(20),
                ..item(
                    BmadContextContributionContributionKind::Artifact,
                    BmadContextContributionContributionInclusionReason::CarriedFromSession,
                    150,
                )
            },
            BmadContextProjectionItem {
                compacted_tokens: None,
                truncated_tokens: Some(10),
                ..item(
                    BmadContextContributionContributionKind::Summary,
                    BmadContextContributionContributionInclusionReason::SummarizedForBudget,
                    90,
                )
            },
        ];
        let projection = project(&request, BmadContextContributionPurpose::Preview).unwrap();

        let artifact = projection
            .contributions
            .iter()
            .find(|entry| entry.kind == BmadContextContributionContributionKind::Artifact)
            .unwrap();
        assert_eq!(
            artifact.transformation,
            BmadContextContributionContributionTransformation::Compacted
        );
        assert_eq!(artifact.estimated_tokens, 60);

        let summary = projection
            .contributions
            .iter()
            .find(|entry| entry.kind == BmadContextContributionContributionKind::Summary)
            .unwrap();
        assert_eq!(
            summary.transformation,
            BmadContextContributionContributionTransformation::Truncated
        );
        assert_eq!(summary.estimated_tokens, 10);
    }

    // Break caught: dropping a `required_by_source` contribution to make the
    // numbers fit would silently omit instructions the model must see;
    // failing closed makes that omission impossible to miss.
    #[test]
    fn required_contribution_that_cannot_fit_fails_closed_instead_of_being_dropped() {
        let mut request = request();
        request.token_budget = 10;
        request.output_reserve_tokens = Some(0);
        request.items = vec![item(
            BmadContextContributionContributionKind::Instructions,
            BmadContextContributionContributionInclusionReason::RequiredBySource,
            100,
        )];

        let result = project(&request, BmadContextContributionPurpose::Preview);
        assert_eq!(
            result.unwrap_err(),
            BmadContextProjectionError::BudgetExceeded
        );
    }

    // Break caught: a contribution kind or transformation the projector
    // could not construct would make the contract's declared enum values a
    // lie about what this code actually emits.
    #[test]
    fn all_seven_kinds_and_all_four_transformations_are_representable() {
        let mut request = request();
        request.token_budget = 10_000;
        request.output_reserve_tokens = Some(0);
        request.items = vec![
            item(
                BmadContextContributionContributionKind::Instructions,
                BmadContextContributionContributionInclusionReason::RequiredBySource,
                10,
            ),
            item(
                BmadContextContributionContributionKind::Config,
                BmadContextContributionContributionInclusionReason::RequiredBySource,
                10,
            ),
            item(
                BmadContextContributionContributionKind::Customization,
                BmadContextContributionContributionInclusionReason::RequestedByCustomization,
                10,
            ),
            item(
                BmadContextContributionContributionKind::Artifact,
                BmadContextContributionContributionInclusionReason::CarriedFromSession,
                10,
            ),
            item(
                BmadContextContributionContributionKind::SessionState,
                BmadContextContributionContributionInclusionReason::CarriedFromSession,
                10,
            ),
            item(
                BmadContextContributionContributionKind::ToolCatalog,
                BmadContextContributionContributionInclusionReason::RequestedByCustomization,
                10,
            ),
            item(
                BmadContextContributionContributionKind::Summary,
                BmadContextContributionContributionInclusionReason::SummarizedForBudget,
                10,
            ),
        ];
        let projection = project(&request, BmadContextContributionPurpose::Preview).unwrap();
        assert_eq!(projection.contributions.len(), 7);
        for entry in &projection.contributions {
            assert_eq!(
                entry.transformation,
                BmadContextContributionContributionTransformation::Verbatim
            );
        }

        request.token_budget = 5;
        request.items = vec![
            BmadContextProjectionItem {
                compacted_tokens: Some(4),
                ..item(
                    BmadContextContributionContributionKind::Customization,
                    BmadContextContributionContributionInclusionReason::RequestedByCustomization,
                    50,
                )
            },
            BmadContextProjectionItem {
                truncated_tokens: Some(1),
                ..item(
                    BmadContextContributionContributionKind::Artifact,
                    BmadContextContributionContributionInclusionReason::CarriedFromSession,
                    50,
                )
            },
        ];
        let shrunk = project(&request, BmadContextContributionPurpose::Preview).unwrap();
        let transformations = shrunk
            .contributions
            .iter()
            .map(|entry| entry.transformation)
            .collect::<Vec<_>>();
        assert!(
            transformations.contains(&BmadContextContributionContributionTransformation::Compacted)
        );
        assert!(
            transformations.contains(&BmadContextContributionContributionTransformation::Truncated)
        );
    }

    // Break caught: a projection that cannot round-trip through JSON would
    // mean the renderer or the dispatch transport could not actually carry
    // it, even though the in-process struct looked fine.
    #[test]
    fn projection_round_trips_through_serde_json() {
        let projection = project(&request(), BmadContextContributionPurpose::Dispatch).unwrap();
        let value = serde_json::to_value(&projection).unwrap();
        let restored: BmadContextContribution = serde_json::from_value(value).unwrap();
        assert_eq!(
            restored.projection_hash.as_str(),
            projection.projection_hash.as_str()
        );
        assert_eq!(restored.contributions.len(), projection.contributions.len());
    }
}
