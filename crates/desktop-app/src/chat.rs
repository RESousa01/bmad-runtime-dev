//! Host composition for the ordinary chat lane — the app's default path.
//!
//! A chat turn is a governed model request like any other: the transcript and
//! any attached workspace files are assembled into one sealed context
//! manifest, bound to a single-use consent decision, dispatched, and verified
//! back into a bounded archetype before it reaches the transcript.
//!
//! Two things differ from the capability lane, both deliberate:
//!
//! * **One command, not four.** A capability run splits prepare / approve /
//!   submit because its output can propose workspace changes, so the user must
//!   see the exact reviewed context before consenting. A chat turn produces
//!   text and nothing else, and its reviewed context is what the user already
//!   curated in the Context panel plus their own words — so pressing send *is*
//!   the consent. The manifest, decision, and receipt are still minted and
//!   still bind the exact bytes sent.
//! * **The persona is optional.** With no agent selected this is an ordinary
//!   assistant. Selecting one adds the sealed persona projection as reviewed
//!   context; it grants nothing and can be cleared at any time.
//!
//! The coordinator mutex is never held across the model round trip: `begin`
//! builds the authorized request under the lock, the caller dispatches
//! unlocked, and `finalize` or `abort` re-acquires it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use desktop_cloud::{AuthorizedModelRequest, RawModelOutput};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressError, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, TurnEgressEntry,
    TurnEgressGrant, MAX_TURN_MODEL_CALLS,
};
use desktop_runtime::{
    canonical_hash, deserialize_strict_json, sha256_bytes, ChatMessage, ChatReply, ChatSession,
    ChatTurn, ContractId, RelativeWorkspacePath, Sha256Digest, ToolInvocation, UnixMillis,
    CHAT_TOOL_TURN_SCHEMA, CHAT_TURN_SCHEMA, MAX_PROMPT_TRANSCRIPT_BYTES,
};
use desktop_store::{ChatTurnAppend, ChatTurnMessage, LocalStore};
use serde::Serialize;
use serde_json::Value;

use crate::bmad_model::config::{HelpModelConfiguration, HelpModelMode};
use crate::bmad_model::context::derived_contract_id;

/// Conversations retained per host process. Bounds unbounded growth across a
/// long session with many tasks; the oldest idle conversation is evicted.
const MAX_CHAT_SESSIONS: usize = 64;
const CHAT_MANIFEST_LIFETIME_MS: u64 = 10 * 60 * 1_000;
/// Single-use consent decisions are capped by `desktop-egress`
/// (`MAX_DECISION_LIFETIME_MS`, 5 minutes). The chat manifest deliberately
/// lives longer than that, so the decision window is clamped against it the
/// same way the capability lane does — a manifest may outlive the one approval
/// minted from it, never the reverse.
const CHAT_DECISION_WINDOW_MS: u64 = 5 * 60 * 1_000;
/// Upper bound on sealed context items in one chat manifest: at most 100 user
/// attachments, the single persona candidate, and the single transcript
/// candidate. Must stay at or under `desktop-egress`'s
/// `HARD_MAX_CONTEXT_ITEMS` (128), which rejects the manifest outright.
///
/// The transcript is deliberately **one** item however long the conversation
/// is. An earlier revision emitted one item per message, bounded only by bytes
/// while retention kept 200 messages — so a long conversation silently crossed
/// the ceiling and every later send failed to seal, permanently, with no way
/// back except starting over.
const MAX_CHAT_CONTEXT_ITEMS: u32 = 102;
const MAX_CHAT_CONTEXT_BYTES: u64 = 512 * 1024;
/// The egress purpose label for the chat lane. Distinct from every capability
/// purpose, so a chat manifest can never be replayed as a capability manifest.
const CHAT_PURPOSE: &str = "chat.turn";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChatCoordinatorError {
    Unauthorized,
    Conflict,
    /// The turn's egress grant window closed before this call was authorized.
    /// Distinct from `Conflict` because it is a different fact about a
    /// different thing: nothing is in flight, the turn simply ran out of time.
    TurnExpired,
    /// The turn spent every model call its grant allows. Also distinct from
    /// `Conflict`, and from `TurnExpired`: the user's next step is a new
    /// message, not the same one again.
    TurnBudgetSpent,
    OutputRejected,
    Transport,
    Integrity,
}

/// One completed exchange as the renderer sees it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatTurnProjection {
    pub session_id: ContractId,
    pub agent_code: Option<String>,
    pub user_message: ChatMessage,
    pub assistant_message: ChatMessage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatHistoryProjection {
    pub session_id: ContractId,
    pub agent_code: Option<String>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatSessionIdentity<'a> {
    schema_version: &'static str,
    installation_id: &'a ContractId,
    workspace_id: &'a ContractId,
    client_session_ref: &'a str,
}

#[derive(Serialize)]
struct ChatRequestIdentity<'a> {
    schema_version: &'static str,
    session_id: &'a ContractId,
    workspace_id: &'a ContractId,
    manifest_hash: Sha256Digest,
    created_at: UnixMillis,
}

/// Everything one dispatch needs, handed to the caller with the lock released.
pub(crate) struct ChatDispatch {
    pub dispatch_id: u64,
    pub authorized_request: AuthorizedModelRequest,
    pub deterministic_fixture: String,
    pub sent_at: UnixMillis,
}

/// Which reply schema one turn is sealed under — and therefore which system
/// prompt the provider selects for it, since `model_instructions` is chosen
/// from the sealed `canonical_output_schema_id` and nothing else.
///
/// This is the single decision that keeps the promise made to the model in
/// step with what the host will actually honour. The v1 prompt tells the model
/// *"you cannot read, write, or execute anything"*; the v2 prompt names the
/// four read-only tools it may ask for. Sealing one schema while intending the
/// other either strands the tool loop behind a prompt that never mentions it,
/// or advertises tools to a turn that cannot run any.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChatOutputSchema {
    /// Prose only. No tool was granted, so none may be asked for.
    TextOnly,
    /// Tools were granted for this turn: the reply may be prose *or* one tool
    /// request. A request is still only a request — the permission gate, not
    /// the schema, decides whether it runs.
    ToolBearing,
}

impl ChatOutputSchema {
    /// Picks the schema from the turn's tool grant, so the two can never be
    /// decided independently.
    pub(crate) const fn for_tool_grant(tools_granted: bool) -> Self {
        if tools_granted {
            Self::ToolBearing
        } else {
            Self::TextOnly
        }
    }

    /// The contract identifier sealed into the manifest and the invocation
    /// binding. The hash sealed alongside it is derived from this same string,
    /// never from a second literal.
    const fn schema_id(self) -> &'static str {
        match self {
            Self::TextOnly => CHAT_TURN_SCHEMA,
            Self::ToolBearing => CHAT_TOOL_TURN_SCHEMA,
        }
    }

    const fn tools_granted(self) -> bool {
        matches!(self, Self::ToolBearing)
    }
}

pub(crate) struct BeginChatTurn<'a> {
    pub installation_id: &'a ContractId,
    pub project_id: &'a ContractId,
    pub workspace_id: &'a ContractId,
    pub client_session_ref: &'a str,
    pub agent_code: Option<String>,
    pub persona_instruction: Option<&'a [u8]>,
    pub text: String,
    pub attachments: Vec<ContextCandidate>,
    pub configuration: &'a HelpModelConfiguration,
    /// Decided by the caller, which is the only place that knows whether this
    /// turn is granted tools. Never inferred here.
    pub output_schema: ChatOutputSchema,
    pub now: UnixMillis,
}

struct AuthorizeChatTurn<'a> {
    manifest: &'a desktop_egress::ContextEgressManifest,
    binding: &'a desktop_egress::ModelInvocationBinding,
    decision_id: ContractId,
    invocation_id: ContractId,
    now: UnixMillis,
}

/// Everything `seal_turn` needs, owned, so a turn can be sealed more than once.
///
/// A tool loop re-seals on every iteration: the transcript has grown, so the
/// manifest and its decision must be minted afresh. `BeginChatTurn` borrows for
/// exactly one call, which is why this exists. `text` is deliberately absent —
/// the user's message is already in the transcript by then, and sealing never
/// reads it.
struct PendingTurn {
    installation_id: ContractId,
    project_id: ContractId,
    workspace_id: ContractId,
    client_session_ref: String,
    persona_instruction: Option<Vec<u8>>,
    attachments: Vec<ContextCandidate>,
    configuration: HelpModelConfiguration,
    /// Carried, not re-decided. Every iteration of one turn's loop is sealed
    /// under the schema the turn opened with, so the prompt the model reads
    /// cannot change underneath it mid-turn.
    output_schema: ChatOutputSchema,
}

