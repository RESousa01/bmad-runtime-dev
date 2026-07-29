//! The Foundry model command lane.
//!
//! Split out of `commands.rs` rather than appended to it. Six commands' worth
//! of catalog projection, selection resolution and response lifecycle is its
//! own subject, and `commands.rs` is already long enough that adding it there
//! would bury the one thing worth noticing about this lane: every host
//! decision about *which model answers* is made here, and none of it is
//! representable in anything the renderer sends.
//!
//! This is a child of `commands`, so it reaches that module's private helpers
//! (`map_workspace_error`, the wire types) directly. The dispatch arms stay in
//! `commands.rs` with every other command — the routing table is one table.

use desktop_runtime::{ContractId, LocalError, LocalErrorCode, UnixMillis};

use super::map_workspace_error;
use crate::state::{HostState, RendererSessionGuard};
use crate::wire::HostCommandData;

/// Turns the held catalog into the renderer projection.
///
/// Everything the canonical profile carries beyond identity and display text —
/// `capabilities`, `limits`, `provider`, `qualifiedAt`, the deployment snapshot
/// hash, the policy hash — is dropped here rather than filtered downstream. The
/// projection type has no field for any of it, so this function could not pass
/// one along even by mistake.
fn catalog_projection_input(
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    app_epoch: u64,
    held: &crate::state::HeldModelCatalog,
) -> desktop_ipc::ModelCatalogInput {
    let verified = held.cached.catalog();
    let catalog = verified.catalog();
    let profiles = catalog
        .profiles
        .iter()
        .map(|profile| desktop_ipc::ModelProfileInput {
            profile_id: profile.profile_id.as_str().to_owned(),
            display_name: profile.display_name.as_str().to_owned(),
            summary: Some(profile.description.as_str().to_owned()),
            variant_ids: profile
                .variants
                .iter()
                .map(|variant| variant.variant_id.as_str().to_owned())
                .collect(),
        })
        .collect();
    let unavailable_profiles = catalog
        .unavailable_profiles
        .iter()
        .map(|profile| desktop_ipc::ModelUnavailableProfileInput {
            profile_id: profile.profile_id.as_str().to_owned(),
            display_name: profile.display_name.as_str().to_owned(),
            availability: map_blocked_availability(profile.availability),
            recovery_action: map_recovery_action(profile.recovery_action),
        })
        .collect();
    desktop_ipc::ModelCatalogInput {
        workspace_id: workspace_id.clone(),
        workspace_grant_epoch,
        app_epoch,
        catalog_epoch: held.catalog_epoch,
        catalog_version: verified.catalog_version().to_owned(),
        catalog_state: match verified.catalog_state() {
            desktop_runtime::generated_contracts::ModelCatalogCatalogState::Fresh => {
                desktop_ipc::ModelCatalogStateProjection::Fresh
            }
            desktop_runtime::generated_contracts::ModelCatalogCatalogState::StaleUsable => {
                desktop_ipc::ModelCatalogStateProjection::Stale
            }
        },
        default_profile_id: verified.default_profile_id().map(str::to_owned),
        profiles,
        unavailable_profiles,
        generated_at: verified.generated_at(),
        expires_at: verified.expires_at(),
        refreshed_at: held.refreshed_at,
    }
}

fn map_blocked_availability(
    availability: desktop_runtime::generated_contracts::ModelCatalogBlockedAvailability,
) -> desktop_ipc::ModelBlockedAvailabilityProjection {
    use desktop_runtime::generated_contracts::ModelCatalogBlockedAvailability as Blocked;
    match availability {
        Blocked::PolicyBlocked => desktop_ipc::ModelBlockedAvailabilityProjection::PolicyBlocked,
        Blocked::NotEntitled => desktop_ipc::ModelBlockedAvailabilityProjection::NotEntitled,
        Blocked::Unqualified => desktop_ipc::ModelBlockedAvailabilityProjection::Unqualified,
        Blocked::DeploymentMissing => {
            desktop_ipc::ModelBlockedAvailabilityProjection::DeploymentMissing
        }
        Blocked::CatalogStale => desktop_ipc::ModelBlockedAvailabilityProjection::CatalogStale,
    }
}

fn map_recovery_action(
    action: desktop_runtime::generated_contracts::ModelCatalogModelRecoveryAction,
) -> desktop_ipc::ModelRecoveryActionProjection {
    use desktop_runtime::generated_contracts::ModelCatalogModelRecoveryAction as Recovery;
    match action {
        Recovery::RefreshCatalog => desktop_ipc::ModelRecoveryActionProjection::RefreshCatalog,
        Recovery::RequestEntitlement => {
            desktop_ipc::ModelRecoveryActionProjection::RequestEntitlement
        }
        Recovery::AwaitingQualification => {
            desktop_ipc::ModelRecoveryActionProjection::AwaitingQualification
        }
        Recovery::ContactAdministrator => {
            desktop_ipc::ModelRecoveryActionProjection::ContactAdministrator
        }
        Recovery::SelectAnotherProfile => {
            desktop_ipc::ModelRecoveryActionProjection::SelectAnotherProfile
        }
    }
}

