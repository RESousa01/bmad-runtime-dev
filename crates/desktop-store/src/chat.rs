//! Durable chat transcripts.
//!
//! Chat is the app's default lane, so a conversation must survive a restart
//! like any other local state. Each message body is stored as an encrypted
//! content-addressed payload — the same protection a capability result gets —
//! and the transcript's *shape* (which session, which turn, whose voice) lives
//! in ordinary rows so it can be read back in order without decrypting
//! everything.
//!
//! The store enforces the same invariants the runtime does: a transcript
//! follows one turn grammar, never moves backwards in time, and is appended in
//! order. A write that would break either is rejected rather than stored, so a
//! corrupt transcript cannot be reconstructed from disk.
//!
//! A **turn** is the unit of durability, not a message. A tool-using turn is
//! `user -> (tool_call -> tool_result)* -> assistant`, and persisting those one
//! commit at a time meant a crash or a single failed append could leave a
//! durable prefix that no later turn can legally follow — a conversation wedged
//! permanently, with no delete anywhere in the store able to free it. So
//! [`LocalStore::append_chat_turn`] writes every message of a turn in one
//! transaction: all of it lands, or none of it does.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{canonical_now, LocalStore, StoreError};

const CHAT_MESSAGE_PAYLOAD_KIND: &str = "chat_message";
const CHAT_MESSAGE_SCHEMA: &str = "sapphirus.chat-message.v1";

/// One persisted transcript message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChatMessage {
    pub role: String,
    pub text: String,
    pub created_at_ms: u64,
}

/// One persisted conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChatSession {
    pub session_id: String,
    pub workspace_id: String,
    pub agent_code: Option<String>,
    pub messages: Vec<StoredChatMessage>,
}

/// One message of a turn, as handed to the store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatTurnMessage<'a> {
    pub role: &'a str,
    pub text: &'a str,
    pub created_at_ms: u64,
}

/// What a caller supplies to persist one turn.
#[derive(Clone, Copy, Debug)]
pub struct ChatTurnAppend<'a> {
    pub session_id: &'a str,
    pub workspace_id: &'a str,
    pub agent_code: Option<&'a str>,
    /// The turn's messages in conversation order. Written all-or-nothing.
    pub messages: &'a [ChatTurnMessage<'a>],
}

/// The turn grammar as a transition table (ADR-0009). A `tool_call` binds its
/// `tool_result` immediately, so a call can never be left dangling and a result
/// can never appear unrequested; only an `assistant` message closes a turn.
fn permits(previous: Option<&str>, next: &str) -> bool {
    let permitted: &[&str] = match previous {
        // A transcript opens with a question, and so does every turn after a
        // reply closes the one before it.
        None | Some("assistant") => &["user"],
        Some("user" | "tool_result") => &["assistant", "tool_call"],
        Some("tool_call") => &["tool_result"],
        Some(_) => &[],
    };
    permitted.contains(&next)
}

/// The newest durable row of a transcript: its ordinal, role, and timestamp.
fn durable_tail(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<(i64, String, u64)>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT ordinal, role, created_at_ms FROM chat_messages WHERE session_id = ?1
             ORDER BY ordinal DESC LIMIT 1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?)
}

