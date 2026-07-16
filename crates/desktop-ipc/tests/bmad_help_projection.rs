#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use desktop_ipc::{
    project_bmad_help_recommendation, BmadHelpProjectionError, MAX_BMAD_HELP_RECOMMENDATION_BYTES,
};
use desktop_runtime::{
    sha256_bytes, BmadCatalogAvailability, BmadEntrypointKind, BmadHelpActionKey,
    BmadHelpConfidence, BmadHelpRecommendation, BmadHelpSourceRef, BmadLoadedPackage,
    BmadLoadedSkill, ContractId,
};
fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("test identifier")
}

fn recommendation() -> BmadHelpRecommendation {
    let action = BmadHelpActionKey {
        capability_catalog_hash: sha256_bytes(b"LEAK_CANARY_CAPABILITY_CATALOG"),
        package_version_id: id("pkgver_LEAK_CANARY_PACKAGE_VERSION"),
        module_code: "bmm".to_owned(),
        skill_name: "bmad-architecture".to_owned(),
        action: Some("create".to_owned()),
    };
    BmadHelpRecommendation {
        action: action.clone(),
        display_name: "Create Architecture".to_owned(),
        reason: "The current intent matches architecture planning.".to_owned(),
        confidence: BmadHelpConfidence::Unknown,
        availability: BmadCatalogAvailability::CapabilityDisabled,
        required_guidance: true,
        expected_outputs: vec!["architecture".to_owned()],
        source_refs: vec![BmadHelpSourceRef {
            capability_catalog_hash: action.capability_catalog_hash,
            package_version_id: action.package_version_id.clone(),
            module_code: action.module_code.clone(),
            skill_name: action.skill_name.clone(),
            action: action.action.clone(),
            source_ordinal: 9_876_543,
        }],
        blocker_codes: vec!["bmad_capability_disabled".to_owned()],
        alternatives: vec![BmadHelpActionKey {
            capability_catalog_hash: sha256_bytes(b"LEAK_CANARY_ALTERNATIVE_HASH"),
            package_version_id: id("pkgver_LEAK_CANARY_ALTERNATIVE_PACKAGE"),
            module_code: "authority_model_config_path_cas_prompt_canary".to_owned(),
            skill_name: "leak_canary_alternative_skill".to_owned(),
            action: Some("leak_canary_alternative_action".to_owned()),
        }],
        completion_claimed: false,
    }
}

fn package() -> BmadLoadedPackage {
    BmadLoadedPackage {
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: id("pkgver_LEAK_CANARY_PACKAGE_VERSION"),
        descriptor_hash: sha256_bytes(b"hidden descriptor"),
        observed_inventory_hash: sha256_bytes(b"hidden inventory"),
        skills: vec![BmadLoadedSkill {
            module_code: "bmm".to_owned(),
            skill_name: "bmad-architecture".to_owned(),
            display_name: "Create Architecture".to_owned(),
            description: "Create architecture.".to_owned(),
            entrypoint_kind: BmadEntrypointKind::StepJit,
            actions: vec!["create".to_owned()],
            distribution_profile: "sapphirus_package".to_owned(),
            install_profile: "SapphirusManagedV1".to_owned(),
            validation_profile: "MethodStepWorkflowV6".to_owned(),
            execution_profile_hash: sha256_bytes(b"hidden execution profile"),
            capability_enabled: false,
            structurally_eligible: false,
        }],
    }
}

#[test]
fn recommendation_projection_is_exact_inert_and_disclosure_safe() {
    let internal = recommendation();
    let hidden_hashes = [
        internal.action.capability_catalog_hash.to_string(),
        internal.alternatives[0].capability_catalog_hash.to_string(),
    ];
    let projection =
        project_bmad_help_recommendation(&package(), &internal).expect("safe projection");

    assert_eq!(projection.schema_version, "bmad-help-recommendation.v1");
    assert_eq!(projection.display_name, "Create Architecture");
    assert_eq!(projection.module_code, "bmm");
    assert_eq!(projection.skill_name, "bmad-architecture");
    assert_eq!(projection.action.as_deref(), Some("create"));
    assert_eq!(projection.source.package_name, "bmad-method");
    assert_eq!(projection.source.package_version, "6.10.0");
    assert!(projection.required_guidance);
    assert_eq!(projection.expected_artifacts, ["architecture"]);
    assert!(!projection.completion_claimed);

    let value = serde_json::to_value(&projection).expect("projection JSON");
    let keys = value
        .as_object()
        .expect("projection object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "action",
            "availability",
            "blockerCodes",
            "completionClaimed",
            "confidence",
            "displayName",
            "expectedArtifacts",
            "moduleCode",
            "reason",
            "requiredGuidance",
            "schemaVersion",
            "skillName",
            "source",
        ])
    );

    let json = serde_json::to_string(&value).expect("projection text");
    for forbidden in [
        "packageVersionId",
        "capabilityCatalogHash",
        "sourceRefs",
        "sourceOrdinal",
        "alternatives",
        "leak_canary_alternative",
        "authority_model_config_path_cas_prompt_canary",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}");
    }
    for hidden_hash in hidden_hashes {
        assert!(!json.contains(&hidden_hash), "leaked internal hash");
    }
    assert!(json.len() <= MAX_BMAD_HELP_RECOMMENDATION_BYTES);
}