impl PendingTurn {
    /// Captures the sealing inputs a later loop iteration will need, since
    /// `BeginChatTurn` borrows for exactly one call.
    fn from(input: &BeginChatTurn<'_>) -> Self {
        Self {
            installation_id: input.installation_id.clone(),
            project_id: input.project_id.clone(),
            workspace_id: input.workspace_id.clone(),
            client_session_ref: input.client_session_ref.to_owned(),
            persona_instruction: input.persona_instruction.map(<[u8]>::to_vec),
            attachments: input.attachments.clone(),
            configuration: input.configuration.clone(),
            output_schema: input.output_schema,
        }
    }
}

struct InFlightTurn {
    session_id: ContractId,
    pending: PendingTurn,
    /// The turn's egress budget and ledger (D7). One per turn, never cloned.
    grant: TurnEgressGrant,
    /// What this dispatch sent, ready to be written to the ledger the moment
    /// the send resolves — either way. Egress is caused by the send, not by the
    /// send succeeding, so the failure path records it too.
    sent: TurnEgressEntry,
}

/// What one completed dispatch means for the turn.
pub(crate) enum TurnOutcome {
    /// The model answered. The turn is closed.
    Completed(Box<ChatTurnProjection>),
    /// The model asked for a tool. The turn stays open, and the host must run
    /// it and resume — this is a *request*, and carries no authority.
    ToolRequested(ToolInvocation),
}

pub(crate) struct ChatCoordinator {
    ledger: MemoryDecisionLedger,
    sessions: BTreeMap<ContractId, ChatSession>,
    /// Insertion/most-recent-use order, oldest first, for eviction.
    order: Vec<ContractId>,
    /// Turns awaiting their model reply, keyed by dispatch id.
    ///
    /// A map rather than one slot: a turn is now a loop of up to
    /// `MAX_TURN_MODEL_CALLS` calls and is held open across every one of them,
    /// so a single global slot meant one agent loop blocked every other
    /// conversation in the app for its whole duration. Keyed by dispatch id
    /// because that is what `finalize`/`abort`/`resume` are handed; the
    /// per-session rule is enforced separately in `begin_turn`.
    in_flight: BTreeMap<u64, InFlightTurn>,
    /// Conversations whose durable transcript is on disk but cannot be rebuilt
    /// into a live session.
    ///
    /// Such a transcript is unreadable, not absent, and the difference matters:
    /// treating it as absent meant `history` handed back an empty conversation
    /// and the next send opened a *fresh* in-memory session over it, so every
    /// later turn appended to a durable record the user could never see. A
    /// conversation in here fails closed on both paths instead.
    unreadable: BTreeSet<ContractId>,
    dispatch_seq: u64,
}