/// The workspace a conversation belongs to, if it has been written to at all.
fn session_owner(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(connection
        .query_row(
            "SELECT workspace_id FROM chat_sessions WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// The tail a run of messages is validated against: the newest durable row's
/// role and timestamp, or `None` for an empty transcript.
type ChatTail<'a> = Option<(&'a str, u64)>;

/// Checks a whole run of messages against the tail it would extend, without
/// touching the database.
///
/// Both rules are the ones [`desktop_runtime::ChatSession`] already applies in
/// memory. Keeping them identical is the point: the store must not admit a
/// transcript the runtime will refuse to rebuild, because a transcript that
/// cannot be rebuilt is one the user can never see again.
fn validate_run(tail: ChatTail<'_>, messages: &[ChatTurnMessage<'_>]) -> Result<(), StoreError> {
    if messages.is_empty() {
        return Err(StoreError::Inconsistent);
    }
    let mut previous_role = tail.map(|(role, _)| role);
    let mut previous_at = tail.map(|(_, at)| at);
    for message in messages {
        if !matches!(
            message.role,
            "user" | "assistant" | "tool_call" | "tool_result"
        ) || message.text.trim().is_empty()
        {
            return Err(StoreError::Inconsistent);
        }
        if !permits(previous_role, message.role) {
            return Err(StoreError::Inconsistent);
        }
        // A transcript that moves backwards in time is not a chronology, and
        // `ChatSession::append` refuses to rebuild one.
        if previous_at.is_some_and(|previous| message.created_at_ms < previous) {
            return Err(StoreError::Inconsistent);
        }
        previous_role = Some(message.role);
        previous_at = Some(message.created_at_ms);
    }
    Ok(())
}

impl LocalStore {
    /// Appends one message to a conversation, creating it on first use.
    ///
    /// Prefer [`Self::append_chat_turn`] for anything the user will see: a turn
    /// persisted one message at a time can be interrupted between commits, and
    /// the durable prefix left behind wedges the conversation for good.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the append would break the
    /// turn grammar, when it would move the transcript backwards in time, when
    /// the role is unknown, or when the session belongs to another workspace.
    pub fn append_chat_message(
        &self,
        session_id: &str,
        workspace_id: &str,
        agent_code: Option<&str>,
        role: &str,
        text: &str,
        created_at_ms: u64,
    ) -> Result<(), StoreError> {
        self.append_chat_turn(ChatTurnAppend {
            session_id,
            workspace_id,
            agent_code,
            messages: &[ChatTurnMessage {
                role,
                text,
                created_at_ms,
            }],
        })
    }

    /// Persists a whole turn in one transaction: every message lands, or none
    /// does.
    ///
    /// This is what makes a conversation repairable by construction. Committing
    /// per message meant a crash — or one `SQLITE_BUSY` past the busy timeout —
    /// could leave a durable `user` or `tool_call` row with no turn behind it.
    /// The transcript then permits only a continuation a fresh turn never
    /// sends, and nothing in the store deletes rows, so every later send in
    /// that conversation failed forever.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the run is empty, breaks the
    /// turn grammar against what is already durable, moves backwards in time,
    /// carries an unknown role or empty text, or addresses a session owned by
    /// another workspace. Any database failure rolls the whole turn back.
    pub fn append_chat_turn(&self, append: ChatTurnAppend<'_>) -> Result<(), StoreError> {
        self.precheck_chat_turn(&append)?;
        let mut payloads = Vec::with_capacity(append.messages.len());
        for message in append.messages {
            payloads.push(self.put_payload(
                CHAT_MESSAGE_PAYLOAD_KIND,
                CHAT_MESSAGE_SCHEMA,
                message.text.as_bytes(),
            )?);
        }
        self.write_chat_turn(&append, &payloads)
    }

    /// Checks a turn against what is already durable *before* any payload is
    /// written.
    ///
    /// `put_payload` takes the same connection lock the write transaction
    /// holds, so it cannot run inside that transaction — the payloads have to
    /// go down first. Without this pass a turn refused on its own grammar would
    /// leave an orphan payload row and CAS file behind for every message it
    /// carried. The authoritative check is still the one inside the
    /// transaction; this one only keeps the ordinary refusal cheap.
    fn precheck_chat_turn(&self, append: &ChatTurnAppend<'_>) -> Result<(), StoreError> {
        let connection = self.connection.lock();
        if session_owner(&connection, append.session_id)?
            .is_some_and(|owner| owner != append.workspace_id)
        {
            return Err(StoreError::Inconsistent);
        }
        let tail = durable_tail(&connection, append.session_id)?;
        validate_run(
            tail.as_ref().map(|(_, role, at)| (role.as_str(), *at)),
            append.messages,
        )
    }

    /// Writes every message of one turn in a single transaction. Returns
    /// without having written anything at all if any part of it is refused.
    fn write_chat_turn(
        &self,
        append: &ChatTurnAppend<'_>,
        payloads: &[super::PayloadRef],
    ) -> Result<(), StoreError> {
        let first = append
            .messages
            .first()
            .ok_or(StoreError::Inconsistent)?
            .created_at_ms;
        let last_at = append
            .messages
            .last()
            .ok_or(StoreError::Inconsistent)?
            .created_at_ms;

        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match session_owner(&transaction, append.session_id)? {
            // A session id is derived from the workspace, so a mismatch means
            // the caller is addressing someone else's conversation.
            Some(owner) if owner != append.workspace_id => return Err(StoreError::Inconsistent),
            Some(_) => {}
            None => {
                // The parent row first, because `chat_messages` references it.
                // Stamped from the turn's opening message; `updated_at_ms` is
                // advanced to the turn's close once every message is in.
                transaction.execute(
                    "INSERT INTO chat_sessions
                     (session_id, workspace_id, agent_code, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![
                        append.session_id,
                        append.workspace_id,
                        append.agent_code,
                        first
                    ],
                )?;
            }
        }

        let tail = durable_tail(&transaction, append.session_id)?;
        validate_run(
            tail.as_ref().map(|(_, role, at)| (role.as_str(), *at)),
            append.messages,
        )?;

        let mut next_ordinal = match &tail {
            None => 0_i64,
            Some((ordinal, _, _)) => ordinal.checked_add(1).ok_or(StoreError::Inconsistent)?,
        };
        for (message, payload) in append.messages.iter().zip(payloads.iter()) {
            transaction.execute(
                "INSERT INTO chat_messages
                 (session_id, ordinal, role, content_hash, payload_kind, schema_version,
                  created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    append.session_id,
                    next_ordinal,
                    message.role,
                    payload.content_hash,
                    CHAT_MESSAGE_PAYLOAD_KIND,
                    payload.schema_version,
                    message.created_at_ms,
                ],
            )?;
            next_ordinal = next_ordinal
                .checked_add(1)
                .ok_or(StoreError::Inconsistent)?;
        }

        transaction.execute(
            "UPDATE chat_sessions SET agent_code = ?2, updated_at_ms = ?3
             WHERE session_id = ?1",
            params![append.session_id, append.agent_code, last_at],
        )?;
        let _ = canonical_now();
        transaction.commit()?;
        Ok(())
    }

    /// Removes the trailing messages that belong to no completed turn, so a
    /// conversation wedged by an older build can be used again. Returns how
    /// many rows were removed.
    ///
    /// This is the store's only delete, and it is deliberately the narrowest
    /// one that can free a wedged transcript: it removes exactly the rows after
    /// the newest `assistant`, and nothing at or before it. Those rows are
    /// already invisible — [`desktop_runtime::ChatSession::rollback_pending_user`]
    /// drops precisely the same run from memory on every load, so the user has
    /// never seen them and never will. What they still do is block every future
    /// send, because the grammar demands a continuation a new turn never sends.
    ///
    /// A transcript whose newest message is an `assistant` reply, or one that
    /// is already empty, is left untouched.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the session belongs to another
    /// workspace — the same guard every write carries, because a delete aimed
    /// at someone else's conversation is the one mistake this must not make —
    /// and [`StoreError`] when the database cannot be written.
    pub fn repair_dangling_chat_tail(
        &self,
        session_id: &str,
        workspace_id: &str,
    ) -> Result<u64, StoreError> {
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if session_owner(&transaction, session_id)?.is_some_and(|owner| owner != workspace_id) {
            return Err(StoreError::Inconsistent);
        }
        // The newest closed turn. Everything after it is, by the grammar, a
        // partial turn: no `assistant` row can appear past this ordinal.
        let last_closed: Option<i64> = transaction.query_row(
            "SELECT MAX(ordinal) FROM chat_messages
             WHERE session_id = ?1 AND role = 'assistant'",
            params![session_id],
            |row| row.get(0),
        )?;
        let removed = match last_closed {
            Some(ordinal) => transaction.execute(
                "DELETE FROM chat_messages WHERE session_id = ?1 AND ordinal > ?2",
                params![session_id, ordinal],
            )?,
            None => transaction.execute(
                "DELETE FROM chat_messages WHERE session_id = ?1",
                params![session_id],
            )?,
        };
        transaction.commit()?;
        u64::try_from(removed).map_err(|_| StoreError::Inconsistent)
    }

    /// Loads one conversation in order, decrypting each message body.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a message
    /// payload fails authenticated decryption.
    pub fn chat_session(&self, session_id: &str) -> Result<Option<StoredChatSession>, StoreError> {
        let header: Option<(String, Option<String>)> = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT workspace_id, agent_code FROM chat_sessions WHERE session_id = ?1",
                    params![session_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
        };
        let Some((workspace_id, agent_code)) = header else {
            return Ok(None);
        };

        let rows: Vec<(String, String, u64, u32, u64)> = {
            let connection = self.connection.lock();
            let mut statement = connection.prepare(
                "SELECT m.role, m.content_hash, p.byte_count, p.key_version, m.created_at_ms
                 FROM chat_messages m
                 JOIN payloads p
                   ON p.content_hash = m.content_hash
                  AND p.kind = m.payload_kind
                  AND p.schema_version = m.schema_version
                 WHERE m.session_id = ?1
                 ORDER BY m.ordinal ASC",
            )?;
            let mapped = statement.query_map(params![session_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let mut messages = Vec::with_capacity(rows.len());
        for (role, content_hash, byte_count, key_version, created_at_ms) in rows {
            let reference = super::PayloadRef {
                content_hash,
                kind: CHAT_MESSAGE_PAYLOAD_KIND.to_owned(),
                schema_version: CHAT_MESSAGE_SCHEMA.to_owned(),
                byte_count,
                key_version,
            };
            let bytes = self.get_payload(&reference)?;
            messages.push(StoredChatMessage {
                role,
                text: String::from_utf8(bytes).map_err(|_| StoreError::Inconsistent)?,
                created_at_ms,
            });
        }

        Ok(Some(StoredChatSession {
            session_id: session_id.to_owned(),
            workspace_id,
            agent_code,
            messages,
        }))
    }
}
