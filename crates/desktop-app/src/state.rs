use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine as _;
use desktop_ipc::{deserialize_strict, AdmissionPolicy, RequestGate};
use desktop_runtime::{
    sha256_bytes, ContractId, DesktopLocalIdentity, LocalError, LocalErrorCode, MethodSession,
    ModelProviderCredential, ModelProviderId, ProjectionEvent, ProjectionEventKind,
    ProjectionSnapshot, UnixMillis,
};
use desktop_store::{
    BmadHelpRunCreateRequest, BmadHelpRunCreationReceipt, BmadHelpRunLatest,
    BmadHelpRunReplayRequest, EvidenceAppend, LocalStore, PayloadRef, StoreError,
    UserDpapiProtector,
};
use desktop_workspace::{WorkspaceBroker, WorkspaceProjection};
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::bmad_model::coordinator::BmadHelpCoordinator;
use crate::wire::{BootMode, HostDispatchReply};

/// Catalog-backed model selection. See [`model`] for why it is its own file.
mod model;

pub(crate) use model::{
    CatalogSelectionOutcome, HeldModelCatalog, ModelCatalogState, ModelSelectionState,
};

const WORKSPACE_CATALOG_SCHEMA: &str = "workspace-catalog.v1";
const WORKSPACE_ROOT_SCHEMA: &str = "workspace-root.v1";
const MAX_ROOT_BYTES: usize = 64 * 1024;
const MAX_EVENTS: usize = 1_024;
const MAX_CURSORS: usize = 4_096;
const MAX_CACHED_REPLIES: usize = 2_048;
const MAX_RENDERER_SAFE_MODEL_AUTH_EPOCH: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceCatalog {
    schema_version: String,
    entries: Vec<PersistedWorkspace>,
}

impl Default for WorkspaceCatalog {
    fn default() -> Self {
        Self {
            schema_version: WORKSPACE_CATALOG_SCHEMA.to_owned(),
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedWorkspace {
    projection: WorkspaceProjection,
    root_payload: PayloadRef,
    revoked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceRootSecret {
    schema_version: String,
    root_utf16_le: String,
    root_identity_hash: String,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryCursor {
    pub renderer_session_id: ContractId,
    pub workspace_id: ContractId,
    pub grant_epoch: u64,
    pub relative_directory: String,
    pub after: Option<String>,
}

#[derive(Debug)]
struct CatalogState {
    version: u64,
    value: WorkspaceCatalog,
}

#[derive(Debug, Default)]
struct CursorState {
    values: HashMap<String, DirectoryCursor>,
    order: VecDeque<String>,
}

#[derive(Debug, Default)]
struct ReplyCache {
    values: HashMap<ContractId, HostDispatchReply>,
    order: VecDeque<ContractId>,
}

pub(crate) struct HostState {
    pub workspace: WorkspaceBroker,
    /// Session-scoped "allow always" answers for agent tools (ADR-0009).
    /// Cleared on sign-out and workspace revocation, so authority never
    /// outlives the context it was granted in.
    pub(crate) tool_grants: Mutex<desktop_tools::GrantedRules>,
    pub gate: RequestGate,
    /// The renderer dispatch lane. See [`HostState::dispatch_lane`].
    dispatch_lane: Mutex<()>,
    workspace_commits: Mutex<()>,
    store: Option<LocalStore>,
    installation_id: ContractId,
    boot_mode: RwLock<BootMode>,
    catalog: Mutex<CatalogState>,
    renderer_sessions: RwLock<HashMap<String, ContractId>>,
    cursors: Mutex<CursorState>,
    replies: Mutex<ReplyCache>,
    pending_proposals: Mutex<crate::edits::PendingProposals>,
    pending_recoveries: Mutex<crate::recovery::PendingRecoveries>,
    sequence: AtomicU64,
    model_auth_epoch: AtomicU64,
    model_selection: RwLock<ModelSelectionState>,
    /// The catalog-backed selection authority. See [`ModelCatalogState`].
    model_catalog: RwLock<ModelCatalogState>,
    events: Mutex<VecDeque<ProjectionEvent>>,
    pub(crate) bmad_model: Mutex<BmadHelpCoordinator>,
    pub(crate) bmad_capabilities:
        Mutex<crate::bmad_model::capability_coordinator::BmadCapabilityCoordinator>,
    /// The ordered architecture progression that rides the generic capability
    /// lane. Locked *after* `bmad_capabilities` is released, never across it.
    pub(crate) bmad_architecture: Mutex<crate::bmad_architecture::BmadArchitectureCoordinator>,
    /// The ordinary chat lane. Independent of every BMAD coordinator: a
    /// conversation needs no BMAD concept, and holds no other lock.
    pub(crate) chat: Mutex<crate::chat::ChatCoordinator>,
}

/// Proof that the local authority remained in Ready mode while a D1 operation
/// was performed. Holding this guard prevents a concurrent recovery transition
/// from crossing the operation's stateful boundary.
pub(crate) struct ReadyAuthorityGuard<'a> {
    _mode: RwLockReadGuard<'a, BootMode>,
}

/// Serializes workspace-bound durable commits with revocation. The constructor
/// always acquires this host barrier before checking Ready mode, then guarded
/// callers may acquire a [`desktop_workspace::WorkspaceScopeAuthorityGuard`]
/// and finally the store transaction. This lock order must not be inverted.
pub(crate) struct ReadyWorkspaceCommitGuard<'a> {
    state: &'a HostState,
    commit: MutexGuard<'a, ()>,
    ready: ReadyAuthorityGuard<'a>,
}

impl ReadyWorkspaceCommitGuard<'_> {
    pub fn authority(&self) -> &ReadyAuthorityGuard<'_> {
        &self.ready
    }

    /// Returns the durable catalog version represented by this process's
    /// authorized workspace broker while the commit barrier remains held.
    pub fn workspace_catalog_version(&self) -> u64 {
        self.state.catalog.lock().version
    }

    /// Enters recovery while still excluding another workspace-bound commit.
    /// The Ready read guard is released before the mode write is requested.
    pub fn enter_recovery(self) -> u64 {
        let Self {
            state,
            commit,
            ready,
        } = self;
        drop(ready);
        let sequence = state.enter_recovery();
        drop(commit);
        sequence
    }
}

/// Pins the renderer session mapped to a window until request processing is
/// complete. A new bootstrap must acquire the corresponding write lock and
/// therefore cannot revoke this session midway through a native operation.
///
/// This pin must not span a provider round trip or a native dialog.
/// `bind_renderer` takes the write half of the same lock, so a pin held across
/// one parks every rebind for its whole duration — and because a parked writer
/// makes `parking_lot` queue every later *reader* behind it, it parks the
/// projection lane too. `host_dispatch` therefore takes the pin only long
/// enough to read the bound identity, and each command that genuinely needs the
/// pin re-takes it, checked against the identity the envelope was validated
/// under (see `HostState::pin_renderer_session`).
///
/// **One command still breaks this rule**, and the rule is stated absolutely
/// above only because that exception is meant to be closed, not tolerated:
/// `bmad.help.submit` holds the pin across `dispatch.send()`, because
/// `SubmitBmadHelpReviewInput` and `FinalizeBmadHelpDispatchInput` take a
/// `&RendererSessionGuard`. A Help submit is therefore still able to park a
/// renderer rebind, and every projection read behind it, for the length of one
/// provider call. It is bounded to a single call rather than a sixteen-call
/// turn, and it no longer runs on the UI thread, so the window keeps painting —
/// but the underlying hazard is the same one this comment describes.
///
/// Note also that `a_blocked_dispatch_does_not_park_the_window_facing_commands`
/// below models `host_dispatch`'s shape by hand rather than driving the real
/// command, so it cannot fail on that path. Closing the exception means
/// threading `ContractId` instead of the guard; `finalize_submit` only ever
/// calls `.session_id()`.
pub(crate) struct RendererSessionGuard<'a> {
    _sessions: RwLockReadGuard<'a, HashMap<String, ContractId>>,
    session_id: ContractId,
}

