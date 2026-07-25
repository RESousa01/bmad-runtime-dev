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

use std::collections::BTreeMap;

use desktop_cloud::{AuthorizedModelRequest, RawModelOutput};
use desktop_egress::{
    ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
    ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput,
};
use desktop_runtime::{
    canonical_hash, deserialize_strict_json, sha256_bytes, ChatMessage, ChatRole, ChatSession,
    ChatTurn, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis, CHAT_TURN_SCHEMA,
    MAX_PROMPT_TRANSCRIPT_BYTES,
};
use desktop_store::LocalStore;
use serde::Serialize;
use serde_json::Value;

use crate::bmad_model::config::{HelpModelConfiguration, HelpModelMode};
use crate::bmad_model::context::derived_contract_id;

/// Conversations retained per host process. Bounds unbounded growth across a
/// long session with many tasks; the oldest idle conversation is evicted.
const MAX_CHAT_SESSIONS: usize = 64;
const CHAT_MANIFEST_LIFETIME_MS: u64 = 10 * 60 * 1_000;
const MAX_CHAT_CONTEXT_ITEMS: u32 = 320;
const MAX_CHAT_CONTEXT_BYTES: u64 = 512 * 1024;
/// The egress purpose label for the chat lane. Distinct from every capability
/// purpose, so a chat manifest can never be replayed as a capability manifest.
const CHAT_PURPOSE: &str = "chat.turn";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChatCoordinatorError {
    Unauthorized,
    Conflict,
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
    pub now: UnixMillis,
}

struct AuthorizeChatTurn<'a> {
    manifest: &'a desktop_egress::ContextEgressManifest,
    binding: &'a desktop_egress::ModelInvocationBinding,
    decision_id: ContractId,
    invocation_id: ContractId,
    now: UnixMillis,
}

struct InFlightTurn {
    dispatch_id: u64,
    session_id: ContractId,
}

pub(crate) struct ChatCoordinator {
    ledger: MemoryDecisionLedger,
    sessions: BTreeMap<ContractId, ChatSession>,
    /// Insertion/most-recent-use order, oldest first, for eviction.
    order: Vec<ContractId>,
    in_flight: Option<InFlightTurn>,
    dispatch_seq: u64,
}