impl ChatCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            sessions: BTreeMap::new(),
            order: Vec::new(),
            in_flight: BTreeMap::new(),
            unreadable: BTreeSet::new(),
            dispatch_seq: 0,
        }
    }

    /// Drops every conversation, for sign-out and workspace revocation
    /// (ADR-0002). An in-flight reply is abandoned rather than delivered into
    /// a transcript the user is no longer authorized to see.
    pub(crate) fn invalidate(&mut self) {
        self.sessions.clear();
        self.order.clear();
        self.in_flight.clear();
        self.unreadable.clear();
    }

    /// How many conversations this process is holding in memory.
    ///
    /// Test-only, and the only observable `invalidate` has: residency is the
    /// whole point of it, and `history` will happily rehydrate a cleared
    /// session straight back out of the store, so the ordinary read path cannot
    /// distinguish "dropped" from "never dropped".
    #[cfg(test)]
    pub(crate) fn resident_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Loads one conversation, rehydrating it from the store when this process
    /// has not seen it yet. That is what makes a transcript survive a restart:
    /// the store is the record, this map is only a live cache.
    ///
    /// # Errors
    ///
    /// Returns [`ChatCoordinatorError::Integrity`] when a durable transcript
    /// exists but cannot be rebuilt. That is deliberately *not* reported as an
    /// empty conversation: an empty conversation invites the user to type into
    /// a record they cannot see.
    pub(crate) fn history(
        &mut self,
        installation_id: &ContractId,
        workspace_id: &ContractId,
        client_session_ref: &str,
        store: &LocalStore,
    ) -> Result<ChatHistoryProjection, ChatCoordinatorError> {
        let session_id = chat_session_id(installation_id, workspace_id, client_session_ref)?;
        if !self.sessions.contains_key(&session_id) {
            self.rehydrate(&session_id, workspace_id, store)?;
        }
        let session = self.sessions.get(&session_id);
        Ok(ChatHistoryProjection {
            session_id,
            agent_code: session.and_then(|session| session.agent_code().map(str::to_owned)),
            messages: session
                .map(|session| session.messages().to_vec())
                .unwrap_or_default(),
        })
    }

    /// Rebuilds one conversation from durable rows. A stored transcript that
    /// does not satisfy the runtime's own invariants is dropped rather than
    /// half-loaded, so a corrupt record can never become live state.
    ///
    /// # Errors
    ///
    /// Returns [`ChatCoordinatorError::Unauthorized`] when the stored session
    /// belongs to another workspace, and [`ChatCoordinatorError::Integrity`]
    /// when it cannot be read or rebuilt. Nothing stored at all is `Ok`: a
    /// conversation that has never been written to is simply new.
    fn rehydrate(
        &mut self,
        session_id: &ContractId,
        workspace_id: &ContractId,
        store: &LocalStore,
    ) -> Result<(), ChatCoordinatorError> {
        let unreadable = |coordinator: &mut Self, error: ChatCoordinatorError| {
            coordinator.unreadable.insert(session_id.clone());
            Err(error)
        };
        let loaded = store.chat_session(session_id.as_str());
        let stored = match loaded {
            Ok(Some(stored)) => stored,
            Ok(None) => {
                self.unreadable.remove(session_id);
                return Ok(());
            }
            Err(_) => return unreadable(self, ChatCoordinatorError::Integrity),
        };
        if stored.workspace_id != workspace_id.as_str() {
            return unreadable(self, ChatCoordinatorError::Unauthorized);
        }
        let mut session = ChatSession::open(session_id.clone(), workspace_id.clone());
        session.set_agent_code(stored.agent_code.clone());
        for message in stored.messages {
            let at = UnixMillis(message.created_at_ms);
            let applied = match message.role.as_str() {
                "user" => session.append_user(message.text, at),
                "assistant" => {
                    ChatTurn::new(message.text).and_then(|turn| session.append_assistant(&turn, at))
                }
                // Stored as "<tool> <subject>" by `append_tool_call`, so the
                // split has to mirror that exactly or the turn will not rebuild.
                "tool_call" => match message.text.split_once(' ') {
                    Some((tool, subject)) => {
                        ToolInvocation::new(tool.to_owned(), subject.to_owned())
                            .and_then(|invocation| session.append_tool_call(&invocation, at))
                    }
                    None => return unreadable(self, ChatCoordinatorError::Integrity),
                },
                "tool_result" => session.append_tool_result(message.text, at),
                _ => return unreadable(self, ChatCoordinatorError::Integrity),
            };
            if applied.is_err() {
                return unreadable(self, ChatCoordinatorError::Integrity);
            }
        }
        // A dangling question would block the next send; drop it on load.
        let dangling = session.has_pending_user_message();
        session.rollback_pending_user();
        if dangling {
            // The durable rows behind it are the ones just dropped, and only
            // those: `repair_dangling_chat_tail` removes exactly the run after
            // the newest assistant reply. Reached only for a store an older
            // build left mid-turn — a turn is now written all-or-nothing, so
            // this build cannot produce a dangling tail of its own.
            //
            // Best effort on purpose. A failed repair leaves the conversation
            // exactly as wedged as it already was, while still loading for
            // reading; the next send then fails loudly at the store's grammar
            // rather than silently growing an invisible transcript. Failing the
            // whole load here would take away read access too, over a
            // recovery step that was never owed.
            let _ = store.repair_dangling_chat_tail(session_id.as_str(), workspace_id.as_str());
        }
        self.unreadable.remove(session_id);
        self.sessions.insert(session_id.clone(), session);
        self.order.push(session_id.clone());
        self.evict_oldest(session_id);
        Ok(())
    }

    /// Whether this conversation has a durable transcript that could not be
    /// rebuilt. Sending into one is refused: the record is unreadable, not
    /// absent, and a new turn appended to it would be invisible to the user.
    fn is_unreadable(&self, session_id: &ContractId) -> bool {
        self.unreadable.contains(session_id)
    }

    fn session_mut(
        &mut self,
        session_id: &ContractId,
        workspace_id: &ContractId,
    ) -> &mut ChatSession {
        if !self.sessions.contains_key(session_id) {
            self.sessions.insert(
                session_id.clone(),
                ChatSession::open(session_id.clone(), workspace_id.clone()),
            );
            self.order.push(session_id.clone());
            self.evict_oldest(session_id);
        }
        self.sessions
            .entry(session_id.clone())
            .or_insert_with(|| ChatSession::open(session_id.clone(), workspace_id.clone()))
    }

    /// Evicts oldest-first, never the conversation just touched.
    fn evict_oldest(&mut self, keep: &ContractId) {
        while self.order.len() > MAX_CHAT_SESSIONS {
            let Some(position) = self.order.iter().position(|candidate| candidate != keep) else {
                break;
            };
            let evicted = self.order.remove(position);
            self.sessions.remove(&evicted);
        }
    }

    /// Appends the user's message and builds the authorized request under the
    /// lock. The caller dispatches with the lock released.
    ///
    /// # Errors
    ///
    /// Fails closed when the model configuration is offline, another turn is
    /// already in flight, the message is unbounded, or any identity or
    /// manifest cannot be sealed.
    pub(crate) fn begin_turn(
        &mut self,
        mut input: BeginChatTurn<'_>,
    ) -> Result<ChatDispatch, ChatCoordinatorError> {
        // Reject only a second turn in the *same* conversation. Two different
        // conversations may now run at once.
        let session_busy = |coordinator: &Self, id: &ContractId| {
            coordinator
                .in_flight
                .values()
                .any(|turn| turn.session_id == *id)
        };
        if input.configuration.mode == HelpModelMode::Offline {
            return Err(ChatCoordinatorError::Transport);
        }
        let session_id = chat_session_id(
            input.installation_id,
            input.workspace_id,
            input.client_session_ref,
        )?;
        if session_busy(self, &session_id) {
            return Err(ChatCoordinatorError::Conflict);
        }
        // A conversation whose durable transcript could not be rebuilt is not
        // an empty one. Opening a fresh in-memory session over it is what made
        // every later turn append to a record the user cannot see, so sending
        // is refused until the record can be read again.
        if self.is_unreadable(&session_id) {
            return Err(ChatCoordinatorError::Integrity);
        }
        {
            let session = self.session_mut(&session_id, input.workspace_id);
            if input.agent_code.is_some() {
                session.set_agent_code(input.agent_code.clone());
            }
            session
                .append_user(std::mem::take(&mut input.text), input.now)
                .map_err(|_| ChatCoordinatorError::OutputRejected)?;
        }

        let dispatch = self.seal_turn(&session_id, &input, None);
        match dispatch {
            Ok(dispatch) => Ok(dispatch),
            Err(error) => {
                // Never leave the question stranded when sealing failed.
                if let Some(session) = self.sessions.get_mut(&session_id) {
                    session.rollback_pending_user();
                }
                Err(error)
            }
        }
    }

    fn seal_turn(
        &mut self,
        session_id: &ContractId,
        input: &BeginChatTurn<'_>,
        carried_grant: Option<TurnEgressGrant>,
    ) -> Result<ChatDispatch, ChatCoordinatorError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ChatCoordinatorError::Integrity)?;
        let mut candidates = Vec::new();
        if let Some(persona) = input.persona_instruction {
            candidates.push(persona_candidate(persona)?);
        }
        candidates.extend(transcript_candidate(
            session.prompt_window(MAX_PROMPT_TRANSCRIPT_BYTES),
        )?);
        candidates.extend(input.attachments.iter().cloned());

        let manifest = seal_chat_manifest(input, candidates)?;

        let request_digest = canonical_hash(
            "chat-turn-request-id",
            1,
            &ChatRequestIdentity {
                schema_version: "sapphirus.chat-turn-request.v1",
                session_id,
                workspace_id: input.workspace_id,
                manifest_hash: manifest.manifest_hash,
                created_at: input.now,
            },
        )
        .map_err(|_| ChatCoordinatorError::Integrity)?;
        let request_id = derived_contract_id("modelreq", request_digest)
            .map_err(|_| ChatCoordinatorError::Integrity)?;
        let decision_id = derived_contract_id("decision", request_digest)
            .map_err(|_| ChatCoordinatorError::Integrity)?;
        let invocation_id = derived_contract_id("invoke", request_digest)
            .map_err(|_| ChatCoordinatorError::Integrity)?;

        let binding = ModelInvocationBindingDraft {
            schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
            request_id,
            tenant_ref: manifest.draft.tenant_ref.clone(),
            project_ref: manifest.draft.project_ref.clone(),
            run_ref: manifest.draft.run_ref.clone(),
            installation_id: input.installation_id.clone(),
            session_authority_hash: canonical_hash(
                "chat-session-authority",
                1,
                &ChatSessionIdentity {
                    schema_version: "sapphirus.chat-session.v1",
                    installation_id: input.installation_id,
                    workspace_id: input.workspace_id,
                    client_session_ref: input.client_session_ref,
                },
            )
            .map_err(|_| ChatCoordinatorError::Integrity)?,
            manifest_hash: manifest.manifest_hash,
            purpose: CHAT_PURPOSE.to_owned(),
            model_role: manifest.draft.model_role.clone(),
            canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
            provider_profile_hash: manifest.draft.provider_profile_hash,
            model_profile_hash: manifest.draft.model_profile_hash,
            deployment_hash: manifest.draft.deployment_hash,
            policy_hash: manifest.draft.policy_hash,
            region: manifest.draft.region.clone(),
            retention_mode: manifest.draft.retention_mode,
            consent_disclosure_hash: sha256_bytes(
                b"Sending this message sends the conversation and any context you attached, once.",
            ),
        }
        .seal()
        .map_err(|_| ChatCoordinatorError::Integrity)?;

        let authorized_request = self.authorize(AuthorizeChatTurn {
            manifest: &manifest,
            binding: &binding,
            decision_id,
            invocation_id,
            now: input.now,
        })?;

        self.dispatch_seq = self.dispatch_seq.wrapping_add(1);
        let dispatch_id = self.dispatch_seq;
        let grant = turn_grant_for(session_id, carried_grant, input.now)?;
        let turn = InFlightTurn {
            session_id: session_id.clone(),
            pending: PendingTurn::from(input),
            grant,
            sent: TurnEgressEntry {
                manifest_hash: manifest.manifest_hash,
                item_count: u32::try_from(manifest.draft.items.len()).unwrap_or(u32::MAX),
                context_bytes: manifest.draft.total_outbound_bytes,
                at: input.now,
            },
        };
        self.in_flight.insert(dispatch_id, turn);
        Ok(ChatDispatch {
            dispatch_id,
            authorized_request,
            deterministic_fixture: deterministic_chat_fixture(input.output_schema),
            sent_at: input.now,
        })
    }

    /// Mints the single-use decision, consumes it, and builds the authorized
    /// request. Consent for a chat turn is the send itself: the reviewed
    /// context is the user's own words plus the attachments they curated, and
    /// the reply can only be text.
    fn authorize(
        &self,
        input: AuthorizeChatTurn<'_>,
    ) -> Result<AuthorizedModelRequest, ChatCoordinatorError> {
        let consent = ConsentService::new(&self.ledger);
        let decision = consent
            .approve(ApproveDecisionInput {
                manifest: input.manifest,
                binding: input.binding,
                decision_id: input.decision_id,
                issued_at: input.now,
                expires_at: UnixMillis(
                    input
                        .now
                        .0
                        .checked_add(CHAT_DECISION_WINDOW_MS)
                        .map_or(input.manifest.draft.expires_at.0, |window| {
                            window.min(input.manifest.draft.expires_at.0)
                        }),
                ),
            })
            .map_err(|_| ChatCoordinatorError::Integrity)?;
        let consumption = consent
            .consume(ConsumeDecisionInput {
                decision: &decision,
                binding: input.binding,
                invocation_id: input.invocation_id,
                consumed_at: input.now,
            })
            .map_err(|_| ChatCoordinatorError::Integrity)?;
        AuthorizedModelRequest::new(input.manifest, input.binding, consumption)
            .map_err(|_| ChatCoordinatorError::Integrity)
    }

    /// Verifies the reply and completes the pending turn.
    ///
    /// # Errors
    ///
    /// Returns [`ChatCoordinatorError::Conflict`] when this dispatch was
    /// superseded or invalidated while the request was in flight, and
    /// [`ChatCoordinatorError::OutputRejected`] when the reply is not one
    /// bounded chat turn.
    pub(crate) fn finalize_turn(
        &mut self,
        dispatch_id: u64,
        raw_output: &RawModelOutput,
        at: UnixMillis,
        store: &LocalStore,
    ) -> Result<TurnOutcome, ChatCoordinatorError> {
        let Some(mut in_flight) = self.in_flight.remove(&dispatch_id) else {
            return Err(ChatCoordinatorError::Conflict);
        };
        // The send resolved, so it happened: write it to the ledger.
        let entry = in_flight.sent.clone();
        in_flight.grant.record_call(entry);

        // Everything below can fail, and `in_flight` is already out of `self`.
        // Dropping it on an error would leave the transcript mid-turn with
        // nothing able to clean it up — `abort_turn` sees `self.in_flight` as
        // `None` and does nothing, so the stranded question makes every later
        // send in this conversation fail `TurnOutOfOrder`, permanently. One
        // malformed model reply would brick the conversation until restart.
        // So the fallible work is isolated, and the error path below is the
        // single place that decides what happens to the open turn.
        let outcome = self.complete_dispatch(&in_flight, raw_output, at, store);
        match outcome {
            Ok(TurnOutcome::ToolRequested(invocation)) => {
                self.in_flight.insert(dispatch_id, in_flight);
                Ok(TurnOutcome::ToolRequested(invocation))
            }
            Ok(completed) => Ok(completed),
            Err(error) => {
                if let Some(session) = self.sessions.get_mut(&in_flight.session_id) {
                    session.rollback_pending_user();
                }
                Err(error)
            }
        }
    }

    /// The fallible half of [`Self::finalize_turn`]. Never touches
    /// `self.in_flight`; its caller owns that decision.
    fn complete_dispatch(
        &mut self,
        in_flight: &InFlightTurn,
        raw_output: &RawModelOutput,
        at: UnixMillis,
        store: &LocalStore,
    ) -> Result<TurnOutcome, ChatCoordinatorError> {
        // Verified against the schema this turn was actually sealed under, so
        // a reply can never claim a shape the turn did not ask for.
        let schema = in_flight.pending.output_schema;
        let turn = match parse_chat_reply(&raw_output.payload_json, schema)? {
            // A tool request keeps the turn open. The transcript records the
            // ask, and the caller runs it and resumes.
            ChatReply::ToolCall(invocation) if schema.tools_granted() => {
                let session = self
                    .sessions
                    .get_mut(&in_flight.session_id)
                    .ok_or(ChatCoordinatorError::Unauthorized)?;
                session
                    .append_tool_call(&invocation, at)
                    .map_err(|_| ChatCoordinatorError::OutputRejected)?;
                return Ok(TurnOutcome::ToolRequested(invocation));
            }
            // Unreachable: the parser refuses tool fields under the text-only
            // schema. Kept as a second refusal rather than a plausible-looking
            // fallthrough, because this is the arm where a parser regression
            // would otherwise become a tool run nobody granted.
            ChatReply::ToolCall(_) => return Err(ChatCoordinatorError::OutputRejected),
            ChatReply::Text(turn) => turn,
        };
        let session = self
            .sessions
            .get_mut(&in_flight.session_id)
            .ok_or(ChatCoordinatorError::Unauthorized)?;
        session
            .append_assistant(&turn, at)
            .map_err(|_| ChatCoordinatorError::OutputRejected)?;
        let messages = session.messages();
        let assistant_message = messages
            .last()
            .cloned()
            .ok_or(ChatCoordinatorError::Integrity)?;
        // The turn opens at the newest question. With tool exchanges in
        // between it is no longer "the message before last", and persisting a
        // fixed pair would silently drop every tool_call and tool_result — the
        // store's own grammar would then reject the next append.
        let turn_start = messages
            .iter()
            .rposition(|message| message.role.opens_turn())
            .ok_or(ChatCoordinatorError::Integrity)?;
        let user_message = messages
            .get(turn_start)
            .cloned()
            .ok_or(ChatCoordinatorError::Integrity)?;
        let turn_messages = messages
            .get(turn_start..)
            .ok_or(ChatCoordinatorError::Integrity)?
            .to_vec();
        // Persist the completed exchange so it survives a restart. A storage
        // failure must not silently drop a reply the user can already see, so
        // it fails the turn rather than being swallowed.
        //
        // One call, one transaction, for the whole turn. Appending message by
        // message meant a crash or a single failed append between commits left
        // a durable `user` or `tool_call` row with no turn behind it — which
        // the store's grammar then refuses to let any later turn follow, and
        // no delete exists to clear. All-or-nothing is what keeps a partial
        // turn from becoming a permanent one.
        let agent_code = session.agent_code().map(str::to_owned);
        let workspace_id = session.workspace_id().as_str().to_owned();
        let rows: Vec<ChatTurnMessage<'_>> = turn_messages
            .iter()
            .map(|message| ChatTurnMessage {
                role: message.role.as_str(),
                text: &message.text,
                created_at_ms: message.created_at.0,
            })
            .collect();
        store
            .append_chat_turn(ChatTurnAppend {
                session_id: in_flight.session_id.as_str(),
                workspace_id: &workspace_id,
                agent_code: agent_code.as_deref(),
                messages: &rows,
            })
            .map_err(|_| ChatCoordinatorError::Integrity)?;
        Ok(TurnOutcome::Completed(Box::new(ChatTurnProjection {
            session_id: in_flight.session_id.clone(),
            agent_code,
            user_message,
            assistant_message,
        })))
    }

    /// Withdraws the pending question after a transport or verification
    /// failure, so a failed send leaves the transcript as it was.
    pub(crate) fn abort_turn(&mut self, dispatch_id: u64) {
        let Some(mut in_flight) = self.in_flight.remove(&dispatch_id) else {
            return;
        };
        // The request was sent before it failed, so its context left the
        // machine. Discarding the turn must not discard the accounting — this
        // is the path that would otherwise turn every failed turn into
        // unrecorded egress.
        let entry = in_flight.sent.clone();
        in_flight.grant.record_call(entry);
        if let Some(session) = self.sessions.get_mut(&in_flight.session_id) {
            session.rollback_pending_user();
        }
    }

    /// Feeds a tool result back into the open turn and seals the next call.
    ///
    /// The result is workspace-derived and therefore attacker-influenced. It
    /// enters as an ordinary bounded transcript message carrying no authority,
    /// and the model is told — in the v2 instructions — to treat it as data.
    ///
    /// # Errors
    ///
    /// Returns [`ChatCoordinatorError::Conflict`] when this dispatch was
    /// superseded or no tool call is outstanding,
    /// [`ChatCoordinatorError::TurnBudgetSpent`] when the turn has spent its
    /// egress budget, and [`ChatCoordinatorError::TurnExpired`] when the
    /// turn's grant window closed while the tool ran.
    pub(crate) fn resume_turn(
        &mut self,
        dispatch_id: u64,
        tool_output: String,
        at: UnixMillis,
    ) -> Result<ChatDispatch, ChatCoordinatorError> {
        let Some(in_flight) = self.in_flight.remove(&dispatch_id) else {
            return Err(ChatCoordinatorError::Conflict);
        };
        let session_id = in_flight.session_id.clone();
        // Same hazard as `finalize_turn`: `in_flight` is already out of `self`,
        // so a bare `?` here would strand a `tool_call` awaiting a result that
        // can never arrive, blocking the conversation permanently.
        let appended = self
            .sessions
            .get_mut(&session_id)
            .ok_or(ChatCoordinatorError::Unauthorized)
            .and_then(|session| {
                session
                    .append_tool_result(tool_output, at)
                    .map_err(|_| ChatCoordinatorError::OutputRejected)
            });
        if let Err(error) = appended {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.rollback_pending_user();
            }
            return Err(error);
        }

        // Re-seal: the transcript has grown, so this call needs its own
        // manifest and its own single-use decision. Reusing the previous one
        // would consent to context that is no longer what is being sent.
        let InFlightTurn { pending, grant, .. } = in_flight;
        let next = BeginChatTurn {
            installation_id: &pending.installation_id,
            project_id: &pending.project_id,
            workspace_id: &pending.workspace_id,
            client_session_ref: &pending.client_session_ref,
            agent_code: None,
            persona_instruction: pending.persona_instruction.as_deref(),
            text: String::new(),
            attachments: pending.attachments.clone(),
            configuration: &pending.configuration,
            output_schema: pending.output_schema,
            now: at,
        };
        let dispatch = self.seal_turn(&session_id, &next, Some(grant));
        if dispatch.is_err() {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.rollback_pending_user();
            }
        }
        dispatch
    }
}