impl RendererSessionGuard<'_> {
    pub fn session_id(&self) -> &ContractId {
        &self.session_id
    }
}

/// Exclusive occupancy of the renderer dispatch lane.
///
/// Until `host_dispatch` was moved off the main thread this exclusion existed
/// by accident: a synchronous `#[tauri::command]` runs on the thread that pumps
/// the `WebView2` message loop, so exactly one dispatch could be in flight and
/// nothing else could record a projection event while one was. That accident is
/// what made three separate host invariants true, and removing it without
/// replacing it would have turned a reported freeze into an unreported race:
///
/// * **One event writer.** Every `record_event` call site is reached from a
///   dispatch. With one dispatch at a time, the projection sequence a reply
///   carries covers that dispatch's own events and nothing else. Concurrent
///   dispatches would let a reply be stamped with a sequence that includes an
///   event some *other* dispatch recorded, which the renderer then treats as
///   delivered — `HostClient.projectionEvents` asks only for events after its
///   cursor, so the skipped event is never sent again.
/// * **Monotone reply sequences.** `HostClient.advanceProjectionSequence`
///   refuses a sequence below the one it already holds, and refusing is a hard
///   protocol error, not a retry. Two dispatch replies that race can arrive in
///   the opposite order to the one they sampled in.
/// * **Terminating seqlocks.** `host_bootstrap` and [`HostState::snapshot`]
///   both re-read until the sequence is unchanged across the read. They cannot
///   livelock while at most one bounded writer exists.
///
/// The lane is the outermost host lock: it is taken first and nothing else ever
/// waits on it, so it introduces no ordering edge and cannot participate in a
/// cycle. It does *not* keep the UI thread busy — that is the whole point of
/// the move — so a queued dispatch waits without the window freezing.
pub(crate) struct DispatchLaneGuard<'a> {
    state: &'a HostState,
    _lane: MutexGuard<'a, ()>,
}

impl DispatchLaneGuard<'_> {
    /// The projection sequence this dispatch's reply may publish.
    ///
    /// Sampled through the lane guard so the value is, by construction, a point
    /// the event log reached under this dispatch's own exclusion.
    pub fn sequence(&self) -> u64 {
        self.state.sequence()
    }
}

impl HostState {
    pub fn initialize(root: Option<PathBuf>) -> Result<Self, LocalError> {
        let recovery_id = new_contract_id("installation_recovery")?;
        let Some(root) = root else {
            return Ok(Self::recovery(recovery_id, None));
        };

        let Ok(store) = LocalStore::open(root, &UserDpapiProtector) else {
            return Ok(Self::recovery(recovery_id, None));
        };
        let installation_id = match store.local_identity() {
            Ok(identity) => identity.installation_id().clone(),
            Err(_) => return Ok(Self::recovery(recovery_id, Some(store))),
        };

        if store.verify_integrity().is_err() {
            return Ok(Self::recovery(installation_id, Some(store)));
        }

        // Interrupted effect journals are reconciled from the durable state
        // machine before any command can start a new governed effect.
        if crate::edits::reconcile_execution_journals(&store).is_err() {
            return Ok(Self::recovery(installation_id, Some(store)));
        }

        let Some((workspace, catalog)) = load_workspace_catalog(&store) else {
            return Ok(Self::recovery(installation_id, Some(store)));
        };

        let model_selection = ModelSelectionState::provider_fallback();
        #[cfg(feature = "deterministic-help")]
        let bmad_model = BmadHelpCoordinator::new();
        #[cfg(not(feature = "deterministic-help"))]
        let bmad_model = {
            let mut coordinator = BmadHelpCoordinator::new();
            let composition = crate::bmad_model::config::frontend_model_composition(
                model_selection.provider_id,
                &model_selection.model_id,
            )
            .map_err(|_| recovery_error())?;
            coordinator
                .configure_frontend_selection(composition, now())
                .map_err(|_| recovery_error())?;
            coordinator
        };

        Ok(Self {
            workspace,
            tool_grants: Mutex::new(desktop_tools::GrantedRules::new()),
            gate: RequestGate::new(AdmissionPolicy::default()),
            dispatch_lane: Mutex::new(()),
            workspace_commits: Mutex::new(()),
            store: Some(store),
            installation_id,
            boot_mode: RwLock::new(BootMode::Ready),
            catalog: Mutex::new(catalog),
            renderer_sessions: RwLock::new(HashMap::new()),
            cursors: Mutex::new(CursorState::default()),
            replies: Mutex::new(ReplyCache::default()),
            pending_proposals: Mutex::new(crate::edits::PendingProposals::default()),
            pending_recoveries: Mutex::new(crate::recovery::PendingRecoveries::default()),
            sequence: AtomicU64::new(0),
            model_auth_epoch: AtomicU64::new(1),
            model_selection: RwLock::new(model_selection),
            model_catalog: RwLock::new(ModelCatalogState::default()),
            events: Mutex::new(VecDeque::new()),
            bmad_model: Mutex::new(bmad_model),
            bmad_capabilities: Mutex::new(
                crate::bmad_model::capability_coordinator::BmadCapabilityCoordinator::new(),
            ),
            bmad_architecture: Mutex::new(
                crate::bmad_architecture::BmadArchitectureCoordinator::new(),
            ),
            chat: Mutex::new(crate::chat::ChatCoordinator::new()),
        })
    }

    fn recovery(installation_id: ContractId, store: Option<LocalStore>) -> Self {
        Self {
            workspace: WorkspaceBroker::new(),
            tool_grants: Mutex::new(desktop_tools::GrantedRules::new()),
            gate: RequestGate::new(AdmissionPolicy::default()),
            dispatch_lane: Mutex::new(()),
            workspace_commits: Mutex::new(()),
            store,
            installation_id,
            boot_mode: RwLock::new(BootMode::ReadOnlyRecovery),
            catalog: Mutex::new(CatalogState {
                version: 0,
                value: WorkspaceCatalog::default(),
            }),
            renderer_sessions: RwLock::new(HashMap::new()),
            cursors: Mutex::new(CursorState::default()),
            replies: Mutex::new(ReplyCache::default()),
            pending_proposals: Mutex::new(crate::edits::PendingProposals::default()),
            pending_recoveries: Mutex::new(crate::recovery::PendingRecoveries::default()),
            sequence: AtomicU64::new(0),
            model_auth_epoch: AtomicU64::new(1),
            model_selection: RwLock::new(ModelSelectionState::provider_fallback()),
            model_catalog: RwLock::new(ModelCatalogState::default()),
            events: Mutex::new(VecDeque::new()),
            bmad_model: Mutex::new(BmadHelpCoordinator::new()),
            bmad_capabilities: Mutex::new(
                crate::bmad_model::capability_coordinator::BmadCapabilityCoordinator::new(),
            ),
            bmad_architecture: Mutex::new(
                crate::bmad_architecture::BmadArchitectureCoordinator::new(),
            ),
            chat: Mutex::new(crate::chat::ChatCoordinator::new()),
        }
    }

    pub fn installation_id(&self) -> &ContractId {
        &self.installation_id
    }

    /// Returns the renderer-visible model identity epoch. It is process-local
    /// in D2-D because no production identity session is composed yet.
    pub fn model_auth_epoch(&self) -> u64 {
        self.model_auth_epoch.load(Ordering::SeqCst)
    }

