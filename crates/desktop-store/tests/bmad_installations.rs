#![allow(clippy::expect_used)]

//! Restart recovery and atomic activation for governed installation (ENG-16).

use desktop_runtime::{
    generated_contracts::{BmadInstallation, BmadInstallationState},
    sha256_bytes,
};
use desktop_store::{BmadInstallationActivationCommit, KeyProtector, LocalStore, StoreError};
use serde_json::json;

const WORKSPACE: &str = "workspace_01J00000000000000000000000";
const PACKAGE: &str = "package_01J00000000000000000000000";

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

fn digest(seed: u8) -> String {
    sha256_bytes(&[seed]).to_string()
}

fn installation(installation_id: &str, seed: u8) -> BmadInstallation {
    serde_json::from_value(json!({
        "activation": { "active": false, "catalogHash": digest(seed) },
        "installationHash": digest(seed.wrapping_add(1)),
        "installationId": installation_id,
        "manifest": {
            "destinations": [{
                "relativePath": "skills/example/SKILL.md",
                "operation": "create",
                "contentHash": digest(seed.wrapping_add(2)),
            }],
            "manifestHash": digest(seed.wrapping_add(3)),
        },
        "packageId": PACKAGE,
        "schemaVersion": "sapphirus.bmad-installation.v1",
        "state": "planned",
        "validations": [],
        "version": 1,
        "workspaceId": WORKSPACE,
    }))
    .expect("generated installation")
}

fn advance(installation: &mut BmadInstallation, state: BmadInstallationState, active: bool) {
    installation.state = state;
    installation.activation.active = active;
    installation.version = installation
        .version
        .checked_add(1)
        .expect("version increments");
}

// Break caught: treating "every destination on disk" as proof of activation.
// A `written` installation with a complete manifest must still be absent
// from the catalog's active lookup for its package (acceptance 1).
#[test]
fn bmad_installations_written_installation_is_not_served_by_the_catalog(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = installation("installation_01J00000000000000000000000", 1);
    store.create_bmad_installation(&value, WORKSPACE, 1_700_000_000_000)?;

    for state in [
        BmadInstallationState::Reviewed,
        BmadInstallationState::Approved,
        BmadInstallationState::Checkpointed,
        BmadInstallationState::Written,
    ] {
        let from_version = value.version.get();
        advance(&mut value, state, false);
        store.commit_bmad_installation_transition(
            &value,
            "advance",
            from_version,
            1_700_000_000_001,
        )?;
    }

    assert_eq!(value.state, BmadInstallationState::Written);
    assert!(
        store
            .active_bmad_installation_for_package(PACKAGE)?
            .is_none(),
        "a written-only installation must not be resolvable as the active one"
    );
    let stored = store
        .load_bmad_installation(value.installation_id.as_str())?
        .expect("row persists");
    assert!(!stored.active);
    Ok(())
}

// Break caught: two writers racing to advance the same installation both
// succeeding, leaving an event log that cannot explain the stored state.
#[test]
fn bmad_installations_reject_a_stale_writers_transition() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = installation("installation_01J00000000000000000000000", 1);
    store.create_bmad_installation(&value, WORKSPACE, 1_700_000_000_000)?;

    advance(&mut value, BmadInstallationState::Reviewed, false);
    store.commit_bmad_installation_transition(&value, "review", 1, 1_700_000_000_001)?;

    // A second writer still believes it is at version 1, and independently
    // computed a different accepted hash for its (different) transition to
    // the same target version.
    let mut competing = installation("installation_01J00000000000000000000000", 1);
    advance(&mut competing, BmadInstallationState::Reviewed, false);
    competing.installation_hash = digest(99).parse()?;
    let refused = store
        .commit_bmad_installation_transition(&competing, "review", 1, 1_700_000_000_002)
        .expect_err("a stale writer must not commit");
    assert!(matches!(
        refused,
        StoreError::Inconsistent | StoreError::AlreadyConsumed
    ));

    let stored = store
        .load_bmad_installation(value.installation_id.as_str())?
        .expect("row");
    assert_eq!(stored.installation.version.get(), 2);
    assert_eq!(
        store
            .bmad_installation_event_log(value.installation_id.as_str())?
            .len(),
        1
    );
    Ok(())
}

