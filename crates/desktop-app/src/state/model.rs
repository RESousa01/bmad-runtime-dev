//! Catalog-backed model selection state.
//!
//! Split out of `state.rs` rather than appended to it: which model answers is
//! its own authority, with its own ordering rule and its own durable record,
//! and it does not need to be read alongside workspace catalogs, cursors and
//! effect journals to be understood.
//!
//! This is a child of `state`, so it reaches `HostState`'s private fields
//! directly — the split is an organisational one, not a new trust boundary.
//!
//! Deliberately still in `state.rs`: `set_model_selection`, `sign_in_model` and
//! `sign_out_model`. Those drive the *provider composition* lane and invalidate
//! three coordinators apiece; they are entangled with the offline and
//! direct-provider builds rather than with the catalog, and moving them would
//! be a large diff in a file other lanes are editing.

use desktop_runtime::{ContractId, LocalError, LocalErrorCode, ModelProviderId, UnixMillis};

use super::{conflict_error, recovery_error, HostState, ReadyAuthorityGuard};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModelSelectionState {
    pub provider_id: ModelProviderId,
    pub model_id: String,
}

impl ModelSelectionState {
    /// The provider composition to use when no catalog-backed selection exists.
    ///
    /// This deliberately no longer implements `Default`. A `Default` impl is a
    /// value you fall into without writing anything down, and "which model
    /// answered" is not a question that may ever be settled that way. The
    /// authority for that is [`HostState::resolve_catalog_selection`], which
    /// reads the durable per-workspace selection and resolves it against the
    /// held catalog; this is only the composition the offline and
    /// direct-provider lanes need before any catalog has been installed, and
    /// every call site that uses it is one that has already established there
    /// is no catalog to consult.
    pub(crate) fn provider_fallback() -> Self {
        Self {
            provider_id: ModelProviderId::OpenAi,
            model_id: crate::bmad_model::config::DEFAULT_OPENAI_MODEL.to_owned(),
        }
    }
}

/// The catalog the host currently holds, and when it accepted it.
///
/// A [`desktop_cloud::CachedModelCatalog`] can only be produced by
/// `SupportApiTransport::fetch_model_catalog`, so this is the single place a
/// catalog enters the host and there is no way to synthesize one past it.
#[derive(Clone, Debug)]
pub(crate) struct HeldModelCatalog {
    pub cached: desktop_cloud::CachedModelCatalog,
    pub catalog_epoch: u64,
    pub refreshed_at: UnixMillis,
}

/// The catalog seam.
///
/// Empty until the shell composes a support transport and installs a fetched
/// catalog, exactly like the production model round trip. Every command in the
/// catalog lane reads `None` as "unavailable" and refuses; none of them
/// substitutes a locally invented catalog, because a locally invented catalog
/// is indistinguishable to the renderer from a real one.
#[derive(Clone, Debug, Default)]
pub(crate) struct ModelCatalogState {
    held: Option<HeldModelCatalog>,
}

/// What resolving a workspace's selection against the held catalog produced.
///
/// One variant for one variant of [`desktop_cloud::ModelSelectionOutcome`],
/// plus the case where there is no catalog at all.
#[derive(Clone, Debug)]
pub(crate) enum CatalogSelectionOutcome {
    /// No catalog has been installed, so nothing can be resolved.
    CatalogUnavailable,
    /// A model was resolved.
    Selected {
        selection: desktop_cloud::ResolvedModelSelection,
        source: desktop_cloud::ModelSelectionSource,
        catalog_epoch: u64,
        catalog_version: String,
    },
    /// The user's own selection is gone or blocked. Carried through as itself:
    /// nothing downstream may turn this into a default, into the catalog's
    /// first profile, or into anything from the user's history.
    RequiresExplicitChoice {
        previous: desktop_cloud::PersistedModelSelection,
        blocked: desktop_cloud::ModelSelectionBlocked,
        catalog_epoch: u64,
        catalog_version: String,
    },
    /// The catalog offers nothing selectable.
    NoneSelectable {
        catalog_epoch: u64,
        catalog_version: String,
    },
}

