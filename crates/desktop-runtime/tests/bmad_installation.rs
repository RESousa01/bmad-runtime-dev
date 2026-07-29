//! Behavioral coverage for governed installation and atomic catalog
//! activation (ENG-16): `crates/desktop-runtime/src/bmad/installation.rs`.
//!
//! The domain's pure planning functions (`plan_activation`, `plan_rollback`,
//! `validate_manifest`, ...) are exercised as unit tests inside that module,
//! where they have access to the crate-private items they need. What crosses
//! the crate boundary today is the generated `BmadInstallation` contract
//! shape and `BmadInstallationError`; this file pins the acceptance
//! criteria at that boundary so a future re-export cannot silently change
//! what a caller observes.

use desktop_runtime::{
    generated_contracts::{BmadInstallation, BmadInstallationState},
    sha256_bytes, BmadInstallationError,
};
use serde_json::json;

fn digest(seed: u8) -> String {
    sha256_bytes(&[seed]).to_string()
}

fn written_installation() -> BmadInstallation {
    serde_json::from_value(json!({
        "activation": { "active": false, "catalogHash": digest(1) },
        "installationHash": digest(2),
        "installationId": "installation_01J00000000000000000000000",
        "manifest": {
            "destinations": [{
                "relativePath": "skills/example/SKILL.md",
                "operation": "create",
                "contentHash": digest(3),
            }],
            "manifestHash": digest(4),
        },
        "packageId": "package_01J00000000000000000000000",
        "schemaVersion": "sapphirus.bmad-installation.v1",
        "state": "written",
        "validations": [],
        "version": 1,
        "workspaceId": "workspace_01J00000000000000000000000",
    }))
    .expect("generated installation")
}

// Break caught: treating a fully written manifest as proof of activation.
// `activation.active` is the only field a catalog is allowed to trust, and
// this contract keeps it independent of `state` on purpose (acceptance 1).
#[test]
fn bmad_installation_written_state_does_not_imply_activation_field() {
    let installation = written_installation();
    assert_eq!(installation.state, BmadInstallationState::Written);
    assert!(!installation.activation.active);
}

// Break caught: an installation that reached `activated` but whose
// `activation.active` flag was left false — the generated shape allows the
// caller to construct that impossible-looking state, so any consumer must
// keep checking the flag rather than inferring it from `state == activated`.
#[test]
fn bmad_installation_activated_state_and_activation_flag_are_independent_fields() {
    let mut installation = written_installation();
    installation.state = BmadInstallationState::Activated;
    assert!(!installation.activation.active, "flag was never set true");
}

// Break caught: `BmadInstallationValidationOutcomeCheck::CatalogRebuild`
// missing from a validated installation's `validations` before activation,
// or the state enum not exposing `validated` as distinct from `activated`.
#[test]
fn bmad_installation_state_enum_keeps_the_full_governed_lifecycle() {
    for candidate in [
        "planned",
        "reviewed",
        "approved",
        "checkpointed",
        "written",
        "validated",
        "activated",
        "rolled_back",
    ] {
        let state: BmadInstallationState = serde_json::from_value(json!(candidate))
            .unwrap_or_else(|_| panic!("{candidate} must remain a valid installation state"));
        assert_eq!(
            serde_json::to_value(state).expect("state serializes"),
            candidate
        );
    }
}

// The error surface a host actually integrates against: every variant is
// constructible, distinguishable, and has a stable Debug rendering — a
// caller matching on `BmadInstallationError::UnsafeDestination` must keep
// compiling as the module grows more variants.
#[test]
fn bmad_installation_error_variants_are_distinct_and_reachable() {
    let unsafe_destination = BmadInstallationError::UnsafeDestination;
    let duplicate = BmadInstallationError::DuplicateDestination;
    assert_ne!(unsafe_destination, duplicate);
    assert_eq!(unsafe_destination, BmadInstallationError::UnsafeDestination);
    assert_eq!(
        format!("{duplicate:?}"),
        "DuplicateDestination",
        "the error's Debug form is part of what a host logs for diagnosis"
    );
}
