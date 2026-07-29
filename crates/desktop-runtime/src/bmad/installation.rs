//! Governed installation and atomic catalog activation (ENG-16).
//!
//! Without this module a written package could be mistaken for a served one:
//! nothing would stop a caller from treating "every file landed on disk" as
//! "the catalog now resolves this version". The two are kept as separate
//! facts on purpose — `state` records how far the write pipeline got,
//! `activation.active` records what the catalog is actually allowed to
//! serve — so a package can sit fully `written` on disk and still be inert.
//! This module computes the pure transitions (path safety, activation
//! eligibility, rollback, supersession); it never touches a filesystem or a
//! database, so the same rules apply whether the host is validating a plan
//! or replaying history.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::generated_contracts::{
    BmadInstallation, BmadInstallationActivation, BmadInstallationInstallManifest,
    BmadInstallationState, BmadInstallationValidationOutcomeCheck,
};
use crate::Sha256Digest;

/// The manifest's own bound; kept in sync with the generated schema's
/// `maxItems` so a caller cannot construct a manifest the contract would
/// reject.
const MAX_DESTINATIONS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BmadInstallationError {
    /// A destination path escaped its governed root.
    #[error("installation destination path is unsafe")]
    UnsafeDestination,
    /// Two destinations in one manifest claim the same relative path.
    #[error("installation manifest has a colliding destination")]
    DuplicateDestination,
    /// A manifest carried no destinations, or more than the contract allows.
    #[error("installation manifest is empty or exceeds its bound")]
    InvalidManifestSize,
    /// Activation was attempted before the state reached `validated`.
    #[error("installation is not validated")]
    NotValidated,
    /// Activation was attempted without a passing `catalog_rebuild` check.
    #[error("installation has no passing catalog_rebuild validation")]
    MissingCatalogRebuild,
    /// Rollback was attempted on an installation with no checkpoint to
    /// restore.
    #[error("installation has no checkpoint to roll back to")]
    NotCheckpointed,
    /// Rollback was attempted on an installation that already left the
    /// governed write window.
    #[error("installation state cannot be rolled back")]
    NotRollbackable,
    /// A digest could not be represented as the contract's `CommonSha256`.
    #[error("installation hash could not be represented")]
    Hash,
}

/// One relative destination path is safe to write only if it cannot leave
/// the governed installation root: no absolute path, no backslash (which
/// would be a path separator on the host even though the schema pattern is
/// written for `/`), and no `..` component at any position.
///
/// # Errors
///
/// Returns [`BmadInstallationError::UnsafeDestination`] for an absolute
/// path, a path containing a backslash, or a path with a `.` or `..`
/// component.
pub fn validate_destination_path(relative_path: &str) -> Result<(), BmadInstallationError> {
    if relative_path.is_empty() || relative_path.starts_with('/') || relative_path.contains('\\') {
        return Err(BmadInstallationError::UnsafeDestination);
    }
    for component in relative_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(BmadInstallationError::UnsafeDestination);
        }
    }
    Ok(())
}

/// Validates a full install manifest: every destination path is safe, no
/// two destinations collide on the same relative path, and the destination
/// count stays within the contract's bound.
///
/// # Errors
///
/// Returns [`BmadInstallationError::InvalidManifestSize`] for an empty or
/// oversized manifest, [`BmadInstallationError::UnsafeDestination`] for any
/// unsafe path, and [`BmadInstallationError::DuplicateDestination`] for a
/// repeated relative path.
pub fn validate_manifest(
    manifest: &BmadInstallationInstallManifest,
) -> Result<(), BmadInstallationError> {
    if manifest.destinations.is_empty() || manifest.destinations.len() > MAX_DESTINATIONS {
        return Err(BmadInstallationError::InvalidManifestSize);
    }
    let mut seen = BTreeSet::new();
    for destination in &manifest.destinations {
        let path = destination.relative_path.to_string();
        validate_destination_path(&path)?;
        if !seen.insert(path) {
            return Err(BmadInstallationError::DuplicateDestination);
        }
    }
    Ok(())
}