impl HostState {
    /// Installs one verified catalog. The single ingestion point.
    ///
    /// Takes an already-fetched [`desktop_cloud::CachedModelCatalog`], which
    /// only `SupportApiTransport::fetch_model_catalog` can produce, so there is
    /// no path into this state that skipped the transport's verification.
    ///
    /// # Errors
    ///
    /// Returns an invalid-request error when the catalog version is not shaped
    /// like a catalog version, because the epoch is what orders two catalogs
    /// and an unorderable catalog cannot be reasoned about.
    #[allow(
        dead_code,
        reason = "the shell composes the catalog transport at desktop enablement; \
                  this is the seam it installs through, and its ordering rule is \
                  the reason it exists before that"
    )]
    pub(crate) fn install_model_catalog(
        &self,
        cached: desktop_cloud::CachedModelCatalog,
        refreshed_at: UnixMillis,
    ) -> Result<u64, LocalError> {
        let catalog_epoch = catalog_epoch(cached.catalog().catalog_version()).ok_or_else(|| {
            LocalError::new(
                LocalErrorCode::IntegrityFailure,
                "The model catalog could not be ordered against the one already held.",
                false,
            )
        })?;
        let mut state = self.model_catalog.write();
        // A catalog never moves backwards. The support plane can legitimately
        // reissue the same epoch, but an older one arriving after a newer one
        // means a stale response won a race, and honoring it would silently
        // reintroduce profiles the newer catalog withdrew.
        if state
            .held
            .as_ref()
            .is_some_and(|held| held.catalog_epoch > catalog_epoch)
        {
            return Err(conflict_error(
                "A newer model catalog is already in use; nothing was changed.",
            ));
        }
        state.held = Some(HeldModelCatalog {
            cached,
            catalog_epoch,
            refreshed_at,
        });
        Ok(catalog_epoch)
    }

    /// The catalog currently held, if the shell installed one.
    #[must_use]
    pub(crate) fn held_model_catalog(&self) -> Option<HeldModelCatalog> {
        self.model_catalog.read().held.clone()
    }

    /// Drops the held catalog so the next read must re-establish one.
    pub(crate) fn invalidate_model_catalog(&self) {
        self.model_catalog.write().held = None;
    }

    /// Resolves which model one workspace should use.
    ///
    /// Reads the durable per-workspace selection and history, then defers
    /// entirely to [`desktop_cloud::resolve_model_selection`] for the decision.
    /// The `RequiresExplicitChoice` outcome is returned as itself and is never
    /// collapsed into a fallback here or anywhere above here.
    ///
    /// # Errors
    ///
    /// Returns recovery-required when the authority store cannot be read.
    pub(crate) fn resolve_catalog_selection(
        &self,
        authority: &ReadyAuthorityGuard<'_>,
        workspace_id: &ContractId,
    ) -> Result<CatalogSelectionOutcome, LocalError> {
        let Some(held) = self.held_model_catalog() else {
            return Ok(CatalogSelectionOutcome::CatalogUnavailable);
        };
        let store = self.local_store(authority)?;
        let recent = store
            .recent_model_selections(workspace_id.as_str())
            .map_err(|_| recovery_error())?;
        let current = recent
            .iter()
            .find(|selection| selection.is_current)
            .map(persisted_selection);
        let history: Vec<desktop_cloud::PersistedModelSelection> = recent
            .iter()
            .filter(|selection| !selection.is_current)
            .map(persisted_selection)
            .collect();
        let catalog = held.cached.catalog();
        let catalog_version = catalog.catalog_version().to_owned();
        let outcome = desktop_cloud::resolve_model_selection(catalog, current.as_ref(), &history);
        Ok(match outcome {
            desktop_cloud::ModelSelectionOutcome::Selected { selection, source } => {
                CatalogSelectionOutcome::Selected {
                    selection,
                    source,
                    catalog_epoch: held.catalog_epoch,
                    catalog_version,
                }
            }
            desktop_cloud::ModelSelectionOutcome::RequiresExplicitChoice { previous, blocked } => {
                CatalogSelectionOutcome::RequiresExplicitChoice {
                    previous,
                    blocked,
                    catalog_epoch: held.catalog_epoch,
                    catalog_version,
                }
            }
            desktop_cloud::ModelSelectionOutcome::NoneSelectable => {
                CatalogSelectionOutcome::NoneSelectable {
                    catalog_epoch: held.catalog_epoch,
                    catalog_version,
                }
            }
        })
    }

    /// Records one workspace's catalog-backed selection durably.
    ///
    /// # Errors
    ///
    /// Returns recovery-required when the authority store refuses the write.
    pub(crate) fn record_catalog_selection(
        &self,
        authority: &ReadyAuthorityGuard<'_>,
        workspace_id: &ContractId,
        selection: &desktop_store::StoredModelSelection,
        recorded_at: UnixMillis,
    ) -> Result<(), LocalError> {
        self.local_store(authority)?
            .record_model_selection(workspace_id.as_str(), selection, recorded_at.0)
            .map_err(|_| recovery_error())
    }
}

/// The monotonic epoch of a catalog version (`v<epoch>.<digest>`).
///
/// The store parses this shape too, but keeps its parser private and treats a
/// failure as a rejected write. The host needs the same fact to order two
/// catalogs, so the shape is re-stated here rather than the store's write path
/// being widened into a general-purpose parser.
#[allow(
    dead_code,
    reason = "used only by `install_model_catalog`, which the shell composes"
)]
fn catalog_epoch(catalog_version: &str) -> Option<u64> {
    let rest = catalog_version.strip_prefix('v')?;
    let (epoch, digest) = rest.split_once('.')?;
    if epoch.is_empty()
        || epoch.starts_with('0')
        || !epoch.bytes().all(|byte| byte.is_ascii_digit())
        || digest.len() != 16
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    epoch.parse().ok()
}

fn persisted_selection(
    stored: &desktop_store::StoredModelSelection,
) -> desktop_cloud::PersistedModelSelection {
    desktop_cloud::PersistedModelSelection {
        profile_id: stored.profile_id.clone(),
        variant_id: stored.variant_id.clone(),
        catalog_version: stored.catalog_version.clone(),
        catalog_hash: stored.catalog_hash.clone(),
        selected_at: UnixMillis(stored.selected_at_ms),
    }
}
