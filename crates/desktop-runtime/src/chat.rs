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
/// The tool-bearing reply schema, used only for turns where tools were actually
/// granted.
///
/// It is a separate schema rather than a widening of
/// [`CHAT_TURN_SCHEMA`] on purpose. The v1 system prompt tells the model *"you
/// cannot read, write, or execute anything"*, and that sentence has to stay
/// true whenever it is sent. Splitting the schemas keeps the promise and the
/// capability in step, and leaves the egress manifest hash of every existing
/// text-only turn untouched.
pub const CHAT_TOOL_TURN_SCHEMA: &str = "sapphirus.chat-turn.v2";

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
    /// The model asked the host to run a tool. A request, not an authorisation.
    ToolCall,
    /// What the host got back. Attacker-influenced content: data, never
    /// instruction.
    ToolResult,
}

impl ChatRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
        }
    }

    /// Whether this role closes a turn. Only a final assistant reply does; a
    /// turn stays open across any number of tool exchanges.
    #[must_use]
    pub const fn closes_turn(self) -> bool {
        matches!(self, Self::Assistant)
    }

    /// Whether a turn may begin with this role. A transcript, and every turn in
    /// it, opens with a question.
    #[must_use]
    pub const fn opens_turn(self) -> bool {
        matches!(self, Self::User)
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

/// Longest tool name and subject a reply may carry. A subject is a workspace
/// path or a pattern, so it is bounded by the same ceiling as a relative path.
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_SUBJECT_BYTES: usize = 1_024;

/// One tool the model has asked the host to run.
///
/// This is a *request*, never an authorisation. `tool` and `subject` are the
/// pair the permission gate evaluates, and nothing here has been consented to
/// yet — a reply carrying a tool call has exactly the authority of a reply
/// carrying prose, which is none.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInvocation {
    pub tool: String,
    pub subject: String,
}

impl ToolInvocation {
    /// Builds one bounded tool request.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::BoundsViolation`] when either field is empty or
    /// exceeds its ceiling.
    pub fn new(tool: String, subject: String) -> Result<Self, ChatError> {
        if tool.trim().is_empty()
            || tool.len() > MAX_TOOL_NAME_BYTES
            || subject.trim().is_empty()
            || subject.len() > MAX_TOOL_SUBJECT_BYTES
        {
            return Err(ChatError::BoundsViolation);
        }
        Ok(Self { tool, subject })
    }
}

