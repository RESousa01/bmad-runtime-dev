//! Ordinary conversation: the app's default lane.
//!
//! Forge is a chat application first. A user types, the assistant answers, and
//! the exchange accumulates as a transcript. BMAD is an *optional overlay* on
//! that lane — selecting an agent binds a sealed persona to the conversation,
//! and skills and workflows run through the separate capability lane — but no
//! BMAD concept is required to hold a conversation.
//!
//! Nothing here carries authority. A transcript is bounded text: it cannot
//! reference a path, request an effect, or widen what may be sent. The
//! assistant's reply enters only through [`ChatTurn`], which the host builds
//! from verified model output exactly like any other reviewed archetype.

use crate::{ContractId, UnixMillis};

pub const CHAT_TURN_SCHEMA: &str = "sapphirus.chat-turn.v1";

/// Per-message ceiling. Applies to what the user sends and to what the model
/// is allowed to return.
const MAX_MESSAGE_BYTES: usize = 32_768;
/// Retained transcript depth. Older messages are pruned in whole turns so the
/// transcript never begins mid-exchange.
const MAX_SESSION_MESSAGES: usize = 200;
/// Ceiling on the transcript actually placed in one request's reviewed
/// context. Independent of retention: a long conversation keeps its history
/// but sends only the newest window.
pub const MAX_PROMPT_TRANSCRIPT_BYTES: usize = 131_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatError {
    /// The message is empty, or exceeds its reviewed byte ceiling.
    BoundsViolation,
    /// A user message arrived while the previous one was still unanswered, or
    /// an assistant reply arrived with no question to answer.
    TurnOutOfOrder,
    /// Message timestamps must not move backwards within one session.
    NonMonotonicClock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub created_at: UnixMillis,
}

impl ChatMessage {
    /// Builds one bounded transcript message.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::BoundsViolation`] when the text is empty,
    /// whitespace-only, or exceeds [`MAX_MESSAGE_BYTES`].
    pub fn new(role: ChatRole, text: String, created_at: UnixMillis) -> Result<Self, ChatError> {
        if text.trim().is_empty() || text.len() > MAX_MESSAGE_BYTES {
            return Err(ChatError::BoundsViolation);
        }
        Ok(Self {
            role,
            text,
            created_at,
        })
    }
}

/// One verified assistant reply. Built only from model output that has already
/// passed strict JSON decoding and exact-key checking at the host boundary.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    schema_version: &'static str,
    pub text: String,
}

impl ChatTurn {
    /// Builds one bounded assistant reply.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::BoundsViolation`] when the reply is empty or
    /// exceeds [`MAX_MESSAGE_BYTES`].
    pub fn new(text: String) -> Result<Self, ChatError> {
        if text.trim().is_empty() || text.len() > MAX_MESSAGE_BYTES {
            return Err(ChatError::BoundsViolation);
        }
        Ok(Self {
            schema_version: CHAT_TURN_SCHEMA,
            text,
        })
    }

    #[must_use]
    pub const fn schema_id(&self) -> &'static str {
        CHAT_TURN_SCHEMA
    }
}

/// One conversation bound to a workspace, optionally wearing a BMAD persona.
///
/// The transcript strictly alternates user -> assistant. A pending user
/// message (one whose reply has not landed) is explicit state: the send path
/// either completes it or rolls it back, so a failed request never leaves a
/// question stranded in the transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatSession {
    session_id: ContractId,
    workspace_id: ContractId,
    agent_code: Option<String>,
    messages: Vec<ChatMessage>,
}

