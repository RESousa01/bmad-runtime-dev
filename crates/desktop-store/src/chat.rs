//! Durable chat transcripts.
//!
//! Chat is the app's default lane, so a conversation must survive a restart
//! like any other local state. Each message body is stored as an encrypted
//! content-addressed payload — the same protection a capability result gets —
//! and the transcript's *shape* (which session, which turn, whose voice) lives
//! in ordinary rows so it can be read back in order without decrypting
//! everything.
//!
//! The store enforces the same invariant the runtime does: a transcript
//! strictly alternates user then assistant, appended in order. A write that
//! would break the alternation is rejected rather than stored, so a corrupt
//! transcript cannot be reconstructed from disk.

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

impl LocalStore {
    /// Appends one message to a conversation, creating it on first use.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Inconsistent`] when the append would break the
    /// turn grammar, when the role is unknown, or when the session belongs to
    /// another workspace.
    ///
    /// The grammar is `user -> (tool_call -> tool_result)* -> assistant`
    /// (ADR-0009). It is still strict: a transcript opens with a question, a
    /// `tool_call` is always answered by exactly one `tool_result`, and only an
    /// `assistant` message closes the turn. What it no longer requires is that
    /// the two roles simply alternate.
    pub fn append_chat_message(
        &self,
        session_id: &str,
        workspace_id: &str,
        agent_code: Option<&str>,
        role: &str,
        text: &str,
        created_at_ms: u64,
    ) -> Result<(), StoreError> {
        if !matches!(role, "user" | "assistant" | "tool_call" | "tool_result")
            || text.trim().is_empty()
        {
            return Err(StoreError::Inconsistent);
        }
        let payload = self.put_payload(
            CHAT_MESSAGE_PAYLOAD_KIND,
            CHAT_MESSAGE_SCHEMA,
            text.as_bytes(),
        )?;

        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT workspace_id FROM chat_sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?;
        match existing {
            // A session id is derived from the workspace, so a mismatch means
            // the caller is addressing someone else's conversation.
            Some(owner) if owner != workspace_id => return Err(StoreError::Inconsistent),
            Some(_) => {
                transaction.execute(
                    "UPDATE chat_sessions SET agent_code = ?2, updated_at_ms = ?3
                     WHERE session_id = ?1",
                    params![session_id, agent_code, created_at_ms],
                )?;
            }
            None => {
                transaction.execute(
                    "INSERT INTO chat_sessions
                     (session_id, workspace_id, agent_code, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![session_id, workspace_id, agent_code, created_at_ms],
                )?;
            }
        }

        let last: Option<(i64, String)> = transaction
            .query_row(
                "SELECT ordinal, role FROM chat_messages WHERE session_id = ?1
                 ORDER BY ordinal DESC LIMIT 1",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let next_ordinal = match &last {
            None => {
                // A transcript always opens with a question.
                if role != "user" {
                    return Err(StoreError::Inconsistent);
                }
                0_i64
            }
            Some((ordinal, previous)) => {
                // The turn grammar as a transition table. A `tool_call` binds
                // its `tool_result` immediately, so a call can never be left
                // dangling and a result can never appear unrequested.
                let permitted: &[&str] = match previous.as_str() {
                    "user" | "tool_result" => &["assistant", "tool_call"],
                    "tool_call" => &["tool_result"],
                    "assistant" => &["user"],
                    _ => return Err(StoreError::Inconsistent),
                };
                if !permitted.contains(&role) {
                    return Err(StoreError::Inconsistent);
                }
                ordinal.checked_add(1).ok_or(StoreError::Inconsistent)?
            }
        };

        transaction.execute(
            "INSERT INTO chat_messages
             (session_id, ordinal, role, content_hash, payload_kind, schema_version, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                next_ordinal,
                role,
                payload.content_hash,
                CHAT_MESSAGE_PAYLOAD_KIND,
                payload.schema_version,
                created_at_ms,
            ],
        )?;
        let _ = canonical_now();
        transaction.commit()?;
        Ok(())
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