/// Whether the catalog is permitted to resolve this installation.
///
/// This reads exactly one field — `activation.active` — and nothing about
/// `state`. That is the point: a `written` (or even `validated`) row that
/// has never been activated must report `false` here, so disk presence can
/// never be mistaken for activation (acceptance criterion 1).
#[must_use]
pub fn is_served_by_catalog(installation: &BmadInstallation) -> bool {
    installation.activation.active
}

/// Whether one installation currently holds a passing `catalog_rebuild`
/// validation. Activation must fail closed without this, because switching
/// the active pointer before the catalog is proven to serve the new
/// manifest would let a caller observe a catalog that cannot resolve what it
/// claims to.
#[must_use]
pub fn has_passing_catalog_rebuild(installation: &BmadInstallation) -> bool {
    installation.validations.iter().any(|outcome| {
        outcome.check == BmadInstallationValidationOutcomeCheck::CatalogRebuild && outcome.passed
    })
}

/// Checks that one installation is eligible to move from `validated` to
/// `activated`. This is the fail-closed gate: a caller must not be able to
/// activate a `written` installation directly, and must not be able to
/// activate one whose `catalog_rebuild` check has not passed.
///
/// # Errors
///
/// Returns [`BmadInstallationError::NotValidated`] when `state` is not
/// `validated`, and [`BmadInstallationError::MissingCatalogRebuild`] when no
/// passing `catalog_rebuild` validation is present.
pub fn can_activate(installation: &BmadInstallation) -> Result<(), BmadInstallationError> {
    if installation.state != BmadInstallationState::Validated {
        return Err(BmadInstallationError::NotValidated);
    }
    if !has_passing_catalog_rebuild(installation) {
        return Err(BmadInstallationError::MissingCatalogRebuild);
    }
    Ok(())
}

/// The activation record for a newly activated installation, and — when it
/// is superseding a previously active one — the demoted record for that
/// prior installation. Both are computed together so a caller can persist
/// them inside one transaction and never observe two installations active
/// at once, or the demotion applied without the promotion (or vice versa).
#[derive(Clone, Debug)]
pub struct BmadInstallationActivationPlan {
    /// The activation record to store on the installation being activated.
    pub activation: BmadInstallationActivation,
    /// The activation record to store on the installation being
    /// superseded, when one was supplied.
    pub demoted: Option<BmadInstallationActivation>,
}

/// Plans atomic activation of `installation`, optionally superseding
/// `previous_active`.
///
/// # Errors
///
/// Returns the same errors as [`can_activate`].
pub fn plan_activation(
    installation: &BmadInstallation,
    previous_active: Option<&BmadInstallation>,
    catalog_hash: Sha256Digest,
) -> Result<BmadInstallationActivationPlan, BmadInstallationError> {
    can_activate(installation)?;
    let catalog_hash = sha256_contract(catalog_hash)?;
    let activation = BmadInstallationActivation {
        activated_from_installation_id: previous_active
            .map(|previous| previous.installation_id.clone()),
        active: true,
        catalog_hash,
        superseded_installation_id: None,
    };
    let demoted = previous_active.map(|previous| BmadInstallationActivation {
        activated_from_installation_id: previous.activation.activated_from_installation_id.clone(),
        active: false,
        catalog_hash: previous.activation.catalog_hash.clone(),
        superseded_installation_id: Some(installation.installation_id.clone()),
    });
    Ok(BmadInstallationActivationPlan {
        activation,
        demoted,
    })
}

/// Plans a rollback: restores the activation record to the catalog hash
/// recorded at checkpoint time, and leaves `active` at whatever it was
/// before this attempt (a failed activation never made the row active, so
/// rollback must not flip it either way).
///
/// # Errors
///
/// Returns [`BmadInstallationError::NotCheckpointed`] when the installation
/// has no checkpoint, and [`BmadInstallationError::NotRollbackable`] when the
/// state is not one of `checkpointed`, `written`, or `validated`.
pub fn plan_rollback(
    installation: &BmadInstallation,
) -> Result<BmadInstallationActivation, BmadInstallationError> {
    let Some(checkpoint) = installation.checkpoint.as_ref() else {
        return Err(BmadInstallationError::NotCheckpointed);
    };
    if !matches!(
        installation.state,
        BmadInstallationState::Checkpointed
            | BmadInstallationState::Written
            | BmadInstallationState::Validated
    ) {
        return Err(BmadInstallationError::NotRollbackable);
    }
    Ok(BmadInstallationActivation {
        activated_from_installation_id: installation
            .activation
            .activated_from_installation_id
            .clone(),
        active: false,
        catalog_hash: checkpoint.prior_catalog_hash.clone(),
        superseded_installation_id: installation.activation.superseded_installation_id.clone(),
    })
}