// Break caught: a redelivered transition after an ambiguous crash appending
// a second identical event and desynchronizing the version chain.
#[test]
fn bmad_installations_replayed_transition_converges() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = installation("installation_01J00000000000000000000000", 1);
    store.create_bmad_installation(&value, WORKSPACE, 1_700_000_000_000)?;

    advance(&mut value, BmadInstallationState::Reviewed, false);
    store.commit_bmad_installation_transition(&value, "review", 1, 1_700_000_000_001)?;

    // The exact same accepted transition, redelivered.
    store.commit_bmad_installation_transition(&value, "review", 1, 1_700_000_000_002)?;

    let stored = store
        .load_bmad_installation(value.installation_id.as_str())?
        .expect("row");
    assert_eq!(stored.installation.version.get(), 2);
    assert_eq!(
        store
            .bmad_installation_event_log(value.installation_id.as_str())?
            .len(),
        1,
        "the replay must not append a second event"
    );
    Ok(())
}

// Break caught: activation moving the catalog pointer without validating a
// passing `catalog_rebuild` outcome first, or the demoted row's own catalog
// hash getting overwritten instead of retained (acceptance 3, upgrade path).
#[test]
fn bmad_installations_activation_supersedes_and_keeps_history_resolvable(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    // First installation reaches validated-with-catalog_rebuild and activates.
    let mut first = installation("installation_01J11111111111111111111111", 10);
    store.create_bmad_installation(&first, WORKSPACE, 1_700_000_000_000)?;
    for state in [
        BmadInstallationState::Reviewed,
        BmadInstallationState::Approved,
        BmadInstallationState::Checkpointed,
        BmadInstallationState::Written,
    ] {
        let from_version = first.version.get();
        advance(&mut first, state, false);
        store.commit_bmad_installation_transition(&first, "advance", from_version, 1)?;
    }
    let from_version = first.version.get();
    first.state = BmadInstallationState::Validated;
    first.validations = vec![serde_json::from_value(
        json!({ "check": "catalog_rebuild", "passed": true }),
    )?];
    first.version = first.version.checked_add(1).expect("version increments");
    store.commit_bmad_installation_transition(&first, "validate", from_version, 1)?;

    let activated_from_version = first.version.get();
    first.state = BmadInstallationState::Activated;
    first.activation.active = true;
    first.activation.catalog_hash = digest(50).parse()?;
    first.version = first.version.checked_add(1).expect("version increments");
    store.commit_bmad_installation_activation(
        &BmadInstallationActivationCommit {
            activated: &first,
            activated_from_version,
            demoted: None,
            demoted_from_version: None,
        },
        1,
    )?;

    assert_eq!(
        store
            .active_bmad_installation_for_package(PACKAGE)?
            .expect("first installation is active")
            .installation
            .installation_id,
        first.installation_id
    );

    // Second installation upgrades the package: it activates and, in the
    // same transaction, demotes the first.
    let mut second = installation("installation_01J22222222222222222222222", 20);
    store.create_bmad_installation(&second, WORKSPACE, 1)?;
    for state in [
        BmadInstallationState::Reviewed,
        BmadInstallationState::Approved,
        BmadInstallationState::Checkpointed,
        BmadInstallationState::Written,
    ] {
        let from_version = second.version.get();
        advance(&mut second, state, false);
        store.commit_bmad_installation_transition(&second, "advance", from_version, 1)?;
    }
    let from_version = second.version.get();
    second.state = BmadInstallationState::Validated;
    second.validations = vec![serde_json::from_value(
        json!({ "check": "catalog_rebuild", "passed": true }),
    )?];
    second.version = second.version.checked_add(1).expect("version increments");
    store.commit_bmad_installation_transition(&second, "validate", from_version, 1)?;

    let second_activated_from = second.version.get();
    second.state = BmadInstallationState::Activated;
    second.activation.active = true;
    second.activation.catalog_hash = digest(60).parse()?;
    second.activation.activated_from_installation_id = Some(first.installation_id.clone());
    second.version = second.version.checked_add(1).expect("version increments");

    let mut demoted_first = first.clone();
    let demoted_from_version = demoted_first.version.get();
    demoted_first.activation.active = false;
    demoted_first.activation.superseded_installation_id = Some(second.installation_id.clone());
    demoted_first.version = demoted_first
        .version
        .checked_add(1)
        .expect("version increments");

    store.commit_bmad_installation_activation(
        &BmadInstallationActivationCommit {
            activated: &second,
            activated_from_version: second_activated_from,
            demoted: Some(&demoted_first),
            demoted_from_version: Some(demoted_from_version),
        },
        1,
    )?;

    // The catalog now serves only the second installation...
    let active = store
        .active_bmad_installation_for_package(PACKAGE)?
        .expect("second installation is active");
    assert_eq!(active.installation.installation_id, second.installation_id);

    // ...but the first installation's row is retained, still resolvable by
    // id, still reporting its own manifest destinations and its own
    // (unaltered) catalog hash rather than the new one.
    let history = store
        .load_bmad_installation(first.installation_id.as_str())?
        .expect("superseded installation remains resolvable");
    assert!(!history.active);
    assert_eq!(
        history.installation.activation.catalog_hash,
        first.activation.catalog_hash
    );
    assert_eq!(
        history.installation.manifest.destinations.len(),
        first.manifest.destinations.len()
    );
    assert_eq!(
        history.installation.activation.superseded_installation_id,
        Some(second.installation_id.clone())
    );
    Ok(())
}