/// Carries a turn's egress grant forward, or opens one for a new turn, and
/// checks that one more model call is inside it.
///
/// Minting a fresh grant per loop iteration would reset the budget every pass
/// and make the per-turn ceiling meaningless, so a carried grant is reused
/// rather than replaced. The check happens here, before the call is sealed, so
/// an over-budget iteration never reaches the provider.
fn turn_grant_for(
    session_id: &ContractId,
    carried: Option<TurnEgressGrant>,
    now: UnixMillis,
) -> Result<TurnEgressGrant, ChatCoordinatorError> {
    let grant = match carried {
        Some(existing) => existing,
        None => TurnEgressGrant::open(
            session_id.clone(),
            now,
            UnixMillis(now.0.saturating_add(CHAT_DECISION_WINDOW_MS)),
            MAX_TURN_MODEL_CALLS,
        )
        .map_err(|_| ChatCoordinatorError::Integrity)?,
    };
    // The two ways a grant refuses are two different things to tell the user,
    // and they were both reported as `Conflict` — which the renderer renders as
    // "A reply is already in flight for this conversation." For an elapsed
    // window that is simply untrue: nothing is in flight, and the advice it
    // implies (wait) is the one thing that does not help.
    grant.authorize_call(now).map_err(|error| match error {
        EgressError::TurnGrantExpired => ChatCoordinatorError::TurnExpired,
        EgressError::TurnGrantExhausted => ChatCoordinatorError::TurnBudgetSpent,
        // `authorize_call` returns nothing else. If it ever does, a genuine
        // conflict is the conservative reading: the turn does not continue.
        _ => ChatCoordinatorError::Conflict,
    })?;
    Ok(grant)
}