/// Projects the catalog the host already holds. Never reaches the network.
pub(super) fn model_catalog_data(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    _read_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let _workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let _ = &authority;
    let Some(held) = state.held_model_catalog() else {
        return Ok(HostCommandData::ModelCatalogUnavailable);
    };
    let input = catalog_projection_input(
        workspace_id,
        workspace_grant_epoch,
        state.model_auth_epoch(),
        &held,
    );
    desktop_ipc::project_model_catalog(input)
        .map(HostCommandData::ModelCatalog)
        .map_err(map_model_catalog_projection_error)
}

/// Re-establishes the catalog.
///
/// The renderer names no origin, no credential and no deployment; which support
/// plane is contacted, and with what authority, is entirely the host's. Until
/// the shell composes that transport the held catalog is dropped and the
/// command reports unavailable — the same fail-closed seam the production model
/// round trip uses, rather than degrading to a locally invented catalog the
/// renderer could not tell apart from a real one.
pub(super) fn refresh_model_catalog(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    read_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    {
        let authority = state.ready_authority()?;
        let _workspace_authority = state
            .workspace
            .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
            .map_err(|error| map_workspace_error(&error))?;
        let _ = &authority;
    }
    state.invalidate_model_catalog();
    model_catalog_data(state, workspace_id, workspace_grant_epoch, read_at)
}

/// Projects which model one workspace would use right now.
pub(super) fn model_selection_data(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let _workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let outcome = state.resolve_catalog_selection(&authority, workspace_id)?;
    selection_data(state, workspace_id, workspace_grant_epoch, outcome)
}

/// The one place a [`CatalogSelectionOutcome`] becomes renderer data.
///
/// `RequiresExplicitChoice` is carried through as itself here and at every
/// caller. It is never turned into a default, into the catalog's first profile,
/// or into anything from the user's history: a user part-way through a draft
/// must not discover afterwards that a different model answered.
fn selection_data(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    outcome: crate::state::CatalogSelectionOutcome,
) -> Result<HostCommandData, LocalError> {
    use crate::state::CatalogSelectionOutcome as Outcome;
    let app_epoch = state.model_auth_epoch();
    let (catalog_epoch, catalog_version, projected) = match outcome {
        Outcome::CatalogUnavailable => return Ok(HostCommandData::ModelCatalogUnavailable),
        Outcome::Selected {
            selection,
            source,
            catalog_epoch,
            catalog_version,
        } => (
            catalog_epoch,
            catalog_version,
            desktop_ipc::ModelSelectionOutcomeInput::Selected {
                profile_id: selection.profile_id().to_owned(),
                variant_id: selection.variant_id().to_owned(),
                source: map_selection_source(source),
            },
        ),
        Outcome::RequiresExplicitChoice {
            previous,
            blocked,
            catalog_epoch,
            catalog_version,
        } => (
            catalog_epoch,
            catalog_version,
            desktop_ipc::ModelSelectionOutcomeInput::RequiresExplicitChoice {
                previous_profile_id: previous.profile_id,
                previous_variant_id: previous.variant_id,
                blocked: map_selection_blocked(blocked),
            },
        ),
        Outcome::NoneSelectable {
            catalog_epoch,
            catalog_version,
        } => (
            catalog_epoch,
            catalog_version,
            desktop_ipc::ModelSelectionOutcomeInput::NoneSelectable,
        ),
    };
    desktop_ipc::project_model_selection(desktop_ipc::ModelSelectionInput {
        workspace_id: workspace_id.clone(),
        workspace_grant_epoch,
        app_epoch,
        catalog_epoch,
        catalog_version,
        outcome: projected,
    })
    .map(HostCommandData::ModelSelection)
    .map_err(map_model_catalog_projection_error)
}

fn map_selection_source(
    source: desktop_cloud::ModelSelectionSource,
) -> desktop_ipc::ModelSelectionSourceProjection {
    match source {
        desktop_cloud::ModelSelectionSource::CurrentSelection => {
            desktop_ipc::ModelSelectionSourceProjection::CurrentSelection
        }
        desktop_cloud::ModelSelectionSource::RecentSelection => {
            desktop_ipc::ModelSelectionSourceProjection::RecentSelection
        }
        desktop_cloud::ModelSelectionSource::CatalogDefault => {
            desktop_ipc::ModelSelectionSourceProjection::CatalogDefault
        }
        desktop_cloud::ModelSelectionSource::FirstSelectableProfile => {
            desktop_ipc::ModelSelectionSourceProjection::FirstSelectableProfile
        }
    }
}