    #[must_use]
    pub(crate) fn model_selection(&self) -> ModelSelectionState {
        self.model_selection.read().clone()
    }

    pub(crate) fn model_configuration(
        &self,
    ) -> Result<crate::bmad_model::config::HelpModelConfiguration, LocalError> {
        self.bmad_model
            .lock()
            .model_configuration()
            .map_err(|_| recovery_error())
    }

    #[cfg(feature = "direct-model-providers")]
    #[must_use]
    pub(crate) fn direct_provider_config(&self) -> Option<desktop_cloud::DirectProviderConfig> {
        self.bmad_model.lock().direct_provider_config()
    }

    pub(crate) fn set_model_selection(
        &self,
        provider_id: ModelProviderId,
        model_id: String,
        changed_at: UnixMillis,
    ) -> Result<u64, LocalError> {
        #[cfg(feature = "direct-model-providers")]
        let connected = self.bmad_model.lock().direct_provider_config();
        #[cfg(feature = "direct-model-providers")]
        let composition = if let Some(config) = connected.filter(|config| {
            matches!(
                (provider_id, config.kind()),
                (
                    ModelProviderId::OpenAi,
                    desktop_cloud::DirectProviderKind::OpenAi
                ) | (
                    ModelProviderId::Azure,
                    desktop_cloud::DirectProviderKind::AzureOpenAi
                )
            )
        }) {
            config
                .with_model(&model_id)
                .map_err(|_| invalid_model_selection())
                .and_then(|config| {
                    crate::bmad_model::config::direct_frontend_model_composition(config)
                        .map_err(|_| invalid_model_selection())
                })?
        } else {
            crate::bmad_model::config::frontend_model_composition(provider_id, &model_id)
                .map_err(|_| invalid_model_selection())?
        };
        #[cfg(not(feature = "direct-model-providers"))]
        let composition =
            crate::bmad_model::config::frontend_model_composition(provider_id, &model_id)
                .map_err(|_| invalid_model_selection())?;
        let mut bmad_model = self.bmad_model.lock();
        bmad_model
            .configure_frontend_selection(composition, changed_at)
            .map_err(|_| {
                LocalError::new(
                    LocalErrorCode::TemporarilyUnavailable,
                    "The selected model provider could not be initialized.",
                    true,
                )
            })?;
        self.bmad_capabilities.lock().invalidate();
        self.bmad_architecture.lock().invalidate();
        self.chat.lock().invalidate();
        *self.model_selection.write() = ModelSelectionState {
            provider_id,
            model_id,
        };
        self.advance_model_auth_epoch()
    }

    pub(crate) fn sign_in_model(
        &self,
        provider_id: ModelProviderId,
        model_id: String,
        credential: &ModelProviderCredential,
        endpoint: Option<&str>,
        region: Option<&str>,
        changed_at: UnixMillis,
    ) -> Result<u64, LocalError> {
        #[cfg(feature = "direct-model-providers")]
        let composition = crate::bmad_model::config::frontend_model_composition_with_credential(
            provider_id,
            &model_id,
            credential.expose().to_owned(),
            endpoint,
            region,
        )
        .map_err(|_| {
            LocalError::new(
                LocalErrorCode::InvalidRequest,
                "The provider connection details are invalid.",
                true,
            )
        })?;
        // Without the direct provider compiled in there is nothing to sign in
        // to, so this is the whole body: the block is the tail expression, and
        // `self` is discarded with the arguments to keep clippy honest about a
        // method that genuinely uses neither.
        #[cfg(not(feature = "direct-model-providers"))]
        {
            let _ = (
                self,
                provider_id,
                model_id,
                credential,
                endpoint,
                region,
                changed_at,
            );
            Err(LocalError::new(
                LocalErrorCode::FeatureDisabled,
                "Direct model connections are not enabled in this build.",
                false,
            ))
        }

        #[cfg(feature = "direct-model-providers")]
        {
            self.bmad_model
                .lock()
                .configure_frontend_selection(composition, changed_at)
                .map_err(|_| {
                    LocalError::new(
                        LocalErrorCode::TemporarilyUnavailable,
                        "The provider connection could not be initialized.",
                        true,
                    )
                })?;
            self.bmad_capabilities.lock().invalidate();
            self.bmad_architecture.lock().invalidate();
            self.chat.lock().invalidate();
            *self.model_selection.write() = ModelSelectionState {
                provider_id,
                model_id,
            };
            self.advance_model_auth_epoch()
        }
    }

    fn advance_model_auth_epoch(&self) -> Result<u64, LocalError> {
        let current = self.model_auth_epoch.load(Ordering::SeqCst);
        if current >= MAX_RENDERER_SAFE_MODEL_AUTH_EPOCH {
            return Err(recovery_error());
        }
        let next = current.checked_add(1).ok_or_else(recovery_error)?;
        self.model_auth_epoch.store(next, Ordering::SeqCst);
        self.workspace.advance_all_context_read_epochs();
        Ok(next)
    }

    /// Invalidates every pending Help decision and advances the model identity
    /// epoch without contacting an identity broker.
    ///
    /// # Errors
    ///
    /// Returns recovery-required if the bounded epoch is exhausted.
    pub fn sign_out_model(&self) -> Result<u64, LocalError> {
        let selection = self.model_selection();
        let composition = crate::bmad_model::config::offline_frontend_model_composition(
            selection.provider_id,
            &selection.model_id,
        )
        .map_err(|_| recovery_error())?;
        self.bmad_model
            .lock()
            .configure_frontend_selection(composition, now())
            .map_err(|_| recovery_error())?;
        self.bmad_capabilities.lock().invalidate();
        self.bmad_architecture.lock().invalidate();
        self.chat.lock().invalidate();
        // ADR-0009: an "allow always" answer was given to a model that is now
        // signed out. The next model must ask for itself.
        self.tool_grants.lock().clear();
        // ADR-0002: sign-out withdraws D2 context-read authority everywhere
        // without touching D3 proposals or local work.
        self.advance_model_auth_epoch()
    }

    /// Executes the ADR-0004 offboarding erase: signs out model authority
    /// (withdrawing every D2 context-read epoch), revokes all workspace
    /// grants, cryptographically erases the local authority store, and drops
    /// the session to read-only recovery. Irreversible by design.
    ///
    /// # Errors
    ///
    /// Fails closed before any deletion when Ready authority is not held or
    /// the store is absent; a store failure after key destruction still
    /// leaves only undecryptable ciphertext behind.
    pub fn offboard_erase(&self) -> Result<(), LocalError> {
        let _commit = self.workspace_commits.lock();
        {
            let _authority = self.ready_authority()?;
            // Epoch exhaustion cannot block erasure: the identity being
            // signed out is about to be destroyed anyway.
            let _ = self.sign_out_model();
            for workspace in self.workspace.list() {
                let _ = self.workspace.revoke(&workspace.workspace_id);
            }
            let store = self.store.as_ref().ok_or_else(recovery_error)?;
            // The message this replaced said: "the store key was destroyed, so
            // remaining bytes are undecryptable. Restart and retry." Every
            // clause of that was capable of being false, and the last one was
            // actively harmful.
            //
            //   - "the store key was destroyed" — deleting the key file is
            //     itself fallible. If *that* is the step that failed, the
            //     wrapped key is still on disk and nothing was destroyed.
            //   - "remaining bytes are undecryptable" — only the
            //     content-addressed payloads are encrypted. There is no
            //     SQLCipher in this crate, so `authority.sqlite3` is plaintext,
            //     and it holds the evidence log, aggregate state and memlog
            //     text (stored inline by design). A surviving database, or even
            //     a surviving `-wal`, is readable.
            //   - "Restart and retry" — restarting is the one action that makes
            //     this unrecoverable. A partial erase leaves no key file beside
            //     a live database, which is exactly the state `LocalStore::open`
            //     refuses, so the next launch lands in recovery with no store
            //     at all and `app.offboarding.erase` is not in the recovery
            //     command set. Retrying *in this session* does work:
            //     `erase_for_offboarding` is idempotent and this lane is still
            //     Ready, because the `?` below returns before `enter_recovery`.
            //
            // So the instruction is inverted, and `retryable` is now true.
            store.erase_for_offboarding().map_err(|_| {
                LocalError::new(
                    LocalErrorCode::IntegrityFailure,
                    "Erase did not finish, and some local data may remain readable. \
                     Run Erase again now, in this window — do not restart, because after a \
                     restart this app can no longer complete the erase.",
                    true,
                )
            })?;
        }
        // The Ready read guard is released above; recovery takes the write
        // half of the same lock.
        self.enter_recovery();
        Ok(())
    }