// Break caught: rollback leaving the row promoted, or restoring a catalog
// hash other than the one recorded at checkpoint time (acceptance 2).
#[test]
fn bmad_installations_rollback_restores_the_checkpointed_catalog_hash(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut value = installation("installation_01J00000000000000000000000", 1);
    let prior_catalog_hash = value.activation.catalog_hash.clone();
    store.create_bmad_installation(&value, WORKSPACE, 1)?;

    for state in [
        BmadInstallationState::Reviewed,
        BmadInstallationState::Approved,
        BmadInstallationState::Checkpointed,
        BmadInstallationState::Written,
    ] {
        let from_version = value.version.get();
        advance(&mut value, state, false);
        store.commit_bmad_installation_transition(&value, "advance", from_version, 1)?;
    }

    // A post-write failure is discovered; roll back. The catalog hash must
    // return to exactly what the checkpoint recorded, never the value a
    // half-finished write left behind.
    let from_version = value.version.get();
    value.state = BmadInstallationState::RolledBack;
    value.activation.active = false;
    value.activation.catalog_hash = prior_catalog_hash.clone();
    value.version = value.version.checked_add(1).expect("version increments");
    store.commit_bmad_installation_rollback(&value, from_version, 1)?;

    let stored = store
        .load_bmad_installation(value.installation_id.as_str())?
        .expect("row");
    assert_eq!(stored.installation.state, BmadInstallationState::RolledBack);
    assert!(!stored.active);
    assert_eq!(
        stored.installation.activation.catalog_hash,
        prior_catalog_hash
    );
    Ok(())
}

// Break caught: a lifecycle held only in memory silently forgets its state,
// version, and (most importantly) its activation pointer after a crash.
#[test]
fn bmad_installations_restart_recovery_preserves_state_version_and_activation(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let mut value = installation("installation_01J00000000000000000000000", 1);

    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.create_bmad_installation(&value, WORKSPACE, 1)?;
        for state in [
            BmadInstallationState::Reviewed,
            BmadInstallationState::Approved,
            BmadInstallationState::Checkpointed,
            BmadInstallationState::Written,
        ] {
            let from_version = value.version.get();
            advance(&mut value, state, false);
            store.commit_bmad_installation_transition(&value, "advance", from_version, 1)?;
        }
        let from_version = value.version.get();
        value.state = BmadInstallationState::Validated;
        value.validations = vec![serde_json::from_value(
            json!({ "check": "catalog_rebuild", "passed": true }),
        )?];
        value.version = value.version.checked_add(1).expect("version increments");
        store.commit_bmad_installation_transition(&value, "validate", from_version, 1)?;

        let activated_from_version = value.version.get();
        value.state = BmadInstallationState::Activated;
        value.activation.active = true;
        value.version = value.version.checked_add(1).expect("version increments");
        store.commit_bmad_installation_activation(
            &BmadInstallationActivationCommit {
                activated: &value,
                activated_from_version,
                demoted: None,
                demoted_from_version: None,
            },
            1,
        )?;
    }

    // Reopen: a different process, a cold store, no in-memory carry-over.
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let recovered = store
        .load_bmad_installation(value.installation_id.as_str())?
        .expect("installation survives restart");
    assert_eq!(
        recovered.installation.state,
        BmadInstallationState::Activated
    );
    assert_eq!(recovered.installation.version.get(), value.version.get());
    assert!(recovered.active);
    assert!(recovered.installation.activation.active);
    assert_eq!(
        store
            .active_bmad_installation_for_package(PACKAGE)?
            .expect("recovered activation pointer resolves")
            .installation
            .installation_id,
        value.installation_id
    );
    Ok(())
}
