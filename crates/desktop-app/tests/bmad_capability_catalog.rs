#[allow(dead_code)]
mod generated {
    include!("../src/generated/bmad_capabilities.rs");
}

#[test]
fn bmad_capability_catalog_accounts_for_pinned_sources_and_safe_ui_projection() {
    assert_eq!(generated::METHOD_CAPABILITY_ROW_COUNT, 48);
    assert_eq!(generated::BUILDER_PUBLIC_ACTION_ROW_COUNT, 9);
    assert_eq!(generated::BUILDER_INTERNAL_ACTION_ROW_COUNT, 1);
    assert_eq!(generated::CAPABILITY_ROW_COUNT, 58);

    for alias in generated::COMPATIBILITY_ALIASES {
        assert!(!alias.alias.is_empty());
        assert!(!alias.target.is_empty());
        assert!(!alias.migration_policy.is_empty());
        assert!(generated::CAPABILITIES
            .iter()
            .any(|capability| capability.canonical_id == alias.target));
    }

    assert!(generated::UI_ADVERTISED_CAPABILITIES
        .iter()
        .all(|capability| capability.visibility == "public"
            && capability.support_state == "runtime_available"));
}