    pub fn boot_mode(&self) -> BootMode {
        *self.boot_mode.read()
    }

    pub fn ready_authority(&self) -> Result<ReadyAuthorityGuard<'_>, LocalError> {
        let mode = self.boot_mode.read();
        if *mode != BootMode::Ready {
            return Err(recovery_error());
        }
        Ok(ReadyAuthorityGuard { _mode: mode })
    }

    pub fn ready_workspace_commit(&self) -> Result<ReadyWorkspaceCommitGuard<'_>, LocalError> {
        let commit = self.workspace_commits.lock();
        let ready = self.ready_authority()?;
        Ok(ReadyWorkspaceCommitGuard {
            state: self,
            commit,
            ready,
        })
    }

    pub fn local_identity(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
    ) -> Result<DesktopLocalIdentity, StoreError> {
        self.store
            .as_ref()
            .ok_or(StoreError::Inconsistent)?
            .local_identity()
    }

    /// Returns the open authority store while Ready mode is proven held.
    pub fn local_store(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
    ) -> Result<&LocalStore, LocalError> {
        self.store.as_ref().ok_or_else(recovery_error)
    }

    pub fn insert_pending_proposal(
        &self,
        approval_id: String,
        proposal: crate::edits::PendingChangesProposal,
    ) {
        self.pending_proposals.lock().insert(approval_id, proposal);
    }

    pub fn take_pending_proposal(
        &self,
        approval_id: &str,
    ) -> Option<crate::edits::PendingChangesProposal> {
        self.pending_proposals.lock().take(approval_id)
    }

    #[allow(
        dead_code,
        reason = "the recovery authority methods are consumed by the Task 4 command boundary"
    )]
    pub(crate) fn insert_pending_recovery(&self, pending: crate::recovery::PendingRecovery) {
        self.pending_recoveries.lock().insert(pending);
    }

    #[allow(
        dead_code,
        reason = "the recovery authority methods are consumed by the Task 4 command boundary"
    )]
    pub(crate) fn take_pending_recovery(
        &self,
        approval_id: &ContractId,
    ) -> Option<crate::recovery::PendingRecovery> {
        self.pending_recoveries.lock().take(approval_id)
    }

    pub(crate) fn invalidate_pending_recoveries(&self) {
        self.pending_recoveries.lock().invalidate_all();
    }

    /// Persists an updated projection for an already-registered workspace,
    /// for example after governed edits are enabled at a new grant epoch.
    pub fn persist_workspace_update(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        projection: &WorkspaceProjection,
        event_type: &str,
        correlation_id: &ContractId,
    ) -> Result<(), LocalError> {
        let store = self.store.as_ref().ok_or_else(recovery_error)?;
        let mut catalog = self.catalog.lock();
        let mut next = catalog.value.clone();
        let entry = next
            .entries
            .iter_mut()
            .find(|entry| {
                entry.projection.workspace_id == projection.workspace_id && !entry.revoked
            })
            .ok_or_else(|| not_found_error("The local workspace is not available."))?;
        entry.projection = projection.clone();
        persist_catalog(store, &mut catalog, next, event_type, correlation_id)?;
        self.invalidate_pending_recoveries();
        Ok(())
    }

    pub fn replay_bmad_help_run(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        request: &BmadHelpRunReplayRequest,
    ) -> Result<Option<BmadHelpRunCreationReceipt>, StoreError> {
        self.store
            .as_ref()
            .ok_or(StoreError::Inconsistent)?
            .replay_bmad_help_run(request)
    }

    pub fn create_bmad_help_run(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        candidate: &MethodSession,
        request: &BmadHelpRunCreateRequest,
    ) -> Result<BmadHelpRunCreationReceipt, StoreError> {
        self.store
            .as_ref()
            .ok_or(StoreError::Inconsistent)?
            .create_bmad_help_run(candidate, request)
    }

    pub fn latest_bmad_help_run(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        workspace_id: &ContractId,
        expected_workspace_catalog_version: u64,
    ) -> Result<BmadHelpRunLatest, StoreError> {
        self.store
            .as_ref()
            .ok_or(StoreError::Inconsistent)?
            .latest_bmad_help_run(workspace_id, expected_workspace_catalog_version)
    }

    pub fn method_store<'a>(
        &'a self,
        _authority: &ReadyAuthorityGuard<'_>,
    ) -> Result<&'a LocalStore, StoreError> {
        self.store.as_ref().ok_or(StoreError::Inconsistent)
    }

    /// Enters recovery once and publishes the corresponding projection before
    /// releasing the authority write lock. Callers therefore cannot observe a
    /// recovery mode whose transition event has not yet been sequenced.
    pub fn enter_recovery(&self) -> u64 {
        let mut mode = self.boot_mode.write();
        if *mode == BootMode::ReadOnlyRecovery {
            self.invalidate_pending_recoveries();
            return self.sequence();
        }
        // Acquire the Ready write authority before the Help coordinator. A
        // workspace-bound transition holds Ready before `bmad_model`; taking
        // this order prevents recovery from forming the inverse wait cycle.
        let mut bmad_model = self.bmad_model.lock();
        bmad_model.invalidate(now());
        self.bmad_capabilities.lock().invalidate();
        self.bmad_architecture.lock().invalidate();
        self.chat.lock().invalidate();
        // ADR-0009: recovery is a read-only session with no agent tool lane, so
        // no session grant may survive into whatever authority is restored next.
        // Taken and released by its own statement; `tool_grants` is a leaf, held
        // only across `WorkspaceBroker` calls, which reach neither `boot_mode`
        // nor any coordinator, so this adds no ordering edge.
        self.tool_grants.lock().clear();
        self.invalidate_pending_recoveries();
        *mode = BootMode::ReadOnlyRecovery;
        self.record_event(ProjectionEventKind::BootStateChanged {
            mode: BootMode::ReadOnlyRecovery.as_str().to_owned(),
        })
    }

    pub fn bind_renderer(&self, window_label: &str) -> Result<ContractId, LocalError> {
        let renderer_session_id = new_contract_id("renderer")?;
        let mut renderer_sessions = self.renderer_sessions.write();
        let mut bmad_model = self.bmad_model.lock();
        renderer_sessions.insert(window_label.to_owned(), renderer_session_id.clone());
        bmad_model.invalidate(now());
        self.invalidate_pending_recoveries();
        Ok(renderer_session_id)
    }

    pub fn renderer_session_authority(
        &self,
        window_label: &str,
    ) -> Option<RendererSessionGuard<'_>> {
        let sessions = self.renderer_sessions.read();
        let session_id = sessions.get(window_label)?.clone();
        Some(RendererSessionGuard {
            _sessions: sessions,
            session_id,
        })
    }

    /// The renderer session bound to a window, copied instead of pinned.
    ///
    /// `host_dispatch` needs the identity to validate the envelope against; it
    /// does not need to stop a rebind for the length of a provider round trip,
    /// which is what pinning it would do. Every later use re-takes the pin
    /// through [`Self::pin_renderer_session`], which refuses if the binding
    /// moved in between.
    pub fn renderer_session_id(&self, window_label: &str) -> Option<ContractId> {
        self.renderer_sessions.read().get(window_label).cloned()
    }

    /// Re-takes the renderer pin, but only while the window is still bound to
    /// the session the caller was authorized under.
    ///
    /// Holding one pin for a whole dispatch used to make this check vacuous:
    /// nothing could rebind, so nothing could be observed to have rebound.
    /// Releasing the pin between validation and execution makes a mid-dispatch
    /// rebind reachable, and this is where it is caught — the command fails
    /// closed rather than completing against a session the renderer replaced.
    pub fn pin_renderer_session(
        &self,
        window_label: &str,
        expected: &ContractId,
    ) -> Option<RendererSessionGuard<'_>> {
        let authority = self.renderer_session_authority(window_label)?;
        (authority.session_id() == expected).then_some(authority)
    }

    /// Enters the serialised renderer dispatch lane. See [`DispatchLaneGuard`].
    pub fn dispatch_lane(&self) -> DispatchLaneGuard<'_> {
        DispatchLaneGuard {
            _lane: self.dispatch_lane.lock(),
            state: self,
        }
    }

    pub fn persist_workspace(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        projection: WorkspaceProjection,
        selected_root: &Path,
        root_identity_hash: &str,
        correlation_id: &ContractId,
    ) -> Result<(), LocalError> {
        let store = self.store.as_ref().ok_or_else(recovery_error)?;
        let secret = WorkspaceRootSecret {
            schema_version: WORKSPACE_ROOT_SCHEMA.to_owned(),
            root_utf16_le: encode_root(selected_root)?,
            root_identity_hash: root_identity_hash.to_owned(),
        };
        let secret_bytes = serde_json::to_vec(&secret).map_err(|_| recovery_error())?;
        let root_payload = store
            .put_payload("workspace_root", WORKSPACE_ROOT_SCHEMA, &secret_bytes)
            .map_err(|_| recovery_error())?;

        let mut catalog = self.catalog.lock();
        if catalog
            .value
            .entries
            .iter()
            .any(|entry| entry.projection.workspace_id == projection.workspace_id)
        {
            return Err(conflict_error("The local workspace is already registered."));
        }
        let mut next = catalog.value.clone();
        next.entries.push(PersistedWorkspace {
            projection,
            root_payload,
            revoked: false,
        });
        next.entries.sort_by(|left, right| {
            left.projection
                .workspace_id
                .cmp(&right.projection.workspace_id)
        });
        persist_catalog(
            store,
            &mut catalog,
            next,
            "workspace.granted",
            correlation_id,
        )?;
        self.invalidate_pending_recoveries();
        Ok(())
    }

    pub fn persist_revocation(
        &self,
        _authority: &ReadyAuthorityGuard<'_>,
        workspace_id: &ContractId,
        correlation_id: &ContractId,
    ) -> Result<(), LocalError> {
        let store = self.store.as_ref().ok_or_else(recovery_error)?;
        let mut catalog = self.catalog.lock();
        let mut next = catalog.value.clone();
        let entry = next
            .entries
            .iter_mut()
            .find(|entry| entry.projection.workspace_id == workspace_id.as_str() && !entry.revoked)
            .ok_or_else(|| not_found_error("The local workspace is not available."))?;
        entry.revoked = true;
        persist_catalog(
            store,
            &mut catalog,
            next,
            "workspace.revoked",
            correlation_id,
        )?;
        self.invalidate_pending_recoveries();
        Ok(())
    }

    pub fn insert_cursor(&self, target: DirectoryCursor) -> String {
        let cursor = format!("cursor_{}", Ulid::new());
        let mut state = self.cursors.lock();
        while state.order.len() >= MAX_CURSORS {
            if let Some(evicted) = state.order.pop_front() {
                state.values.remove(&evicted);
            }
        }
        state.order.push_back(cursor.clone());
        state.values.insert(cursor.clone(), target);
        cursor
    }

    pub fn resolve_cursor(
        &self,
        cursor: &str,
        renderer_session_id: &ContractId,
        workspace_id: &ContractId,
    ) -> Result<DirectoryCursor, LocalError> {
        let target = self
            .cursors
            .lock()
            .values
            .get(cursor)
            .cloned()
            .ok_or_else(|| conflict_error("The workspace view changed; refresh it and retry."))?;
        if target.renderer_session_id != *renderer_session_id
            || target.workspace_id != *workspace_id
        {
            return Err(unauthorized_error());
        }
        let binding = self
            .workspace
            .authority_binding(workspace_id.as_str())
            .map_err(|_| conflict_error("The local workspace changed; select it again."))?;
        if binding.grant_epoch != target.grant_epoch {
            return Err(conflict_error(
                "The local workspace changed; refresh it and retry.",
            ));
        }
        Ok(target)
    }

    pub fn cached_reply(&self, request_id: &ContractId) -> Option<HostDispatchReply> {
        self.replies.lock().values.get(request_id).cloned()
    }

    pub fn cache_reply(&self, request_id: ContractId, reply: HostDispatchReply) {
        let mut cache = self.replies.lock();
        if cache.values.contains_key(&request_id) {
            return;
        }
        while cache.order.len() >= MAX_CACHED_REPLIES {
            if let Some(evicted) = cache.order.pop_front() {
                cache.values.remove(&evicted);
            }
        }
        cache.order.push_back(request_id.clone());
        cache.values.insert(request_id, reply);
    }

    pub fn record_event(&self, event: ProjectionEventKind) -> u64 {
        let mut events = self.events.lock();
        let sequence = self.sequence.load(Ordering::SeqCst).saturating_add(1);
        let projection_event = ProjectionEvent {
            sequence,
            occurred_at: now(),
            event,
        };
        while events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(projection_event);
        self.sequence.store(sequence, Ordering::SeqCst);
        sequence
    }

    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    /// An optimistic consistent read of the renderer-visible host projection.
    ///
    /// The retry can now actually be taken: `host_projection_snapshot` runs on
    /// the UI thread while a dispatch runs on the async lane, so a dispatch's
    /// `record_event` can land between the two sequence reads. It still
    /// terminates, and that rests on the dispatch lane
    /// ([`DispatchLaneGuard`]): with at most one dispatch in flight there is at
    /// most one event writer, and it records a bounded number of events and
    /// then stops. Remove the lane and this becomes a loop with an unbounded
    /// number of concurrent writers, which can spin.
    pub fn snapshot(&self) -> ProjectionSnapshot {
        loop {
            let before = self.sequence();
            let boot_mode = self.boot_mode();
            let workspace_count = u32::try_from(self.workspace.list().len()).unwrap_or(u32::MAX);
            let after = self.sequence();
            if before == after {
                return ProjectionSnapshot {
                    sequence: after,
                    generated_at: now(),
                    boot_mode: boot_mode.as_str().to_owned(),
                    workspace_count,
                    active_session_id: None,
                };
            }
            std::hint::spin_loop();
        }
    }

    pub fn events_after(&self, after: u64) -> Result<Vec<ProjectionEvent>, LocalError> {
        let current = self.sequence();
        if after > current {
            return Err(conflict_error(
                "The projection cursor is ahead of the host state.",
            ));
        }
        let events = self.events.lock();
        if let Some(first) = events.front() {
            if after.saturating_add(1) < first.sequence {
                return Err(conflict_error(
                    "The projection cursor expired; request a fresh snapshot.",
                ));
            }
        }
        Ok(events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect())
    }
}