impl ChatSession {
    #[must_use]
    pub const fn open(session_id: ContractId, workspace_id: ContractId) -> Self {
        Self {
            session_id,
            workspace_id,
            agent_code: None,
            messages: Vec::new(),
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> &ContractId {
        &self.session_id
    }

    #[must_use]
    pub const fn workspace_id(&self) -> &ContractId {
        &self.workspace_id
    }

    /// The BMAD agent whose sealed persona is bound to this conversation, if
    /// the user selected one. `None` is an ordinary assistant conversation.
    #[must_use]
    pub fn agent_code(&self) -> Option<&str> {
        self.agent_code.as_deref()
    }

    /// Binds or clears the persona. Selecting an agent mid-conversation
    /// affects subsequent turns only; the transcript is never rewritten.
    pub fn set_agent_code(&mut self, agent_code: Option<String>) {
        self.agent_code = agent_code;
    }

    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Whether a user message is awaiting its reply.
    #[must_use]
    pub fn has_pending_user_message(&self) -> bool {
        matches!(
            self.messages.last(),
            Some(ChatMessage {
                role: ChatRole::User,
                ..
            })
        )
    }

    /// Appends the user's message and marks the turn pending.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::TurnOutOfOrder`] when a previous user message is
    /// still unanswered, [`ChatError::NonMonotonicClock`] on a backwards
    /// clock, and [`ChatError::BoundsViolation`] on an unbounded message.
    pub fn append_user(&mut self, text: String, at: UnixMillis) -> Result<(), ChatError> {
        if self.has_pending_user_message() {
            return Err(ChatError::TurnOutOfOrder);
        }
        self.append(ChatMessage::new(ChatRole::User, text, at)?)
    }

    /// Completes the pending turn with the verified assistant reply.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::TurnOutOfOrder`] when no user message is pending,
    /// and [`ChatError::NonMonotonicClock`] on a backwards clock.
    pub fn append_assistant(&mut self, turn: &ChatTurn, at: UnixMillis) -> Result<(), ChatError> {
        if !self.has_pending_user_message() {
            return Err(ChatError::TurnOutOfOrder);
        }
        self.append(ChatMessage::new(
            ChatRole::Assistant,
            turn.text.clone(),
            at,
        )?)
    }

    /// Withdraws the pending user message after a failed request, so a
    /// transport error cannot strand an unanswered question in the transcript.
    pub fn rollback_pending_user(&mut self) {
        if self.has_pending_user_message() {
            self.messages.pop();
        }
    }

    fn append(&mut self, message: ChatMessage) -> Result<(), ChatError> {
        if let Some(last) = self.messages.last() {
            if message.created_at < last.created_at {
                return Err(ChatError::NonMonotonicClock);
            }
        }
        self.messages.push(message);
        self.prune();
        Ok(())
    }

    /// Drops whole oldest turns so the retained transcript never starts with
    /// an orphaned assistant reply.
    fn prune(&mut self) {
        while self.messages.len() > MAX_SESSION_MESSAGES {
            self.messages.remove(0);
            if matches!(
                self.messages.first(),
                Some(ChatMessage {
                    role: ChatRole::Assistant,
                    ..
                })
            ) {
                self.messages.remove(0);
            }
        }
    }

    /// The newest window of the transcript that fits `max_bytes`, in
    /// conversation order. Always keeps whole turns, and always keeps at least
    /// the newest message so a send can never become contextless.
    #[must_use]
    pub fn prompt_window(&self, max_bytes: usize) -> &[ChatMessage] {
        let mut start = self.messages.len();
        let mut total = 0_usize;
        for (index, message) in self.messages.iter().enumerate().rev() {
            total = total.saturating_add(message.text.len());
            if total > max_bytes && index + 1 < self.messages.len() {
                break;
            }
            start = index;
        }
        // Never open the window on an orphaned assistant reply.
        if matches!(
            self.messages.get(start),
            Some(ChatMessage {
                role: ChatRole::Assistant,
                ..
            })
        ) {
            start = start.saturating_add(1);
        }
        self.messages.get(start..).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        ChatError, ChatRole, ChatSession, ChatTurn, MAX_PROMPT_TRANSCRIPT_BYTES,
        MAX_SESSION_MESSAGES,
    };
    use crate::{ContractId, UnixMillis};

    fn session() -> ChatSession {
        ChatSession::open(
            ContractId::new("chat_test").expect("session id"),
            ContractId::new("ws_test").expect("workspace id"),
        )
    }

    fn exchange(session: &mut ChatSession, index: u64) {
        session
            .append_user(format!("question {index}"), UnixMillis(1_000 + index * 2))
            .expect("user message");
        let turn = ChatTurn::new(format!("answer {index}")).expect("turn");
        session
            .append_assistant(&turn, UnixMillis(1_001 + index * 2))
            .expect("assistant message");
    }

    #[test]
    fn a_conversation_alternates_and_needs_no_bmad_concept() {
        let mut session = session();
        assert!(session.is_empty());
        assert_eq!(session.agent_code(), None);

        exchange(&mut session, 0);
        exchange(&mut session, 1);

        assert_eq!(session.messages().len(), 4);
        assert_eq!(session.messages()[0].role, ChatRole::User);
        assert_eq!(session.messages()[1].role, ChatRole::Assistant);
        assert_eq!(session.messages()[3].text, "answer 1");
        assert!(!session.has_pending_user_message());
    }

    #[test]
    fn a_second_question_cannot_jump_an_unanswered_one() {
        let mut session = session();
        session
            .append_user("first".to_owned(), UnixMillis(1_000))
            .expect("first message");

        assert_eq!(
            session.append_user("second".to_owned(), UnixMillis(1_001)),
            Err(ChatError::TurnOutOfOrder)
        );
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn an_assistant_reply_needs_a_question() {
        let mut session = session();
        let turn = ChatTurn::new("unsolicited".to_owned()).expect("turn");

        assert_eq!(
            session.append_assistant(&turn, UnixMillis(1_000)),
            Err(ChatError::TurnOutOfOrder)
        );
        assert!(session.is_empty());
    }

    #[test]
    fn a_failed_request_does_not_strand_the_question() {
        let mut session = session();
        session
            .append_user("will fail".to_owned(), UnixMillis(1_000))
            .expect("user message");
        assert!(session.has_pending_user_message());

        session.rollback_pending_user();

        assert!(session.is_empty());
        // Rollback is idempotent and never eats an answered turn.
        exchange(&mut session, 0);
        session.rollback_pending_user();
        assert_eq!(session.messages().len(), 2);
    }

    #[test]
    fn empty_and_oversized_messages_fail_closed() {
        let mut session = session();
        for empty in ["", "   ", "\n\t "] {
            assert_eq!(
                session.append_user(empty.to_owned(), UnixMillis(1_000)),
                Err(ChatError::BoundsViolation)
            );
        }
        assert_eq!(
            session.append_user("x".repeat(32_769), UnixMillis(1_000)),
            Err(ChatError::BoundsViolation)
        );
        assert_eq!(
            ChatTurn::new(String::new()),
            Err(ChatError::BoundsViolation)
        );
        assert_eq!(
            ChatTurn::new("y".repeat(32_769)),
            Err(ChatError::BoundsViolation)
        );
        assert!(session.is_empty());
    }

    #[test]
    fn a_backwards_clock_fails_closed() {
        let mut session = session();
        session
            .append_user("first".to_owned(), UnixMillis(5_000))
            .expect("user message");
        let turn = ChatTurn::new("reply".to_owned()).expect("turn");

        assert_eq!(
            session.append_assistant(&turn, UnixMillis(4_999)),
            Err(ChatError::NonMonotonicClock)
        );
    }

    #[test]
    fn retention_prunes_whole_turns() {
        let mut session = session();
        for index in 0..(MAX_SESSION_MESSAGES as u64) {
            exchange(&mut session, index);
        }

        assert!(session.messages().len() <= MAX_SESSION_MESSAGES);
        // The transcript never begins mid-exchange.
        assert_eq!(session.messages()[0].role, ChatRole::User);
        assert_eq!(
            session.messages().last().expect("newest").text,
            format!("answer {}", MAX_SESSION_MESSAGES - 1)
        );
    }

    #[test]
    fn the_prompt_window_keeps_the_newest_whole_turns() {
        let mut session = session();
        for index in 0..8 {
            exchange(&mut session, index);
        }

        let window = session.prompt_window(40);
        assert!(!window.is_empty());
        assert_eq!(window[0].role, ChatRole::User);
        assert_eq!(
            window.last().expect("newest windowed message").text,
            "answer 7"
        );
        assert!(window.len() < session.messages().len());

        // A generous ceiling keeps everything.
        assert_eq!(
            session.prompt_window(MAX_PROMPT_TRANSCRIPT_BYTES).len(),
            session.messages().len()
        );
    }

    #[test]
    fn one_oversized_newest_message_still_reaches_the_model() {
        let mut session = session();
        session
            .append_user("q".repeat(1_000), UnixMillis(1_000))
            .expect("user message");

        // Even under a ceiling smaller than the message, the send is never
        // left with nothing to say.
        assert_eq!(session.prompt_window(10).len(), 1);
    }

    #[test]
    fn the_persona_binding_is_optional_and_affects_only_later_turns() {
        let mut session = session();
        exchange(&mut session, 0);
        assert_eq!(session.agent_code(), None);

        session.set_agent_code(Some("bmad-agent-architect".to_owned()));
        assert_eq!(session.agent_code(), Some("bmad-agent-architect"));
        assert_eq!(session.messages().len(), 2, "transcript is never rewritten");

        session.set_agent_code(None);
        assert_eq!(session.agent_code(), None);
    }
}