/// Derives the stable conversation identity for one renderer task. The host
/// owns the identity; the renderer only supplies its own opaque task
/// reference, so two tasks can never share a transcript.
fn chat_session_id(
    installation_id: &ContractId,
    workspace_id: &ContractId,
    client_session_ref: &str,
) -> Result<ContractId, ChatCoordinatorError> {
    if client_session_ref.is_empty() || client_session_ref.len() > 128 {
        return Err(ChatCoordinatorError::Unauthorized);
    }
    let digest = canonical_hash(
        "chat-session-id",
        1,
        &ChatSessionIdentity {
            schema_version: "sapphirus.chat-session.v1",
            installation_id,
            workspace_id,
            client_session_ref,
        },
    )
    .map_err(|_| ChatCoordinatorError::Integrity)?;
    derived_contract_id("chat", digest).map_err(|_| ChatCoordinatorError::Integrity)
}

/// Seals one chat turn's reviewed context into a manifest. The purpose label
/// is the chat lane's own, so this manifest can never satisfy a capability
/// binding and a capability manifest can never satisfy this one.
///
/// The declared output schema comes from the turn, not from a literal here.
/// The identifier and its hash are both derived from the one `schema_id`
/// binding below, so the pair sealed into the manifest — and copied verbatim
/// into the invocation binding — cannot drift apart.
fn seal_chat_manifest(
    input: &BeginChatTurn<'_>,
    candidates: Vec<ContextCandidate>,
) -> Result<desktop_egress::ContextEgressManifest, ChatCoordinatorError> {
    let configuration = input.configuration;
    let schema_id = input.output_schema.schema_id();
    let output_schema = ContractId::new(schema_id).map_err(|_| ChatCoordinatorError::Integrity)?;
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: input.installation_id.clone(),
            project_ref: input.project_id.clone(),
            run_ref: input.workspace_id.clone(),
            purpose: CHAT_PURPOSE.to_owned(),
            model_role: "chat".to_owned(),
            canonical_output_schema_id: output_schema,
            canonical_output_schema_hash: sha256_bytes(schema_id.as_bytes()),
            provider_profile_hash: configuration.provider_profile_hash,
            model_profile_hash: configuration.trusted_profile.model_binding_hash(),
            deployment_hash: configuration.deployment_hash,
            policy_hash: configuration.policy_hash,
            region: configuration.region.clone(),
            retention_mode: configuration.retention_mode,
            created_at: input.now,
            expires_at: UnixMillis(
                input
                    .now
                    .0
                    .checked_add(CHAT_MANIFEST_LIFETIME_MS)
                    .ok_or(ChatCoordinatorError::Integrity)?,
            ),
            limits: EgressLimits {
                maximum_context_items: MAX_CHAT_CONTEXT_ITEMS,
                maximum_context_bytes: MAX_CHAT_CONTEXT_BYTES,
                maximum_token_estimate: MAX_CHAT_CONTEXT_BYTES,
            },
            candidates,
            exclusions: Vec::new(),
        })
        .map_err(|_| ChatCoordinatorError::Integrity)
}

fn persona_candidate(instruction: &[u8]) -> Result<ContextCandidate, ChatCoordinatorError> {
    let content = std::str::from_utf8(instruction)
        .map_err(|_| ChatCoordinatorError::Integrity)?
        .to_owned();
    Ok(ContextCandidate {
        client_item_id: ContractId::new("chatctx_persona")
            .map_err(|_| ChatCoordinatorError::Integrity)?,
        relative_label: RelativeWorkspacePath::new("review/chat/agent-persona.md")
            .map_err(|_| ChatCoordinatorError::Integrity)?,
        semantic_role: "agent_persona".to_owned(),
        language: Some("markdown".to_owned()),
        classification: ContextClassification::Internal,
        content,
    })
}

/// The conversation so far, as exactly one reviewed context item.
///
/// One item rather than one per message, for two reasons. The manifest has a
/// hard 128-item ceiling, and a per-message shape makes the item count grow
/// with the conversation until sending fails outright. And a consent surface
/// listing two hundred rows discloses less than one row saying what it is: the
/// user is being asked about *the conversation*, which is one thing.
///
/// Returns `None` for an empty transcript so a first turn seals no empty item.
fn transcript_candidate(
    messages: &[ChatMessage],
) -> Result<Option<ContextCandidate>, ChatCoordinatorError> {
    if messages.is_empty() {
        return Ok(None);
    }
    let mut rendered = String::new();
    for message in messages {
        // One source of truth for the label. A duplicate mapping here would
        // eventually disagree with the store's, and a `tool_result` silently
        // rendered as `assistant` would present attacker-influenced workspace
        // content to the model as though Forge had said it.
        let role = message.role.as_str();
        // Role-tagged, oldest first, so attribution survives the flattening the
        // provider does downstream.
        let _ = write!(
            rendered,
            "## {role}

{}

",
            message.text
        );
    }
    Ok(Some(ContextCandidate {
        client_item_id: ContractId::new("chatctx_transcript")
            .map_err(|_| ChatCoordinatorError::Integrity)?,
        relative_label: RelativeWorkspacePath::new("review/chat/conversation.md")
            .map_err(|_| ChatCoordinatorError::Integrity)?,
        semantic_role: "chat_transcript".to_owned(),
        language: Some("markdown".to_owned()),
        classification: ContextClassification::Internal,
        content: rendered,
    }))
}

/// Parses one untrusted reply under the schema the turn was sealed with.
/// Exact keys only: an unexpected field is rejected, never ignored.
///
/// The reply must declare *that* schema, not merely a schema this host knows.
/// A turn sealed text-only was prompted with the v1 promise that the model
/// cannot read anything, so a v2-labelled reply to it is a reply to a question
/// nobody asked, and fails closed.
///
/// The tool-bearing schema widens *what may be asked for*, not what is
/// permitted. Authority-bearing fields — `apply`, `changes`, and anything else
/// not named here — remain refused under both schemas, because the mechanism
/// that stops a model granting itself authority is the exact-key check, and
/// widening the shape must not weaken it.
fn parse_chat_reply(
    payload_json: &str,
    sealed: ChatOutputSchema,
) -> Result<ChatReply, ChatCoordinatorError> {
    let value: Value = deserialize_strict_json(payload_json.as_bytes())
        .map_err(|_| ChatCoordinatorError::OutputRejected)?;
    let object = value
        .as_object()
        .ok_or(ChatCoordinatorError::OutputRejected)?;
    let schema = object
        .get("schemaVersion")
        .and_then(Value::as_str)
        .ok_or(ChatCoordinatorError::OutputRejected)?;

    if schema != sealed.schema_id() {
        return Err(ChatCoordinatorError::OutputRejected);
    }
    let tool_bearing = sealed.tools_granted();

    for key in object.keys() {
        let permitted = key == "schemaVersion"
            || key == "text"
            || (tool_bearing && (key == "tool" || key == "subject"));
        if !permitted {
            return Err(ChatCoordinatorError::OutputRejected);
        }
    }

    let has_text = object.contains_key("text");
    let has_tool = object.contains_key("tool") || object.contains_key("subject");
    // Exactly one of the two shapes, never both and never neither.
    if has_text == has_tool {
        return Err(ChatCoordinatorError::OutputRejected);
    }

    if has_tool {
        let tool = object
            .get("tool")
            .and_then(Value::as_str)
            .ok_or(ChatCoordinatorError::OutputRejected)?;
        let subject = object
            .get("subject")
            .and_then(Value::as_str)
            .ok_or(ChatCoordinatorError::OutputRejected)?;
        return ToolInvocation::new(tool.to_owned(), subject.to_owned())
            .map(ChatReply::ToolCall)
            .map_err(|_| ChatCoordinatorError::OutputRejected);
    }

    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ChatCoordinatorError::OutputRejected)?;
    ChatTurn::new(text.to_owned())
        .map(ChatReply::Text)
        .map_err(|_| ChatCoordinatorError::OutputRejected)
}