#[test]
fn recommendation_projection_rejects_unsafe_or_oversized_fields_without_truncation() {
    let mut cases = Vec::new();

    let mut invalid = recommendation();
    invalid.display_name = "x".repeat(257);
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.action.module_code = "unsafe module".to_owned();
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.action.skill_name = "skill\u{202e}name".to_owned();
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.action.action = Some("bad\naction".to_owned());
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.reason = "r".repeat(4_097);
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.reason = "unsafe\u{2066}reason".to_owned();
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.expected_outputs = vec!["artifact".to_owned(); 17];
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.expected_outputs = vec!["x".repeat(257)];
    cases.push(invalid);

    let mut invalid = recommendation();
    invalid.expected_outputs = vec!["unsafe\u{0000}artifact".to_owned()];
    cases.push(invalid);

    for invalid in cases {
        assert_eq!(
            project_bmad_help_recommendation(&package(), &invalid),
            Err(BmadHelpProjectionError::Unavailable)
        );
    }

    let mut at_bounds = recommendation();
    at_bounds.display_name = "d".repeat(256);
    at_bounds.reason = "r".repeat(4_096);
    at_bounds.expected_outputs = vec!["a".repeat(256); 16];
    let projected =
        project_bmad_help_recommendation(&package(), &at_bounds).expect("inclusive limits");
    assert_eq!(projected.display_name.len(), 256);
    assert_eq!(projected.reason.len(), 4_096);
    assert_eq!(projected.expected_artifacts.len(), 16);
    assert!(
        serde_json::to_vec(&projected)
            .expect("bounded projection JSON")
            .len()
            <= MAX_BMAD_HELP_RECOMMENDATION_BYTES
    );
}

#[test]
fn recommendation_projection_never_turns_internal_state_into_completion() {
    let mut completed = recommendation();
    completed.completion_claimed = true;
    assert_eq!(
        project_bmad_help_recommendation(&package(), &completed),
        Err(BmadHelpProjectionError::Unavailable)
    );

    let projected =
        project_bmad_help_recommendation(&package(), &recommendation()).expect("inert projection");
    assert!(!projected.completion_claimed);
}

#[test]
fn recommendation_projection_uses_only_closed_confidence_availability_and_blocker_values() {
    for (confidence, expected) in [
        (BmadHelpConfidence::Authoritative, "authoritative"),
        (BmadHelpConfidence::UserAsserted, "user_asserted"),
        (BmadHelpConfidence::Heuristic, "heuristic"),
        (BmadHelpConfidence::Contextual, "contextual"),
        (BmadHelpConfidence::Unknown, "unknown"),
    ] {
        let mut internal = recommendation();
        internal.confidence = confidence;
        let value = serde_json::to_value(
            project_bmad_help_recommendation(&package(), &internal).expect("closed confidence"),
        )
        .expect("projection JSON");
        assert_eq!(value["confidence"], expected);
    }

    for (availability, blocker, expected_availability) in [
        (BmadCatalogAvailability::Available, None, "available"),
        (
            BmadCatalogAvailability::CapabilityDisabled,
            Some("bmad_capability_disabled"),
            "capability_disabled",
        ),
        (
            BmadCatalogAvailability::DependencyUnavailable,
            Some("bmad_dependency_unavailable"),
            "dependency_unavailable",
        ),
        (
            BmadCatalogAvailability::OrphanSkill,
            Some("bmad_help_catalog_orphan"),
            "orphan_skill",
        ),
        (
            BmadCatalogAvailability::NetworkUnavailable,
            Some("bmad_network_reference_unavailable"),
            "network_unavailable",
        ),
        (
            BmadCatalogAvailability::SourcePromptUnavailable,
            Some("bmad_source_prompt_unavailable"),
            "source_prompt_unavailable",
        ),
    ] {
        let mut internal = recommendation();
        internal.availability = availability;
        internal.blocker_codes = blocker.into_iter().map(str::to_owned).collect();
        let value = serde_json::to_value(
            project_bmad_help_recommendation(&package(), &internal).expect("closed availability"),
        )
        .expect("projection JSON");
        assert_eq!(value["availability"], expected_availability);
        assert_eq!(
            value["blockerCodes"]
                .as_array()
                .expect("blocker array")
                .len(),
            usize::from(blocker.is_some())
        );
    }

    for blocker_codes in [
        vec!["unknown_blocker".to_owned()],
        vec![
            "bmad_capability_disabled".to_owned(),
            "bmad_capability_disabled".to_owned(),
        ],
        vec!["bmad_capability_disabled".to_owned(); 9],
    ] {
        let mut invalid = recommendation();
        invalid.blocker_codes = blocker_codes;
        assert_eq!(
            project_bmad_help_recommendation(&package(), &invalid),
            Err(BmadHelpProjectionError::Unavailable)
        );
    }

    let mut inconsistent = recommendation();
    inconsistent.availability = BmadCatalogAvailability::Available;
    assert_eq!(
        project_bmad_help_recommendation(&package(), &inconsistent),
        Err(BmadHelpProjectionError::Unavailable)
    );

    let mut substituted = package();
    substituted.package_version_id = id("pkgver_SUBSTITUTED_PACKAGE_VERSION");
    assert_eq!(
        project_bmad_help_recommendation(&substituted, &recommendation()),
        Err(BmadHelpProjectionError::Unavailable)
    );
    assert_eq!(
        BmadHelpProjectionError::Unavailable.code(),
        "bmad_projection_unavailable"
    );
}