fn invalid_model_selection() -> LocalError {
    LocalError::new(
        LocalErrorCode::InvalidRequest,
        "Choose a valid model identifier and verify the native provider configuration.",
        true,
    )
}

fn load_workspace_catalog(store: &LocalStore) -> Option<(WorkspaceBroker, CatalogState)> {
    let Ok(aggregate) = store.load_aggregate("workspace_catalog", "local") else {
        return None;
    };
    let (version, catalog) = match aggregate {
        Some(record) => {
            let value: WorkspaceCatalog = deserialize_strict(record.state_json.as_bytes()).ok()?;
            if value.schema_version != WORKSPACE_CATALOG_SCHEMA {
                return None;
            }
            (record.version, value)
        }
        None => (0, WorkspaceCatalog::default()),
    };

    let mut workspace_ids = HashSet::new();
    if catalog
        .entries
        .iter()
        .any(|entry| !workspace_ids.insert(entry.projection.workspace_id.clone()))
    {
        return None;
    }
    let workspace = WorkspaceBroker::new();
    for entry in catalog.entries.iter().filter(|entry| !entry.revoked) {
        if entry.root_payload.kind != "workspace_root"
            || entry.root_payload.schema_version != WORKSPACE_ROOT_SCHEMA
        {
            return None;
        }
        let secret_bytes = store.get_payload(&entry.root_payload).ok()?;
        let secret: WorkspaceRootSecret = deserialize_strict(&secret_bytes).ok()?;
        if secret.schema_version != WORKSPACE_ROOT_SCHEMA {
            return None;
        }
        let root = decode_root(&secret.root_utf16_le)?;
        workspace
            .restore_grant(entry.projection.clone(), root, &secret.root_identity_hash)
            .ok()?;
    }

    Some((
        workspace,
        CatalogState {
            version,
            value: catalog,
        },
    ))
}