impl ChatCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            ledger: MemoryDecisionLedger::default(),
            sessions: BTreeMap::new(),
            order: Vec::new(),
            in_flight: None,
            dispatch_seq: 0,
        }
    }

    /// Drops every conversation, for sign-out and workspace revocation
    /// (ADR-0002). An in-flight reply is abandoned rather than delivered into
    /// a transcript the user is no longer authorized to see.
    pub(crate) fn invalidate(&mut self) {
        self.sessions.clear();
        self.order.clear();
        self.in_flight = None;
    }

    /// Loads one conversation, rehydrating it from the store when this process
    /// has not seen it yet. That is what makes a transcript survive a restart:
    /// the store is the record, this map is only a live cache.
    pub(crate) fn history(
        &mut self,
        installation_id: &ContractId,
        workspace_id: &ContractId,
        client_session_ref: &str,
        store: &LocalStore,
    ) -> Result<ChatHistoryProjection, ChatCoordinatorError> {
        let session_id = chat_session_id(installation_id, workspace_id, client_session_ref)?;
        if !self.sessions.contains_key(&session_id) {
            self.rehydrate(&session_id, workspace_id, store);
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
    fn rehydrate(
        &mut self,
        session_id: &ContractId,
        workspace_id: &ContractId,
        store: &LocalStore,
    ) {
        let Ok(Some(stored)) = store.chat_session(session_id.as_str()) else {
            return;
        };
        if stored.workspace_id != workspace_id.as_str() {
            return;
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
                _ => return,
            };
            if applied.is_err() {
                return;
            }
        }
        // A dangling question would block the next send; drop it on load.
        session.rollback_pending_user();
        self.sessions.insert(session_id.clone(), session);
        self.order.push(session_id.clone());
        self.evict_oldest(session_id);
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
        if self.in_flight.is_some() {
            return Err(ChatCoordinatorError::Conflict);
        }
        if input.configuration.mode == HelpModelMode::Offline {
            return Err(ChatCoordinatorError::Transport);
        }
        let session_id = chat_session_id(
            input.installation_id,
            input.workspace_id,
            input.client_session_ref,
        )?;
        {
            let session = self.session_mut(&session_id, input.workspace_id);
            if input.agent_code.is_some() {
                session.set_agent_code(input.agent_code.clone());
            }
            session
                .append_user(std::mem::take(&mut input.text), input.now)
                .map_err(|_| ChatCoordinatorError::OutputRejected)?;
        }

        let dispatch = self.seal_turn(&session_id, &input);
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
    ) -> Result<ChatDispatch, ChatCoordinatorError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ChatCoordinatorError::Integrity)?;
        let mut candidates = Vec::new();
        if let Some(persona) = input.persona_instruction {
            candidates.push(persona_candidate(persona)?);
        }
        candidates.extend(transcript_candidates(
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
        self.in_flight = Some(InFlightTurn {
            dispatch_id,
            session_id: session_id.clone(),
        });
        Ok(ChatDispatch {
            dispatch_id,
            authorized_request,
            deterministic_fixture: deterministic_chat_fixture(),
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
                expires_at: input.manifest.draft.expires_at,
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
    ) -> Result<ChatTurnProjection, ChatCoordinatorError> {
        let Some(in_flight) = self.in_flight.take() else {
            return Err(ChatCoordinatorError::Conflict);
        };
        if in_flight.dispatch_id != dispatch_id {
            self.in_flight = Some(in_flight);
            return Err(ChatCoordinatorError::Conflict);
        }
        let turn = parse_chat_turn(&raw_output.payload_json)?;
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
        let user_message = messages
            .get(messages.len().wrapping_sub(2))
            .cloned()
            .ok_or(ChatCoordinatorError::Integrity)?;
        // Persist the completed exchange so it survives a restart. A storage
        // failure must not silently drop a reply the user can already see, so
        // it fails the turn rather than being swallowed.
        let agent_code = session.agent_code().map(str::to_owned);
        let workspace_id = session.workspace_id().as_str().to_owned();
        for message in [&user_message, &assistant_message] {
            store
                .append_chat_message(
                    in_flight.session_id.as_str(),
                    &workspace_id,
                    agent_code.as_deref(),
                    message.role.as_str(),
                    &message.text,
                    message.created_at.0,
                )
                .map_err(|_| ChatCoordinatorError::Integrity)?;
        }
        Ok(ChatTurnProjection {
            session_id: in_flight.session_id,
            agent_code,
            user_message,
            assistant_message,
        })
    }

    /// Withdraws the pending question after a transport or verification
    /// failure, so a failed send leaves the transcript as it was.
    pub(crate) fn abort_turn(&mut self, dispatch_id: u64) {
        let Some(in_flight) = self.in_flight.take() else {
            return;
        };
        if in_flight.dispatch_id != dispatch_id {
            self.in_flight = Some(in_flight);
            return;
        }
        if let Some(session) = self.sessions.get_mut(&in_flight.session_id) {
            session.rollback_pending_user();
        }
    }
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
fn seal_chat_manifest(
    input: &BeginChatTurn<'_>,
    candidates: Vec<ContextCandidate>,
) -> Result<desktop_egress::ContextEgressManifest, ChatCoordinatorError> {
    let configuration = input.configuration;
    let output_schema =
        ContractId::new(CHAT_TURN_SCHEMA).map_err(|_| ChatCoordinatorError::Integrity)?;
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: input.installation_id.clone(),
            project_ref: input.project_id.clone(),
            run_ref: input.workspace_id.clone(),
            purpose: CHAT_PURPOSE.to_owned(),
            model_role: "chat".to_owned(),
            canonical_output_schema_id: output_schema,
            canonical_output_schema_hash: sha256_bytes(CHAT_TURN_SCHEMA.as_bytes()),
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

fn transcript_candidates(
    messages: &[ChatMessage],
) -> Result<Vec<ContextCandidate>, ChatCoordinatorError> {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            Ok(ContextCandidate {
                client_item_id: ContractId::new(format!("chatctx_{index:04}"))
                    .map_err(|_| ChatCoordinatorError::Integrity)?,
                relative_label: RelativeWorkspacePath::new(format!(
                    "review/chat/{index:04}-{}.md",
                    message.role.as_str()
                ))
                .map_err(|_| ChatCoordinatorError::Integrity)?,
                semantic_role: match message.role {
                    ChatRole::User => "chat_user_message",
                    ChatRole::Assistant => "chat_assistant_message",
                }
                .to_owned(),
                language: Some("markdown".to_owned()),
                classification: ContextClassification::Internal,
                content: message.text.clone(),
            })
        })
        .collect()
}

/// Parses one untrusted chat reply. Exact keys only: an unexpected field is
/// rejected, never ignored.
fn parse_chat_turn(payload_json: &str) -> Result<ChatTurn, ChatCoordinatorError> {
    let value: Value = deserialize_strict_json(payload_json.as_bytes())
        .map_err(|_| ChatCoordinatorError::OutputRejected)?;
    let object = value
        .as_object()
        .ok_or(ChatCoordinatorError::OutputRejected)?;
    for key in object.keys() {
        if key != "schemaVersion" && key != "text" {
            return Err(ChatCoordinatorError::OutputRejected);
        }
    }
    if object.get("schemaVersion").and_then(Value::as_str) != Some(CHAT_TURN_SCHEMA) {
        return Err(ChatCoordinatorError::OutputRejected);
    }
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ChatCoordinatorError::OutputRejected)?;
    ChatTurn::new(text.to_owned()).map_err(|_| ChatCoordinatorError::OutputRejected)
}

fn deterministic_chat_fixture() -> String {
    serde_json::json!({
        "schemaVersion": CHAT_TURN_SCHEMA,
        "text": "This reply came from the deterministic local composition; no external model was contacted.",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{parse_chat_turn, ChatCoordinatorError, CHAT_PURPOSE};
    use desktop_runtime::CHAT_TURN_SCHEMA;

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
        ] {
            assert_eq!(
                parse_chat_turn(&rejected).err(),
                Some(ChatCoordinatorError::OutputRejected),
                "accepted a reply outside the closed shape: {rejected}"
            );
        }
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
}