fn sha256_contract(
    digest: Sha256Digest,
) -> Result<crate::generated_contracts::CommonSha256, BmadInstallationError> {
    crate::generated_contracts::CommonSha256::try_from(digest.to_string())
        .map_err(|_| BmadInstallationError::Hash)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sha256_bytes;

    fn digest(seed: u8) -> String {
        sha256_bytes(&[seed]).to_string()
    }

    /// One `written` installation whose manifest is complete on disk, but
    /// which has never been activated.
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
        .expect("fixture parses")
    }

    fn with_catalog_rebuild(mut installation: BmadInstallation, passed: bool) -> BmadInstallation {
        installation.state = BmadInstallationState::Validated;
        installation.validations.push(
            serde_json::from_value(json!({ "check": "catalog_rebuild", "passed": passed }))
                .expect("validation outcome fixture parses"),
        );
        installation
    }

    fn checkpointed(mut installation: BmadInstallation, prior_catalog: &str) -> BmadInstallation {
        installation.checkpoint = Some(
            serde_json::from_value(json!({
                "checkpointId": "checkpoint_01J00000000000000000000000",
                "priorCatalogHash": prior_catalog,
            }))
            .expect("checkpoint fixture parses"),
        );
        installation
    }

    // Break caught: disk presence (a fully `written` manifest) standing in
    // for activation. `is_served_by_catalog` must read only
    // `activation.active`, so a written-but-never-activated installation
    // reports false even though every destination exists.
    #[test]
    fn bmad_installation_written_with_all_files_present_is_not_served_by_catalog() {
        let installation = written_installation();
        assert_eq!(installation.state, BmadInstallationState::Written);
        assert!(!is_served_by_catalog(&installation));
    }

    // Break caught: activating straight from `written`, skipping the
    // `validated` gate entirely.
    #[test]
    fn bmad_installation_activation_from_written_state_fails_closed() {
        let installation = written_installation();
        let result = plan_activation(&installation, None, sha256_bytes(b"catalog"));
        assert!(matches!(result, Err(BmadInstallationError::NotValidated)));
    }

    // Break caught: activating a `validated` installation whose
    // `catalog_rebuild` check either never ran or failed. Rebuilding the
    // catalog must happen and succeed before the active pointer moves.
    #[test]
    fn bmad_installation_activation_without_passing_catalog_rebuild_fails_closed() {
        let missing = {
            let mut i = written_installation();
            i.state = BmadInstallationState::Validated;
            i
        };
        assert!(matches!(
            plan_activation(&missing, None, sha256_bytes(b"catalog")),
            Err(BmadInstallationError::MissingCatalogRebuild)
        ));

        let failed = with_catalog_rebuild(written_installation(), false);
        assert!(matches!(
            plan_activation(&failed, None, sha256_bytes(b"catalog")),
            Err(BmadInstallationError::MissingCatalogRebuild)
        ));
    }

    // The positive path: validated plus a passing catalog_rebuild check
    // activates, and with no prior active installation there is nothing to
    // demote.
    #[test]
    fn bmad_installation_activation_with_passing_catalog_rebuild_succeeds() {
        let installation = with_catalog_rebuild(written_installation(), true);
        let plan = plan_activation(&installation, None, sha256_bytes(b"catalog"))
            .expect("eligible installation activates");
        assert!(plan.activation.active);
        assert!(plan.demoted.is_none());
    }

    // Break caught: rolling back and forgetting to restore the catalog hash
    // that was recorded at checkpoint time, instead of leaving the failed
    // hash in place.
    #[test]
    fn bmad_installation_rollback_after_written_restores_checkpointed_catalog_hash() {
        let prior = digest(9);
        let installation = checkpointed(written_installation(), &prior);
        let restored = plan_rollback(&installation).expect("checkpointed installation rolls back");
        assert_eq!(restored.catalog_hash.to_string(), prior);
        assert!(!restored.active);
    }

    // Break caught: attempting rollback with no checkpoint at all, which
    // would have nothing safe to restore to.
    #[test]
    fn bmad_installation_rollback_without_checkpoint_fails_closed() {
        let installation = written_installation();
        assert!(matches!(
            plan_rollback(&installation),
            Err(BmadInstallationError::NotCheckpointed)
        ));
    }

    // Break caught: history being lost on upgrade. Superseding an active
    // installation must retain a resolvable record of the prior one — this
    // proves the demoted plan still names its own package version's
    // manifest/installation identity rather than being erased.
    #[test]
    fn bmad_installation_supersession_keeps_the_prior_installation_resolvable() {
        let previous_active = {
            let mut installation = with_catalog_rebuild(written_installation(), true);
            installation.installation_id = "installation_01J11111111111111111111111"
                .parse()
                .expect("valid id");
            installation.activation.active = true;
            installation.activation.catalog_hash = digest(5).parse().expect("valid catalog hash");
            installation
        };
        let new_installation = {
            let mut installation = with_catalog_rebuild(written_installation(), true);
            installation.installation_id = "installation_01J22222222222222222222222"
                .parse()
                .expect("valid id");
            installation
        };

        let plan = plan_activation(
            &new_installation,
            Some(&previous_active),
            sha256_bytes(b"c2"),
        )
        .expect("supersession activates");

        assert!(plan.activation.active);
        assert_eq!(
            plan.activation.activated_from_installation_id,
            Some(previous_active.installation_id.clone())
        );
        let demoted = plan.demoted.expect("previous installation is demoted");
        assert!(!demoted.active);
        assert_eq!(
            demoted.superseded_installation_id,
            Some(new_installation.installation_id.clone())
        );
        // The demoted record still carries its own original catalog hash —
        // the prior version remains resolvable at that value, not silently
        // rewritten to the new one.
        assert_eq!(
            demoted.catalog_hash,
            previous_active.activation.catalog_hash
        );
    }

    #[test]
    fn bmad_installation_path_rejects_absolute_paths() {
        assert!(matches!(
            validate_destination_path("/etc/passwd"),
            Err(BmadInstallationError::UnsafeDestination)
        ));
    }

    #[test]
    fn bmad_installation_path_rejects_parent_traversal() {
        assert!(matches!(
            validate_destination_path("skills/../../../etc/passwd"),
            Err(BmadInstallationError::UnsafeDestination)
        ));
        assert!(matches!(
            validate_destination_path(".."),
            Err(BmadInstallationError::UnsafeDestination)
        ));
    }

    #[test]
    fn bmad_installation_path_rejects_backslashes() {
        assert!(matches!(
            validate_destination_path("skills\\example\\SKILL.md"),
            Err(BmadInstallationError::UnsafeDestination)
        ));
    }

    #[test]
    fn bmad_installation_path_accepts_a_clean_relative_path() {
        assert!(validate_destination_path("skills/example/SKILL.md").is_ok());
    }

    #[test]
    fn bmad_installation_manifest_rejects_duplicate_destinations() {
        let manifest: BmadInstallationInstallManifest = serde_json::from_value(json!({
            "destinations": [
                {"relativePath": "a.md", "operation": "create", "contentHash": digest(1)},
                {"relativePath": "a.md", "operation": "replace", "contentHash": digest(2)},
            ],
            "manifestHash": digest(3),
        }))
        .expect("manifest fixture parses");
        assert!(matches!(
            validate_manifest(&manifest),
            Err(BmadInstallationError::DuplicateDestination)
        ));
    }

    #[test]
    fn bmad_installation_manifest_accepts_distinct_destinations() {
        let manifest = written_installation().manifest;
        assert!(validate_manifest(&manifest).is_ok());
    }
}
