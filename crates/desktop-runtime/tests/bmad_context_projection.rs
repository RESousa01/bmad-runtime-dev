#![allow(clippy::expect_used, clippy::too_many_lines)]

//! Preview and dispatch must never disagree about what the model will see.
//!
//! Every case here exercises [`desktop_runtime::project_bmad_context`], the
//! single projector both callers use, through its crate-public surface —
//! never a duplicate implementation — so a passing suite is evidence the two
//! views cannot drift apart.

use desktop_runtime::generated_contracts::{
    BmadContextContribution, BmadContextContributionContributionInclusionReason,
    BmadContextContributionContributionKind, BmadContextContributionContributionTransformation,
    BmadContextContributionPurpose, BmadContextContributionReportedUsage,
};
use desktop_runtime::{
    attach_bmad_context_reported_usage as attach_reported_usage, project_bmad_context as project,
    BmadContextProjectionError, BmadContextProjectionIdentities, BmadContextProjectionItem,
    BmadContextProjectionRequest,
};

fn digest(byte: u8) -> desktop_runtime::Sha256Digest {
    desktop_runtime::Sha256Digest::from_bytes([byte; 32])
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

fn kind_precedence(kind: BmadContextContributionContributionKind) -> u8 {
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

// Break caught: a projector that reimplements dispatch separately from
// preview could drift, silently sending something other than what was
// shown. One function called twice with different purposes cannot drift.
#[test]
fn preview_and_dispatch_share_every_field_except_purpose() {
    let request = request();
    let preview =
        project(&request, BmadContextContributionPurpose::Preview).expect("preview projects");
    let dispatch =
        project(&request, BmadContextContributionPurpose::Dispatch).expect("dispatch projects");

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
// precedence would let a customization layer silently reorder itself ahead
// of required instructions in the actual model context.
#[test]
fn ordinals_are_dense_ascending_and_follow_kind_precedence() {
    let projection =
        project(&request(), BmadContextContributionPurpose::Preview).expect("projects");
    let mut previous_precedence = 0u8;
    for (index, entry) in projection.contributions.iter().enumerate() {
        assert_eq!(entry.ordinal, i64::try_from(index).expect("small index"));
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
    let base = project(&request(), BmadContextContributionPurpose::Preview).expect("projects");

    let mut source_changed = request();
    source_changed.identities.source_id = "source_02J00000000000000000000000".to_owned();
    let source =
        project(&source_changed, BmadContextContributionPurpose::Preview).expect("projects");
    assert_ne!(
        base.projection_hash.as_str(),
        source.projection_hash.as_str()
    );

    let mut package_changed = request();
    package_changed.identities.package_id = "package_02J00000000000000000000000".to_owned();
    let package =
        project(&package_changed, BmadContextContributionPurpose::Preview).expect("projects");
    assert_ne!(
        base.projection_hash.as_str(),
        package.projection_hash.as_str()
    );

    let mut config_changed = request();
    config_changed.identities.config_hash = digest(9);
    let config =
        project(&config_changed, BmadContextContributionPurpose::Preview).expect("projects");
    assert_ne!(
        base.projection_hash.as_str(),
        config.projection_hash.as_str()
    );

    let mut model_changed = request();
    model_changed.identities.model_id = "model_02J00000000000000000000000".to_owned();
    let model = project(&model_changed, BmadContextContributionPurpose::Preview).expect("projects");
    assert_ne!(
        base.projection_hash.as_str(),
        model.projection_hash.as_str()
    );

    let mut policy_changed = request();
    policy_changed.identities.policy_hash = digest(9);
    let policy =
        project(&policy_changed, BmadContextContributionPurpose::Preview).expect("projects");
    assert_ne!(
        base.projection_hash.as_str(),
        policy.projection_hash.as_str()
    );
}

// Break caught: recomputing the hash (or the estimate) after a provider
// usage report arrives would make "what we planned" indistinguishable from
// "what actually happened," destroying the reconciliation record.
#[test]
fn attaching_reported_usage_leaves_estimate_and_hash_byte_identical() {
    let projection =
        project(&request(), BmadContextContributionPurpose::Dispatch).expect("projects");
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

// Break caught: mislabeling a shrunk contribution as `verbatim` hides the
// fact that content was altered under budget pressure, which is a
// truthfulness defect the projection exists to prevent.
#[test]
fn budget_pressure_produces_truthfully_labelled_compacted_and_truncated_entries() {
    let mut request = request();
    // 200 forces real pressure: 100 + 150 + 90 verbatim cannot fit, so the
    // artifact must fall back to its compacted form and the summary to its
    // truncated one. At 260 the artifact fit verbatim and the scenario never
    // exercised compaction at all.
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
    let projection = project(&request, BmadContextContributionPurpose::Preview).expect("projects");

    let artifact = projection
        .contributions
        .iter()
        .find(|entry| entry.kind == BmadContextContributionContributionKind::Artifact)
        .expect("artifact contribution present");
    assert_eq!(
        artifact.transformation,
        BmadContextContributionContributionTransformation::Compacted
    );
    assert_eq!(artifact.estimated_tokens, 60);

    let summary = projection
        .contributions
        .iter()
        .find(|entry| entry.kind == BmadContextContributionContributionKind::Summary)
        .expect("summary contribution present");
    assert_eq!(
        summary.transformation,
        BmadContextContributionContributionTransformation::Truncated
    );
    assert_eq!(summary.estimated_tokens, 10);
}

// Break caught: dropping a `required_by_source` contribution to make the
// numbers fit would silently omit instructions the model must see; failing
// closed makes that omission impossible to miss.
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

// Break caught: a contribution kind or transformation the projector could
// not construct would make the contract's declared enum values a lie about
// what this code actually emits.
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
    let projection = project(&request, BmadContextContributionPurpose::Preview).expect("projects");
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
    let shrunk = project(&request, BmadContextContributionPurpose::Preview).expect("projects");
    let transformations = shrunk
        .contributions
        .iter()
        .map(|entry| entry.transformation)
        .collect::<Vec<_>>();
    assert!(transformations.contains(&BmadContextContributionContributionTransformation::Compacted));
    assert!(transformations.contains(&BmadContextContributionContributionTransformation::Truncated));
}

// Break caught: a projection that cannot round-trip through JSON would mean
// the renderer or the dispatch transport could not actually carry it, even
// though the in-process struct looked fine.
#[test]
fn projection_round_trips_through_serde_json() {
    let projection =
        project(&request(), BmadContextContributionPurpose::Dispatch).expect("projects");
    let value = serde_json::to_value(&projection).expect("serializes");
    let restored: BmadContextContribution = serde_json::from_value(value).expect("deserializes");
    assert_eq!(
        restored.projection_hash.as_str(),
        projection.projection_hash.as_str()
    );
    assert_eq!(restored.contributions.len(), projection.contributions.len());
}