/// The offline stand-in reply. It must declare the schema this turn was sealed
/// under, because it is verified by the same parser as a real reply — a fixture
/// labelled v1 for a v2 turn is rejected, and the deterministic build stops
/// answering at all.
fn deterministic_chat_fixture(schema: ChatOutputSchema) -> String {
    serde_json::json!({
        "schemaVersion": schema.schema_id(),
        "text": "This reply came from the deterministic local composition; no external model was contacted.",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        deterministic_chat_fixture, parse_chat_reply, persona_candidate, seal_chat_manifest,
        transcript_candidate, BeginChatTurn, ChatCoordinatorError, ChatOutputSchema, ChatReply,
        CHAT_DECISION_WINDOW_MS, CHAT_MANIFEST_LIFETIME_MS, CHAT_PURPOSE, CHAT_TOOL_TURN_SCHEMA,
        MAX_CHAT_CONTEXT_BYTES, MAX_CHAT_CONTEXT_ITEMS,
    };
    use desktop_egress::{
        ApproveDecisionInput, ConsentService, ContextCandidate, ContextClassification,
        ContextPreparer, EgressLimits, MemoryDecisionLedger, ModelInvocationBindingDraft,
        PatternSecretScanner, PrepareContextInput, RetentionMode,
    };
    use desktop_runtime::{ChatSession, ChatTurn, MAX_PROMPT_TRANSCRIPT_BYTES};
    use desktop_runtime::{
        ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis, CHAT_TURN_SCHEMA,
    };
    use desktop_store::KeyProtector;

    use super::{ChatCoordinator, ChatTurnAppend, ChatTurnMessage, LocalStore};

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("identifier")
    }

    fn digest(seed: u8) -> Sha256Digest {
        Sha256Digest::from_bytes([seed; 32])
    }

    /// Parses under the text-only schema — what a turn with no tool grant is
    /// sealed with, and therefore what its reply is checked against.
    fn parse_chat_turn(payload_json: &str) -> Result<ChatTurn, ChatCoordinatorError> {
        match parse_chat_reply(payload_json, ChatOutputSchema::TextOnly)? {
            ChatReply::Text(turn) => Ok(turn),
            ChatReply::ToolCall(_) => Err(ChatCoordinatorError::OutputRejected),
        }
    }

    fn candidate(index: usize) -> ContextCandidate {
        ContextCandidate {
            client_item_id: id(&format!("chatctx_{index:03}")),
            relative_label: RelativeWorkspacePath::new(format!("src/file-{index}.rs"))
                .expect("path"),
            semantic_role: "workspace_file".to_owned(),
            language: Some("rust".to_owned()),
            classification: ContextClassification::Internal,
            content: format!("// line {index}\n"),
        }
    }

    /// Seals a manifest with exactly the limits and lifetime `seal_chat_manifest`
    /// uses, so the egress hard caps are exercised rather than restated.
    fn seal_with_chat_limits(
        now: UnixMillis,
        candidates: Vec<ContextCandidate>,
    ) -> Result<desktop_egress::ContextEgressManifest, desktop_egress::EgressError> {
        ContextPreparer::new(PatternSecretScanner).prepare(PrepareContextInput {
            tenant_ref: id("install_chat"),
            project_ref: id("project_chat"),
            run_ref: id("ws_chat"),
            purpose: CHAT_PURPOSE.to_owned(),
            model_role: "assistant".to_owned(),
            canonical_output_schema_id: id(CHAT_TURN_SCHEMA),
            canonical_output_schema_hash: digest(1),
            provider_profile_hash: digest(2),
            model_profile_hash: digest(3),
            deployment_hash: digest(4),
            policy_hash: digest(5),
            region: "local".to_owned(),
            retention_mode: RetentionMode::TransientNoStore,
            created_at: now,
            expires_at: UnixMillis(now.0 + CHAT_MANIFEST_LIFETIME_MS),
            limits: EgressLimits {
                maximum_context_items: MAX_CHAT_CONTEXT_ITEMS,
                maximum_context_bytes: MAX_CHAT_CONTEXT_BYTES,
                maximum_token_estimate: MAX_CHAT_CONTEXT_BYTES,
            },
            candidates,
            exclusions: Vec::new(),
        })
    }

    /// Regression guard: the chat manifest declared 320 context items against
    /// the egress hard cap of 128, so *every* `chat.send` failed to seal and
    /// the app's default lane was inert. Nothing exercised this path before.
    #[test]
    fn a_chat_manifest_seals_under_the_egress_hard_caps() {
        let manifest = seal_with_chat_limits(UnixMillis(1_000), vec![candidate(0)])
            .expect("chat limits must satisfy the egress hard caps");
        assert_eq!(manifest.draft.purpose, CHAT_PURPOSE);
        assert_eq!(
            manifest.draft.limits.maximum_context_items,
            MAX_CHAT_CONTEXT_ITEMS
        );
    }

    /// The 100 attachments the IPC envelope admits, plus the persona, must all
    /// fit — the declared ceiling is only honest if the full load seals.
    #[test]
    fn a_full_chat_context_load_seals() {
        let candidates = (0..MAX_CHAT_CONTEXT_ITEMS as usize)
            .map(candidate)
            .collect();
        seal_with_chat_limits(UnixMillis(1_000), candidates)
            .expect("a full chat context load must seal");
    }

    /// Regression guard: the decision inherited the manifest's 10-minute
    /// expiry, but `desktop-egress` caps a single-use decision at 5 minutes, so
    /// approval failed with `InvalidLifetime` even once the limits were fixed.
    #[test]
    fn the_chat_decision_window_stays_inside_the_consent_cap() {
        let now = UnixMillis(1_000);
        let manifest = seal_with_chat_limits(now, vec![candidate(0)]).expect("manifest");
        let binding = ModelInvocationBindingDraft {
            schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
            request_id: id("req_chat_001"),
            tenant_ref: manifest.draft.tenant_ref.clone(),
            project_ref: manifest.draft.project_ref.clone(),
            run_ref: manifest.draft.run_ref.clone(),
            installation_id: id("install_chat"),
            session_authority_hash: digest(9),
            manifest_hash: manifest.manifest_hash,
            purpose: CHAT_PURPOSE.to_owned(),
            model_role: manifest.draft.model_role.clone(),
            canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
            provider_profile_hash: manifest.draft.provider_profile_hash,
            model_profile_hash: manifest.draft.model_profile_hash,
            deployment_hash: manifest.draft.deployment_hash,
            policy_hash: manifest.draft.policy_hash,
            region: manifest.draft.region.clone(),
            retention_mode: manifest.draft.retention_mode,
            consent_disclosure_hash: digest(10),
        }
        .seal()
        .expect("binding");

        // The clamp `seal`/`authorize` applies: never past the consent cap.
        let expires_at = UnixMillis(
            now.0
                .checked_add(CHAT_DECISION_WINDOW_MS)
                .map_or(manifest.draft.expires_at.0, |window| {
                    window.min(manifest.draft.expires_at.0)
                }),
        );
        assert!(
            expires_at.0 < manifest.draft.expires_at.0,
            "the manifest deliberately outlives the decision minted from it"
        );

        let ledger = MemoryDecisionLedger::default();
        ConsentService::new(&ledger)
            .approve(ApproveDecisionInput {
                manifest: &manifest,
                binding: &binding,
                decision_id: id("decision_chat_001"),
                issued_at: now,
                expires_at,
            })
            .expect("the clamped chat decision must satisfy the consent lifetime cap");
    }

    /// Regression guard, and the only thing that made the agent tool loop
    /// reachable at all: `seal_chat_manifest` hardcoded the v1 schema id, so
    /// every live turn was sealed text-only. The provider picks the system
    /// prompt from that field alone, so the model was always sent the v1
    /// instructions — which end *"you cannot read, write, or execute
    /// anything"* — and was never told a tool existed. `CHAT_TOOL_TURN_SCHEMA`
    /// parsed fine and no code path could ever seal it.
    #[test]
    fn the_sealed_schema_follows_the_turn_s_tool_grant() {
        let configuration = crate::bmad_model::config::offline_frontend_model_composition(
            desktop_runtime::ModelProviderId::OpenAi,
            "gpt-5-mini",
        )
        .expect("offline model composition")
        .configuration;
        let installation_id = id("install_chat");
        let project_id = id("project_chat");
        let workspace_id = id("ws_chat");

        let seal = |output_schema| {
            let input = BeginChatTurn {
                installation_id: &installation_id,
                project_id: &project_id,
                workspace_id: &workspace_id,
                client_session_ref: "task-1",
                agent_code: None,
                persona_instruction: None,
                text: String::new(),
                attachments: Vec::new(),
                configuration: &configuration,
                output_schema,
                now: UnixMillis(1_000),
            };
            seal_chat_manifest(&input, vec![candidate(0)]).expect("chat manifest")
        };

        let tool_granted = seal(ChatOutputSchema::ToolBearing);
        assert_eq!(
            tool_granted.draft.canonical_output_schema_id.as_str(),
            CHAT_TOOL_TURN_SCHEMA,
            "a tool-granted turn must seal the tool-bearing schema, or the model is never told tools exist",
        );

        let text_only = seal(ChatOutputSchema::TextOnly);
        assert_eq!(
            text_only.draft.canonical_output_schema_id.as_str(),
            CHAT_TURN_SCHEMA,
            "a turn with no tool grant must still seal the text-only schema",
        );

        // The id and the hash are both sealed into the manifest and copied into
        // the invocation binding. Disagreement there is an integrity failure at
        // verification time, so the pair is asserted, not just the id.
        for (schema, manifest) in [
            (CHAT_TOOL_TURN_SCHEMA, &tool_granted),
            (CHAT_TURN_SCHEMA, &text_only),
        ] {
            assert_eq!(
                manifest.draft.canonical_output_schema_hash,
                desktop_runtime::sha256_bytes(schema.as_bytes()),
                "the sealed schema hash disagrees with the sealed schema id for {schema}",
            );
        }
        assert_ne!(
            tool_granted.draft.canonical_output_schema_hash,
            text_only.draft.canonical_output_schema_hash,
        );

        // The offline stand-in reply is verified by the same parser as a real
        // one, so it has to speak whichever schema the turn was sealed under.
        for schema in [ChatOutputSchema::ToolBearing, ChatOutputSchema::TextOnly] {
            parse_chat_reply(&deterministic_chat_fixture(schema), schema)
                .expect("the deterministic fixture must satisfy its own turn's schema");
        }
    }

    #[test]
    fn a_bounded_reply_is_accepted() {
        let payload = format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hello."}}"#);
        assert_eq!(parse_chat_turn(&payload).expect("chat turn").text, "Hello.");
    }

    #[test]
    fn replies_outside_the_closed_shape_fail_closed() {
        let digest = format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hi"}}"#);
        assert!(parse_chat_turn(&digest).is_ok());

        for rejected in [
            // Wrong or missing schema version.
            r#"{"schemaVersion":"sapphirus.bmad-document-artifact.v1","text":"Hi"}"#.to_owned(),
            r#"{"text":"Hi"}"#.to_owned(),
            // Smuggled authority-bearing fields.
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hi","apply":true}}"#),
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hi","changes":[]}}"#),
            // Duplicate decoded keys.
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hi","text":"Bye"}}"#),
            // Wrong types and unbounded text.
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":7}}"#),
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":""}}"#),
            // Not an object at all.
            r#"["hi"]"#.to_owned(),
            // A tool call under the text-only schema: tools were never granted
            // for this turn, so asking for one is out of shape.
            format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","tool":"read","subject":"a.md"}}"#),
        ] {
            assert_eq!(
                parse_chat_turn(&rejected).err(),
                Some(ChatCoordinatorError::OutputRejected),
                "accepted a reply outside the closed shape: {rejected}"
            );
        }
    }

    #[test]
    fn a_rejected_reply_does_not_brick_the_conversation() {
        // A malformed model reply is ordinary, not exotic. Before this was
        // fixed, `finalize_turn` dropped the in-flight turn without restoring
        // it, so `abort_turn` became a no-op and the unanswered question stayed
        // in the transcript — making every later send in that conversation fail
        // `TurnOutOfOrder` for the life of the process.
        let mut session = ChatSession::open(
            ContractId::new("chat_reject").expect("session id"),
            ContractId::new("ws_reject").expect("workspace id"),
        );
        session
            .append_user("first question".to_owned(), UnixMillis(1_000))
            .expect("user message");
        assert!(session.has_pending_user_message());

        // What the error path must do: leave the transcript as it was.
        session.rollback_pending_user();
        assert!(!session.has_pending_user_message());

        // The next send therefore succeeds instead of failing forever.
        session
            .append_user("second question".to_owned(), UnixMillis(2_000))
            .expect("a later send must not be blocked by the failed one");
    }

    #[test]
    fn the_tool_schema_widens_the_shape_without_widening_authority() {
        // The point of the widening: a tool *request* now parses.
        let call = format!(
            r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"grep","subject":"src/**"}}"#
        );
        let invocation =
            match parse_chat_reply(&call, ChatOutputSchema::ToolBearing).expect("tool call") {
                ChatReply::ToolCall(invocation) => Some(invocation),
                ChatReply::Text(_) => None,
            }
            .expect("expected a tool call, not prose");
        assert_eq!(invocation.tool, "grep");
        assert_eq!(invocation.subject, "src/**");

        // Prose still parses under the same schema.
        let prose = format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","text":"Hi"}}"#);
        assert!(matches!(
            parse_chat_reply(&prose, ChatOutputSchema::ToolBearing),
            Ok(ChatReply::Text(_))
        ));

        for rejected in [
            // The authority-bearing fields stay refused under the *new* schema
            // too. This is the assertion the original test existed for.
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","text":"Hi","apply":true}}"#),
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","text":"Hi","changes":[]}}"#),
            format!(
                r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"read","subject":"a","apply":true}}"#
            ),
            // Narrating one thing while asking for another.
            format!(
                r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","text":"Hi","tool":"read","subject":"a"}}"#
            ),
            // Half a tool call is not a tool call.
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"read"}}"#),
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","subject":"a.md"}}"#),
            // Neither shape.
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}"}}"#),
            // Bounds still apply to both fields.
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"","subject":"a"}}"#),
            format!(r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"read","subject":""}}"#),
            format!(
                r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"read","subject":{}}}"#,
                serde_json::Value::String("s".repeat(1_025))
            ),
        ] {
            assert_eq!(
                parse_chat_reply(&rejected, ChatOutputSchema::ToolBearing).err(),
                Some(ChatCoordinatorError::OutputRejected),
                "accepted a reply outside the closed shape: {rejected}"
            );
        }

        // A reply may only speak the schema its own turn was sealed with.
        // Otherwise a text-only turn — whose prompt promised the model it could
        // read nothing — could still hand the host a tool call to run.
        let call = format!(
            r#"{{"schemaVersion":"{CHAT_TOOL_TURN_SCHEMA}","tool":"read","subject":"a.md"}}"#
        );
        assert_eq!(
            parse_chat_reply(&call, ChatOutputSchema::TextOnly).err(),
            Some(ChatCoordinatorError::OutputRejected),
            "a text-only turn accepted a tool-bearing reply"
        );
        assert_eq!(
            parse_chat_reply(
                &format!(r#"{{"schemaVersion":"{CHAT_TURN_SCHEMA}","text":"Hi"}}"#),
                ChatOutputSchema::ToolBearing,
            )
            .err(),
            Some(ChatCoordinatorError::OutputRejected),
            "a tool-granted turn accepted a reply from the other schema"
        );
    }

    #[test]
    fn the_chat_purpose_is_distinct_from_every_capability_purpose() {
        // Capability purposes are `bmm.*` or `builder.*` (the closure id with
        // its first colon replaced). A chat manifest can therefore never be
        // replayed against a capability, or the reverse.
        assert_eq!(CHAT_PURPOSE, "chat.turn");
        assert!(!CHAT_PURPOSE.starts_with("bmm."));
        assert!(!CHAT_PURPOSE.starts_with("builder."));
    }

    /// Regression: the ceiling was sized for "100 attachments plus the persona"
    /// while the same function added *one item per transcript message*, bounded
    /// only by bytes while retention kept 200 messages. A long conversation
    /// therefore crossed the 128-item hard cap and every later send failed to
    /// seal — permanently, with no recovery but starting a new conversation.
    ///
    /// This is the second time this lane shipped a ceiling its own inputs could
    /// exceed, so the assertion is on the arithmetic, not on a sample.
    #[test]
    fn a_long_conversation_with_a_full_attachment_load_still_seals() {
        let mut session = ChatSession::open(id("chatsess_01"), id("ws_chat"));
        // Well past the 200-message retention ceiling, so the transcript is as
        // long as this lane can ever make it.
        for index in 0..130_u64 {
            session
                .append_user(format!("user turn {index}"), UnixMillis(2_000 + index * 10))
                .expect("user");
            let turn = ChatTurn::new(format!("assistant turn {index}")).expect("turn");
            session
                .append_assistant(&turn, UnixMillis(2_005 + index * 10))
                .expect("assistant");
        }
        let transcript = transcript_candidate(session.prompt_window(MAX_PROMPT_TRANSCRIPT_BYTES))
            .expect("transcript")
            .expect("a non-empty conversation yields one item");

        let mut candidates = vec![persona_candidate(b"Be an analyst.").expect("persona")];
        candidates.push(transcript);
        // The most attachments the IPC envelope admits.
        for index in 0..100 {
            candidates.push(candidate(index));
        }
        assert_eq!(
            candidates.len(),
            MAX_CHAT_CONTEXT_ITEMS as usize,
            "persona + one transcript + 100 attachments is the declared ceiling",
        );
        seal_with_chat_limits(UnixMillis(1_000), candidates)
            .expect("the heaviest load this lane can assemble must seal");
    }

    /// The transcript is one item however long it gets — that is the property
    /// which keeps the item count independent of conversation length.
    #[test]
    fn the_transcript_is_always_one_item_and_keeps_attribution() {
        let mut session = ChatSession::open(id("chatsess_02"), id("ws_chat"));
        session
            .append_user("first question".to_owned(), UnixMillis(2_000))
            .expect("user");
        let turn = ChatTurn::new("first answer".to_owned()).expect("turn");
        session
            .append_assistant(&turn, UnixMillis(2_100))
            .expect("assistant");
        session
            .append_user("second question".to_owned(), UnixMillis(2_200))
            .expect("user");

        let candidate = transcript_candidate(session.prompt_window(MAX_PROMPT_TRANSCRIPT_BYTES))
            .expect("transcript")
            .expect("one item");
        assert_eq!(candidate.semantic_role, "chat_transcript");
        // Role attribution survives the flattening the provider does later.
        assert!(candidate.content.contains(
            "## user

first question"
        ));
        assert!(candidate.content.contains(
            "## assistant

first answer"
        ));
        let user_turns = candidate.content.matches("## user").count();
        assert_eq!(user_turns, 2);
    }

    /// A first turn has no history, and an empty item would be a sealed,
    /// consented, meaningless byte range.
    #[test]
    fn an_empty_transcript_yields_no_item() {
        assert!(transcript_candidate(&[]).expect("transcript").is_none());
    }

    #[derive(Debug)]
    struct TestProtector;

    impl KeyProtector for TestProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, desktop_store::StoreError> {
            Ok(plaintext.to_vec())
        }

        fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, desktop_store::StoreError> {
            Ok(protected.to_vec())
        }
    }

    fn chat_store(directory: &tempfile::TempDir) -> LocalStore {
        LocalStore::open(directory.path(), &TestProtector).expect("store")
    }

    fn conversation() -> (ContractId, ContractId, ContractId) {
        let installation = id("install_chat");
        let workspace = id("ws_chat");
        let session_id =
            super::chat_session_id(&installation, &workspace, "ref_1").expect("session id");
        (installation, workspace, session_id)
    }

    /// Regression guard for the wedge. An older build persisted a turn one
    /// message per transaction, so a crash between commits left a durable
    /// `user` row with no turn behind it. `rollback_pending_user` hid it from
    /// the user but only in memory, and the store's grammar then refused every
    /// later turn — permanently, with no delete anywhere able to clear it.
    #[test]
    fn a_partial_turn_left_by_an_older_build_is_repaired_on_load() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = chat_store(&directory);
        let (installation, workspace, session_id) = conversation();
        store
            .append_chat_message(
                session_id.as_str(),
                workspace.as_str(),
                None,
                "user",
                "the question whose turn never closed",
                1_000,
            )
            .expect("the wedged row an older build left behind");

        let mut coordinator = ChatCoordinator::new();
        let history = coordinator
            .history(&installation, &workspace, "ref_1", &store)
            .expect("a repairable conversation must still load");
        assert!(history.messages.is_empty());

        // Repaired durably, not just in memory: the row the user could never
        // see is gone, and it was the only one.
        assert!(store
            .chat_session(session_id.as_str())
            .expect("read back")
            .is_none_or(|stored| stored.messages.is_empty()));

        // Which is the whole point — the next turn is no longer refused.
        store
            .append_chat_turn(ChatTurnAppend {
                session_id: session_id.as_str(),
                workspace_id: workspace.as_str(),
                agent_code: None,
                messages: &[
                    ChatTurnMessage {
                        role: "user",
                        text: "asked again",
                        created_at_ms: 2_000,
                    },
                    ChatTurnMessage {
                        role: "assistant",
                        text: "answered",
                        created_at_ms: 2_001,
                    },
                ],
            })
            .expect("a repaired conversation must accept a new turn");
    }

    /// The repair is only ever applied to a transcript that was understood in
    /// full. A completed turn behind the wreckage survives it.
    #[test]
    fn repair_on_load_keeps_every_completed_turn() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = chat_store(&directory);
        let (installation, workspace, session_id) = conversation();
        for (role, text, at) in [
            ("user", "first", 1_000_u64),
            ("assistant", "answer", 1_001),
            ("user", "second", 2_000),
            ("tool_call", "glob src/**", 2_001),
        ] {
            store
                .append_chat_message(
                    session_id.as_str(),
                    workspace.as_str(),
                    None,
                    role,
                    text,
                    at,
                )
                .expect("seed");
        }

        let mut coordinator = ChatCoordinator::new();
        let history = coordinator
            .history(&installation, &workspace, "ref_1", &store)
            .expect("history");
        let seen: Vec<(&str, &str)> = history
            .messages
            .iter()
            .map(|message| (message.role.as_str(), message.text.as_str()))
            .collect();
        assert_eq!(seen, vec![("user", "first"), ("assistant", "answer")]);

        let stored = store
            .chat_session(session_id.as_str())
            .expect("read back")
            .expect("session");
        let durable: Vec<(&str, &str)> = stored
            .messages
            .iter()
            .map(|message| (message.role.as_str(), message.text.as_str()))
            .collect();
        assert_eq!(durable, vec![("user", "first"), ("assistant", "answer")]);
    }

    /// A durable transcript that cannot be rebuilt is reported as a failure,
    /// not handed back as an empty conversation. Returning empty is what let
    /// the app open a *fresh* session over it and keep appending turns the user
    /// could never see.
    ///
    /// The two shapes below are what remains of the store/runtime disagreement
    /// after the clock is enforced: `chat_messages` bounds neither the message
    /// text nor a `tool_call`'s "<tool> <subject>" rendering, and
    /// `ChatMessage::new` / `ToolInvocation::new` refuse both. Closing either
    /// gap in the store is a fix, not a regression — this test would then need
    /// a row injected some other way.
    #[test]
    fn a_transcript_that_cannot_be_rebuilt_is_reported_rather_than_emptied() {
        for (label, tail) in [
            ("a message past the runtime's byte ceiling", None),
            ("a tool call with no subject", Some(("tool_call", "glob"))),
        ] {
            let directory = tempfile::tempdir().expect("tempdir");
            let store = chat_store(&directory);
            let (installation, workspace, session_id) = conversation();
            let oversized = "x".repeat(32_769);
            let opening = if tail.is_some() {
                "a question"
            } else {
                &oversized
            };
            store
                .append_chat_message(
                    session_id.as_str(),
                    workspace.as_str(),
                    None,
                    "user",
                    opening,
                    1_000,
                )
                .expect("the store admits it");
            if let Some((role, text)) = tail {
                store
                    .append_chat_message(
                        session_id.as_str(),
                        workspace.as_str(),
                        None,
                        role,
                        text,
                        1_001,
                    )
                    .expect("the store admits it");
            }

            let mut coordinator = ChatCoordinator::new();
            let outcome = coordinator.history(&installation, &workspace, "ref_1", &store);
            assert_eq!(
                outcome.err(),
                Some(ChatCoordinatorError::Integrity),
                "{label}: an unreadable transcript must not read as an empty one"
            );
            // And sending is refused too, so nothing is appended to a record
            // the user cannot see.
            assert!(
                coordinator.is_unreadable(&session_id),
                "{label}: the conversation must stay closed to new turns"
            );
            assert!(
                !coordinator.sessions.contains_key(&session_id),
                "{label}: no live session may stand in for the durable one"
            );
            // Nothing was deleted: a transcript that was never understood is
            // never repaired.
            let stored = store
                .chat_session(session_id.as_str())
                .expect("read back")
                .expect("session");
            assert_eq!(
                stored.messages.len(),
                if tail.is_some() { 2 } else { 1 },
                "{label}: an uninterpretable transcript must be left intact"
            );
        }
    }

    /// A conversation with nothing durable behind it is simply new.
    #[test]
    fn an_unwritten_conversation_loads_empty_without_error() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = chat_store(&directory);
        let (installation, workspace, session_id) = conversation();
        let mut coordinator = ChatCoordinator::new();
        let history = coordinator
            .history(&installation, &workspace, "ref_1", &store)
            .expect("a new conversation is not a failure");
        assert!(history.messages.is_empty());
        assert!(!coordinator.is_unreadable(&session_id));
    }

    /// A transcript belonging to someone else is a grant failure, not an empty
    /// conversation, and it is never repaired.
    #[test]
    fn another_workspace_s_transcript_is_refused() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = chat_store(&directory);
        let (installation, workspace, session_id) = conversation();
        store
            .append_chat_message(
                session_id.as_str(),
                "ws_someone_else",
                None,
                "user",
                "not yours",
                1_000,
            )
            .expect("seed");

        let mut coordinator = ChatCoordinator::new();
        let outcome = coordinator.history(&installation, &workspace, "ref_1", &store);
        assert_eq!(outcome.err(), Some(ChatCoordinatorError::Unauthorized));
        assert_eq!(
            store
                .chat_session(session_id.as_str())
                .expect("read back")
                .expect("session")
                .messages
                .len(),
            1
        );
    }
}