fn persist_catalog(
    store: &LocalStore,
    current: &mut CatalogState,
    next: WorkspaceCatalog,
    event_type: &str,
    correlation_id: &ContractId,
) -> Result<(), LocalError> {
    let Some(next_version) = current.version.checked_add(1) else {
        return Err(recovery_error());
    };
    let state_json = serde_json::to_string(&next).map_err(|_| recovery_error())?;
    let event = EvidenceAppend {
        stream_id: "workspace:catalog".to_owned(),
        event_type: event_type.to_owned(),
        payload_hash: sha256_bytes(state_json.as_bytes()).to_string(),
        payload_ref: None,
        correlation_id: correlation_id.to_string(),
        causation_id: None,
        redaction_level: "metadata".to_owned(),
        retention_class: "evidence".to_owned(),
    };
    store
        .append_transition(
            "workspace_catalog",
            "local",
            next_version,
            &state_json,
            &event,
        )
        .map_err(|_| recovery_error())?;
    current.version = next_version;
    current.value = next;
    Ok(())
}

#[cfg(windows)]
fn encode_root(path: &Path) -> Result<String, LocalError> {
    use std::os::windows::ffi::OsStrExt as _;

    let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let byte_count = wide.len().saturating_mul(2);
    if byte_count == 0 || byte_count > MAX_ROOT_BYTES || wide.contains(&0) {
        return Err(recovery_error());
    }
    let mut bytes = Vec::with_capacity(byte_count);
    for unit in wide {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(STANDARD_NO_PAD.encode(bytes))
}

#[cfg(windows)]
fn decode_root(encoded: &str) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;

    let bytes = STANDARD_NO_PAD.decode(encoded).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_ROOT_BYTES || bytes.len() % 2 != 0 {
        return None;
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    if wide.contains(&0) {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&wide)))
}

#[cfg(not(windows))]
fn encode_root(_path: &Path) -> Result<String, LocalError> {
    Err(recovery_error())
}

#[cfg(not(windows))]
fn decode_root(_encoded: &str) -> Option<PathBuf> {
    None
}

fn new_contract_id(prefix: &str) -> Result<ContractId, LocalError> {
    ContractId::new(format!("{prefix}_{}", Ulid::new())).map_err(|_| internal_error())
}

pub(crate) fn now() -> UnixMillis {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    UnixMillis(u64::try_from(milliseconds).unwrap_or(u64::MAX))
}

pub(crate) fn invalid_request(message: &str) -> LocalError {
    LocalError::new(LocalErrorCode::InvalidRequest, message, false)
}

pub(crate) fn unauthorized_error() -> LocalError {
    LocalError::new(
        LocalErrorCode::Unauthorized,
        "The renderer session is not authorized for this request.",
        false,
    )
}

pub(crate) fn conflict_error(message: &str) -> LocalError {
    LocalError::new(LocalErrorCode::Conflict, message, false)
}

pub(crate) fn not_found_error(message: &str) -> LocalError {
    LocalError::new(LocalErrorCode::NotFound, message, false)
}

pub(crate) fn resource_limit_error(message: &str) -> LocalError {
    LocalError::new(LocalErrorCode::ResourceLimit, message, false)
}

pub(crate) fn temporarily_unavailable(message: &str) -> LocalError {
    LocalError::new(LocalErrorCode::TemporarilyUnavailable, message, true)
}

pub(crate) fn recovery_error() -> LocalError {
    LocalError::new(
        LocalErrorCode::RecoveryRequired,
        "Local authority storage requires recovery. Inspection and recovery remain available.",
        false,
    )
}