/// What one model reply may be under the tool-bearing schema: prose, or a
/// single tool request. Never both, and never neither.
///
/// Keeping these mutually exclusive is what stops a reply from narrating one
/// thing to the user while asking the host to do another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatReply {
    Text(ChatTurn),
    ToolCall(ToolInvocation),
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

    /// Whether a turn is open — a question has been asked and no final
    /// assistant reply has closed it yet.
    ///
    /// A turn stays open across tool exchanges, so this is *not* simply "the
    /// last message is from the user".
    #[must_use]
    pub fn has_pending_user_message(&self) -> bool {
        self.messages
            .last()
            .is_some_and(|message| !message.role.closes_turn())
    }

    /// Whether the transcript is waiting for the host to answer a tool call.
    /// Only a `tool_result` may follow.
    #[must_use]
    pub fn awaits_tool_result(&self) -> bool {
        matches!(
            self.messages.last(),
            Some(ChatMessage {
                role: ChatRole::ToolCall,
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
        if !self.has_pending_user_message() || self.awaits_tool_result() {
            return Err(ChatError::TurnOutOfOrder);
        }
        self.append(ChatMessage::new(
            ChatRole::Assistant,
            turn.text.clone(),
            at,
        )?)
    }

    /// Records that the model asked for a tool, mid-turn.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::TurnOutOfOrder`] when no turn is open or a previous
    /// tool call is still unanswered, [`ChatError::NonMonotonicClock`] on a
    /// backwards clock, and [`ChatError::BoundsViolation`] on an unbounded
    /// rendering.
    pub fn append_tool_call(
        &mut self,
        invocation: &ToolInvocation,
        at: UnixMillis,
    ) -> Result<(), ChatError> {
        if !self.has_pending_user_message() || self.awaits_tool_result() {
            return Err(ChatError::TurnOutOfOrder);
        }
        self.append(ChatMessage::new(
            ChatRole::ToolCall,
            format!("{} {}", invocation.tool, invocation.subject),
            at,
        )?)
    }

    /// Records what the host got back for the outstanding tool call.
    ///
    /// The text is workspace-derived and therefore attacker-influenced. It is
    /// stored as an ordinary transcript message so it is visible to the user
    /// and bounded like everything else — it carries no authority.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError::TurnOutOfOrder`] when no tool call is outstanding,
    /// [`ChatError::NonMonotonicClock`] on a backwards clock, and
    /// [`ChatError::BoundsViolation`] on an unbounded result.
    pub fn append_tool_result(&mut self, text: String, at: UnixMillis) -> Result<(), ChatError> {
        if !self.awaits_tool_result() {
            return Err(ChatError::TurnOutOfOrder);
        }
        self.append(ChatMessage::new(ChatRole::ToolResult, text, at)?)
    }

    /// Withdraws the pending user message after a failed request, so a
    /// transport error cannot strand an unanswered question in the transcript.
    pub fn rollback_pending_user(&mut self) {
        // Drops the whole open turn, not one message: a failed request may have
        // already accumulated tool exchanges, and leaving those behind would
        // strand a call with no reply in the transcript.
        while self.has_pending_user_message() {
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
            // Drop the remainder of the partial turn. A turn is no longer two
            // messages — with tool exchanges it can be many — so this consumes
            // whatever is left rather than exactly one more.
            while self
                .messages
                .first()
                .is_some_and(|message| !message.role.opens_turn())
            {
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
        // Never open the window mid-turn: an orphaned assistant reply, or a
        // tool result whose call has been windowed out, would present the model
        // with an effect it never asked for.
        while self
            .messages
            .get(start)
            .is_some_and(|message| !message.role.opens_turn())
        {
            start = start.saturating_add(1);
        }
        // Skipping forward can run off the end, when the byte cutoff lands
        // inside the newest turn and there is no later turn to fall back on. A
        // tool-using turn is many messages long, so this is reachable rather
        // than theoretical — and returning an empty window here would seal a
        // request carrying no transcript and no question at all. Send the whole
        // current turn instead: over budget beats contextless, which is what
        // "always keeps at least the newest message" is there to promise.
        if start >= self.messages.len() {
            start = self
                .messages
                .iter()
                .rposition(|message| message.role.opens_turn())
                .unwrap_or(0);
        }
        self.messages.get(start..).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        ChatError, ChatRole, ChatSession, ChatTurn, ToolInvocation, MAX_PROMPT_TRANSCRIPT_BYTES,
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
    fn a_long_tool_turn_never_yields_an_empty_prompt_window() {
        // The byte cutoff lands inside the newest, still-open turn: there is no
        // later turn to skip forward to. Returning an empty window here would
        // seal a request with no transcript and no question.
        let mut session = session();
        session
            .append_user("question".to_owned(), UnixMillis(1_000))
            .expect("user message");
        for index in 0..4 {
            let invocation =
                ToolInvocation::new("grep".to_owned(), format!("pattern{index}")).expect("call");
            session
                .append_tool_call(&invocation, UnixMillis(1_001 + index * 2))
                .expect("tool call");
            session
                .append_tool_result("x".repeat(64), UnixMillis(1_002 + index * 2))
                .expect("tool result");
        }

        let window = session.prompt_window(16);
        assert!(!window.is_empty(), "a send must never become contextless");
        assert!(
            window[0].role.opens_turn(),
            "the window must still open on a question, not mid-turn"
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