fn map_selection_blocked(
    blocked: desktop_cloud::ModelSelectionBlocked,
) -> desktop_ipc::ModelSelectionBlockedProjection {
    match blocked {
        desktop_cloud::ModelSelectionBlocked::ProfileRemoved => {
            desktop_ipc::ModelSelectionBlockedProjection::ProfileRemoved
        }
        desktop_cloud::ModelSelectionBlocked::ProfileUnavailable { .. } => {
            desktop_ipc::ModelSelectionBlockedProjection::ProfileUnavailable
        }
        desktop_cloud::ModelSelectionBlocked::VariantRemoved => {
            desktop_ipc::ModelSelectionBlockedProjection::VariantRemoved
        }
    }
}

/// Starts one task-bound model response.
///
/// The order matters. Authority is proven, then the selection is resolved, and
/// only a *resolved* selection reaches the dispatch seam. When the user's own
/// selection can no longer be honored this returns the selection projection
/// rather than an error: the draft is untouched, the renderer is told exactly
/// which profile was lost and why, and nothing here picks a replacement.
pub(super) fn send_model_response(
    state: &HostState,
    _renderer_session: &RendererSessionGuard<'_>,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    accepted_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let _workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let outcome = state.resolve_catalog_selection(&authority, workspace_id)?;
    match outcome {
        crate::state::CatalogSelectionOutcome::Selected {
            ref selection,
            source,
            ..
        } => {
            // Committing to a model is what makes the explicit-choice guard
            // real. Resolution falls back to the catalog default or the first
            // selectable profile only while the workspace has *no* current
            // selection; if that never became current, a later catalog that
            // withdrew the profile would quietly fall back again instead of
            // stopping to ask. Recording it here — at the moment the user
            // actually commits to an answer, not merely while browsing —
            // means the next resolution reads `CurrentSelection`, and losing
            // it becomes `RequiresExplicitChoice`.
            if source != desktop_cloud::ModelSelectionSource::CurrentSelection {
                state.record_catalog_selection(
                    &authority,
                    workspace_id,
                    &desktop_store::StoredModelSelection {
                        profile_id: selection.profile_id().to_owned(),
                        variant_id: selection.variant_id().to_owned(),
                        catalog_version: selection.catalog_version().to_owned(),
                        catalog_hash: selection.catalog_hash().to_owned(),
                        selected_at_ms: accepted_at.0,
                        is_current: true,
                    },
                    accepted_at,
                )?;
            }
            // The dispatch seam. A resolved selection is not yet a composed
            // transport: `SupportApiTransport` needs an HTTPS executor and a
            // bearer the shell has not composed in this build, and inventing
            // either here would be exactly the silent degradation the trust
            // modes exist to prevent.
            Err(LocalError::new(
                LocalErrorCode::FeatureDisabled,
                "Connected model responses are not enabled in this build.",
                false,
            ))
        }
        // Both of the "cannot answer yet" outcomes are data, not errors, so the
        // reason survives the command boundary structurally.
        outcome => selection_data(state, workspace_id, workspace_grant_epoch, outcome),
    }
}

/// Stops one task-bound model response.
///
/// Idempotent by construction: stopping a run that already finished reports
/// `stopped: false` rather than failing, because a user pressing a stop button
/// twice has not made a mistake. Naming both the task and the run is what makes
/// the request checkable — a renderer holding only a run identifier cannot ask
/// to stop a run belonging to a task it does not own.
pub(super) fn stop_model_response(
    state: &HostState,
    workspace_id: &ContractId,
    workspace_grant_epoch: u64,
    task_id: &ContractId,
    run_id: &ContractId,
    stopped_at: UnixMillis,
) -> Result<HostCommandData, LocalError> {
    let authority = state.ready_authority()?;
    let _workspace_authority = state
        .workspace
        .authorize_scope(workspace_id.as_str(), workspace_grant_epoch)
        .map_err(|error| map_workspace_error(&error))?;
    let _ = &authority;
    // No run can be in flight while the dispatch seam above is closed, so this
    // is the truthful answer rather than a stub: there was nothing to stop.
    Ok(HostCommandData::ModelResponseStopped(
        crate::wire::ModelResponseStoppedWire {
            task_id: task_id.clone(),
            run_id: run_id.clone(),
            app_epoch: state.model_auth_epoch(),
            stopped: false,
            stopped_at,
        },
    ))
}

fn map_model_catalog_projection_error(
    error: desktop_ipc::ModelCatalogProjectionError,
) -> LocalError {
    match error {
        desktop_ipc::ModelCatalogProjectionError::Unavailable => LocalError::new(
            LocalErrorCode::TemporarilyUnavailable,
            "The model catalog could not be projected safely.",
            true,
        ),
        desktop_ipc::ModelCatalogProjectionError::UnsafeDisplayText
        | desktop_ipc::ModelCatalogProjectionError::DisclosesConnectionAuthority => {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The model catalog failed integrity validation.",
                false,
            )
        }
    }
}