fn internal_error() -> LocalError {
    LocalError::new(
        LocalErrorCode::Internal,
        "The desktop host could not complete the request.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    use super::*;

    const BLOCKED_ASSERTION_WINDOW: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

    fn ready_state() -> Result<HostState, String> {
        let installation_id =
            ContractId::new("installation_test").map_err(|error| error.to_string())?;
        let state = HostState::recovery(installation_id, None);
        *state.boot_mode.write() = BootMode::Ready;
        Ok(state)
    }

    fn map_local_error(error: LocalError) -> String {
        error.safe_message
    }

    #[test]
    fn model_sign_out_is_broker_free_and_advances_the_process_epoch() -> Result<(), String> {
        let state = ready_state()?;
        assert_eq!(state.model_auth_epoch(), 1);
        assert_eq!(state.sign_out_model().map_err(map_local_error)?, 2);
        assert_eq!(state.model_auth_epoch(), 2);
        assert_eq!(state.sign_out_model().map_err(map_local_error)?, 3);
        assert_eq!(state.model_auth_epoch(), 3);
        Ok(())
    }

    #[cfg(feature = "direct-model-providers")]
    #[test]
    fn runtime_provider_connection_is_ready_until_sign_out_wipes_the_transport(
    ) -> Result<(), String> {
        let state = ready_state()?;
        let credential: ModelProviderCredential =
            serde_json::from_value(serde_json::json!(
                ["sk", "test", "runtime", "only"].join("-")
            ))
            .map_err(|error| error.to_string())?;

        state
            .sign_in_model(
                ModelProviderId::OpenAi,
                "gpt-5-mini".to_owned(),
                &credential,
                None,
                None,
                UnixMillis(1_000),
            )
            .map_err(map_local_error)?;
        assert!(state.direct_provider_config().is_some());
        assert_eq!(
            state.model_configuration().map_err(map_local_error)?.mode,
            crate::bmad_model::config::HelpModelMode::DirectProviderDevelopment
        );

        state.sign_out_model().map_err(map_local_error)?;
        assert!(state.direct_provider_config().is_none());
        assert_eq!(
            state.model_configuration().map_err(map_local_error)?.mode,
            crate::bmad_model::config::HelpModelMode::Offline
        );
        Ok(())
    }

    #[test]
    fn model_sign_out_fails_closed_at_the_renderer_safe_epoch_bound() -> Result<(), String> {
        let state = ready_state()?;
        state
            .model_auth_epoch
            .store(MAX_RENDERER_SAFE_MODEL_AUTH_EPOCH, Ordering::SeqCst);

        let error = match state.sign_out_model() {
            Ok(epoch) => return Err(format!("epoch exhaustion unexpectedly advanced to {epoch}")),
            Err(error) => error,
        };
        assert_eq!(error.code, LocalErrorCode::RecoveryRequired);
        assert_eq!(state.model_auth_epoch(), MAX_RENDERER_SAFE_MODEL_AUTH_EPOCH);
        Ok(())
    }

    /// An "allow always" answer (ADR-0009) is authority the user handed to a
    /// particular model, about a particular workspace, in a particular session.
    /// Every transition that withdraws one of those has to withdraw the answer
    /// with it, or the next model inherits a yes it never asked for.
    ///
    /// `GrantedRules::clear` had no caller at all for a while — `tool_grants`
    /// was only ever read — so this was true of sign-out, of recovery, and of
    /// revocation alike. Revocation is covered by
    /// `commands::tests::workspace_revocation_withdraws_every_lane_the_grant_authorized`;
    /// these are the other two.
    #[test]
    fn model_sign_out_withdraws_every_session_tool_grant() -> Result<(), String> {
        let state = ready_state()?;
        assert!(state.tool_grants.lock().grant("read", "notes.md"));
        assert!(!state.tool_grants.lock().rules().is_empty());

        state.sign_out_model().map_err(map_local_error)?;

        assert!(
            state.tool_grants.lock().rules().is_empty(),
            "a session tool grant must not outlive the model it was given to",
        );
        Ok(())
    }

    #[test]
    fn entering_recovery_withdraws_every_session_tool_grant() -> Result<(), String> {
        let state = ready_state()?;
        assert!(state.tool_grants.lock().grant("read", "notes.md"));
        assert!(!state.tool_grants.lock().rules().is_empty());

        state.enter_recovery();

        assert_eq!(state.boot_mode(), BootMode::ReadOnlyRecovery);
        assert!(
            state.tool_grants.lock().rules().is_empty(),
            "recovery has no agent tool lane, so no session grant may survive into it",
        );
        Ok(())
    }

    fn assert_single_recovery_event(state: &HostState) -> Result<(), String> {
        let events = state.events_after(0).map_err(map_local_error)?;
        assert_eq!(state.sequence(), 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert!(matches!(
            &events[0].event,
            ProjectionEventKind::BootStateChanged { mode }
                if mode == BootMode::ReadOnlyRecovery.as_str()
        ));
        Ok(())
    }

    #[test]
    fn initialization_projects_the_stable_sealed_store_installation_identity() -> Result<(), String>
    {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let root = directory.path().join("authority");
        let first = HostState::initialize(Some(root.clone())).map_err(map_local_error)?;
        let first_identity = first
            .store
            .as_ref()
            .ok_or_else(|| "ready store is unavailable".to_owned())?
            .local_identity()
            .map_err(|error| error.to_string())?;
        assert_eq!(first.installation_id(), first_identity.installation_id());
        let retained = first.installation_id().clone();
        drop(first);

        let reopened = HostState::initialize(Some(root)).map_err(map_local_error)?;
        assert_eq!(reopened.installation_id(), &retained);
        Ok(())
    }

    fn simulated_renderer_error_scope(state: &HostState) -> Result<(), &'static str> {
        let _authority = state
            .renderer_session_authority("main")
            .ok_or("renderer session is unavailable")?;
        Err("simulated guarded error")
    }

    #[test]
    fn renderer_guard_blocks_rebind_and_error_scope_releases() -> Result<(), String> {
        let state = Arc::new(ready_state()?);
        let original_session = state.bind_renderer("main").map_err(map_local_error)?;
        let authority = state
            .renderer_session_authority("main")
            .ok_or_else(|| "renderer session is unavailable".to_owned())?;
        assert_eq!(authority.session_id(), &original_session);
        assert!(state.renderer_sessions.try_write().is_none());

        let start = Arc::new(Barrier::new(2));
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        let worker_start = Arc::clone(&start);
        let worker = thread::spawn(move || {
            worker_start.wait();
            let _ = attempted_tx.send(());
            let rebound = worker_state.bind_renderer("main");
            let _ = completed_tx.send(rebound);
        });

        start.wait();
        attempted_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("rebind worker did not start: {error}"))?;
        assert!(matches!(
            completed_rx.recv_timeout(BLOCKED_ASSERTION_WINDOW),
            Err(RecvTimeoutError::Timeout)
        ));
        drop(authority);
        let rebound_session = completed_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("rebind did not complete after guard release: {error}"))?
            .map_err(map_local_error)?;
        worker
            .join()
            .map_err(|_| "rebind worker panicked".to_owned())?;

        assert_ne!(rebound_session, original_session);
        {
            let current = state
                .renderer_session_authority("main")
                .ok_or_else(|| "rebound renderer session is unavailable".to_owned())?;
            assert_eq!(current.session_id(), &rebound_session);
            assert_ne!(current.session_id(), &original_session);
        }

        assert_eq!(
            simulated_renderer_error_scope(&state),
            Err("simulated guarded error")
        );
        assert!(state.renderer_sessions.try_write().is_some());
        Ok(())
    }

    #[test]
    fn workspace_commit_guard_orders_ready_checks_and_recovery_without_a_race() -> Result<(), String>
    {
        let state = Arc::new(ready_state()?);
        let authority = state.ready_workspace_commit().map_err(map_local_error)?;
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        let worker = thread::spawn(move || {
            let _ = attempted_tx.send(());
            let result = worker_state
                .ready_workspace_commit()
                .map(|_| ())
                .map_err(map_local_error);
            let _ = completed_tx.send(result);
        });

        attempted_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("workspace commit worker did not start: {error}"))?;
        assert!(matches!(
            completed_rx.recv_timeout(BLOCKED_ASSERTION_WINDOW),
            Err(RecvTimeoutError::Timeout)
        ));

        authority.enter_recovery();
        let result = completed_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("workspace commit worker did not complete: {error}"))?;
        assert!(result.is_err_and(|message| message.contains("requires recovery")));
        worker
            .join()
            .map_err(|_| "workspace commit worker panicked".to_owned())?;
        assert_single_recovery_event(&state)
    }

    #[test]
    fn ready_guard_blocks_recovery_until_stateful_scope_ends() -> Result<(), String> {
        let state = Arc::new(ready_state()?);
        let authority = state.ready_authority().map_err(map_local_error)?;
        assert!(state.boot_mode.try_write().is_none());

        let start = Arc::new(Barrier::new(2));
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker_state = Arc::clone(&state);
        let worker_start = Arc::clone(&start);
        let worker = thread::spawn(move || {
            worker_start.wait();
            let _ = attempted_tx.send(());
            let sequence = worker_state.enter_recovery();
            let _ = completed_tx.send(sequence);
        });

        start.wait();
        attempted_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("recovery worker did not start: {error}"))?;
        assert!(matches!(
            completed_rx.recv_timeout(BLOCKED_ASSERTION_WINDOW),
            Err(RecvTimeoutError::Timeout)
        ));
        assert!(
            state.bmad_model.try_lock().is_some(),
            "recovery must not hold the Help coordinator while waiting for Ready"
        );

        drop(authority);
        let transition_sequence = completed_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("recovery did not complete after guard release: {error}"))?;
        worker
            .join()
            .map_err(|_| "recovery worker panicked".to_owned())?;

        assert_eq!(transition_sequence, 1);
        assert_eq!(state.boot_mode(), BootMode::ReadOnlyRecovery);
        assert_single_recovery_event(&state)
    }

    fn workspace_event(label: &str) -> Result<ProjectionEventKind, String> {
        Ok(ProjectionEventKind::WorkspaceChanged {
            workspace_id: ContractId::new(label).map_err(|error| error.to_string())?,
        })
    }

    /// The freeze, and its absence, as far as this process can observe them.
    ///
    /// The thread placement itself is the compiler's to prove —
    /// `#[tauri::command(async)]` routes the body through
    /// `async_runtime::spawn` — but the consequence that made the freeze
    /// *survive* the move is observable here: a dispatch that blocks for a
    /// provider round trip must not park the commands the window itself needs.
    /// `host_bootstrap` takes the write half of `renderer_sessions`, and
    /// `host_dispatch` used to hold the read half for its entire body, so a
    /// rebind — and, `parking_lot` being task-fair, every projection read queued
    /// behind that rebind — waited out the whole round trip.
    ///
    /// The worker below is `host_dispatch`'s shape exactly: enter the lane, read
    /// the bound renderer identity *without* pinning it, then block.
    #[test]
    fn a_blocked_dispatch_does_not_park_the_window_facing_commands() -> Result<(), String> {
        let state = Arc::new(ready_state()?);
        let bound = state.bind_renderer("main").map_err(map_local_error)?;

        let (in_flight_tx, in_flight) = mpsc::channel();
        let (release_tx, release) = mpsc::channel::<()>();
        let dispatch_state = Arc::clone(&state);
        let dispatch = thread::spawn(move || {
            let lane = dispatch_state.dispatch_lane();
            let session_id = dispatch_state.renderer_session_id("main");
            let _ = in_flight_tx.send(session_id);
            // Stands in for the blocking provider round trip.
            let _ = release.recv_timeout(COMPLETION_TIMEOUT);
            drop(lane);
        });

        let in_flight_session = in_flight
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("the dispatch never reached its round trip: {error}"))?
            .ok_or_else(|| "the dispatch read no renderer session".to_owned())?;
        assert_eq!(in_flight_session, bound);

        // Detached rather than joined: on the code this test exists to catch
        // these calls never return, so joining would hang the suite instead of
        // failing it.
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let window_state = Arc::clone(&state);
        thread::spawn(move || {
            let _ = attempted_tx.send(());
            // `host_bootstrap`, then `host_projection_snapshot`/`_events`.
            let rebound = window_state.bind_renderer("main");
            let snapshot = window_state.snapshot();
            let events = window_state.events_after(0);
            let _ = completed_tx.send(rebound.map(|session| (session, snapshot, events)));
        });

        attempted_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("the window-facing worker did not start: {error}"))?;
        let (rebound, snapshot, events) = completed_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| {
                format!("the window lane parked behind an in-flight dispatch: {error}")
            })?
            .map_err(map_local_error)?;

        assert_ne!(rebound, bound);
        assert_eq!(snapshot.boot_mode, BootMode::Ready.as_str());
        assert!(events.map_err(map_local_error)?.is_empty());

        let _ = release_tx.send(());
        dispatch
            .join()
            .map_err(|_| "the dispatch worker panicked".to_owned())?;
        Ok(())
    }

    /// A dispatch reply may publish its own dispatch's projection sequence and
    /// no other's.
    ///
    /// `HostClient.projectionEvents` only ever asks for events *after* the
    /// cursor it holds, so a reply stamped with a sequence covering an event
    /// some other dispatch appended tells the renderer that event was delivered
    /// when it never was — and it is never offered again. On the main thread
    /// this was impossible by accident; the lane is what keeps it impossible.
    #[test]
    fn a_dispatch_cannot_publish_a_sequence_another_dispatch_appended() -> Result<(), String> {
        let state = Arc::new(ready_state()?);

        let (recorded_tx, recorded) = mpsc::channel();
        let (release_tx, release) = mpsc::channel::<()>();
        let (published_tx, published) = mpsc::channel();
        let first_state = Arc::clone(&state);
        let first_event = workspace_event("workspace_first")?;
        let first = thread::spawn(move || {
            let lane = first_state.dispatch_lane();
            let entered_at = lane.sequence();
            first_state.record_event(first_event);
            let _ = recorded_tx.send(());
            let _ = release.recv_timeout(COMPLETION_TIMEOUT);
            let _ = published_tx.send((entered_at, lane.sequence()));
        });

        recorded
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("the first dispatch never recorded its event: {error}"))?;

        // A second dispatch arrives while the first is still open.
        let (second_tx, second_rx) = mpsc::channel();
        let second_state = Arc::clone(&state);
        let second_event = workspace_event("workspace_second")?;
        let second = thread::spawn(move || {
            let lane = second_state.dispatch_lane();
            let entered_at = lane.sequence();
            second_state.record_event(second_event);
            let _ = second_tx.send((entered_at, lane.sequence()));
        });

        // It cannot append anything into the window the first will publish.
        let blocked = matches!(
            second_rx.recv_timeout(BLOCKED_ASSERTION_WINDOW),
            Err(RecvTimeoutError::Timeout)
        );
        assert!(
            blocked,
            "a second dispatch appended a projection event inside the first dispatch's window",
        );

        let _ = release_tx.send(());
        let (first_entered_at, first_published) = published
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("the first dispatch never published: {error}"))?;
        assert_eq!(first_entered_at, 0);
        assert_eq!(
            first_published, 1,
            "a reply published a sequence that covers another dispatch's event",
        );

        let (second_entered_at, second_published) = second_rx
            .recv_timeout(COMPLETION_TIMEOUT)
            .map_err(|error| format!("the second dispatch never published: {error}"))?;
        assert_eq!(second_entered_at, 1);
        assert_eq!(second_published, 2);

        for worker in [first, second] {
            worker
                .join()
                .map_err(|_| "a dispatch worker panicked".to_owned())?;
        }
        assert_eq!(state.sequence(), 2);
        Ok(())
    }

    /// Releasing the pin between validation and execution makes a mid-dispatch
    /// rebind reachable for the first time, so the identity check that was
    /// previously vacuous now has to hold.
    #[test]
    fn re_pinning_a_rebound_window_is_refused() -> Result<(), String> {
        let state = ready_state()?;
        let first = state.bind_renderer("main").map_err(map_local_error)?;
        assert!(state.pin_renderer_session("main", &first).is_some());

        let second = state.bind_renderer("main").map_err(map_local_error)?;
        assert_ne!(second, first);
        assert!(
            state.pin_renderer_session("main", &first).is_none(),
            "a command must not execute against the renderer session the window replaced",
        );
        let current = state
            .pin_renderer_session("main", &second)
            .ok_or_else(|| "the current renderer session must still pin".to_owned())?;
        assert_eq!(current.session_id(), &second);
        assert!(state.pin_renderer_session("other", &second).is_none());
        Ok(())
    }

    #[test]
    fn concurrent_recovery_transition_is_idempotent() -> Result<(), String> {
        const WORKER_COUNT: usize = 8;

        let state = Arc::new(ready_state()?);
        let start = Arc::new(Barrier::new(WORKER_COUNT + 1));
        let mut workers = Vec::with_capacity(WORKER_COUNT);
        for _ in 0..WORKER_COUNT {
            let worker_state = Arc::clone(&state);
            let worker_start = Arc::clone(&start);
            workers.push(thread::spawn(move || {
                worker_start.wait();
                worker_state.enter_recovery()
            }));
        }

        start.wait();
        for worker in workers {
            let sequence = worker
                .join()
                .map_err(|_| "concurrent recovery worker panicked".to_owned())?;
            assert_eq!(sequence, 1);
        }

        assert_eq!(state.boot_mode(), BootMode::ReadOnlyRecovery);
        assert_single_recovery_event(&state)
    }
}
