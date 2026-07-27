//! Durable retention for chat transcripts.
//!
//! `ChatSession::prune` bounds a conversation at
//! `desktop_runtime`'s `MAX_SESSION_MESSAGES`, but it prunes a `Vec` in memory.
//! Nothing pruned the rows, so `chat_messages` and the encrypted payloads behind
//! it grew for the life of the install. The rows past the runtime's ceiling were
//! not even reachable: `ChatCoordinator::rehydrate` replays every stored message
//! through `ChatSession::append_*`, and each append prunes, so a transcript of
//! ten thousand rows rebuilds into at most the newest couple of hundred. The
//! store was paying — in disk, in boot-time decryption — to keep a record no
//! surface could ever show.
//!
//! ## The unit of deletion is a turn
//!
//! A turn is `user -> (tool_call -> tool_result)* -> assistant`, and a partial
//! turn is worse than no turn: a durable `user` or `tool_call` row with nothing
//! behind it permits only a continuation a fresh turn never sends, which wedged
//! conversations permanently until [`super::LocalStore::append_chat_turn`] made
//! a turn atomic. Retention must not reintroduce what that fixed, so it cuts
//! only at a turn boundary.
//!
//! The grammar makes those boundaries free to find: `permits` admits `user` only
//! after an `assistant` reply or at the start of a transcript, so *every* `user`
//! row opens a turn and cutting at one can never split a turn.
//!
//! The ceiling therefore yields rather than the turn: a single turn longer than
//! the whole budget is kept whole, because half of it would cost the user the
//! conversation.

use rusqlite::{params, OptionalExtension};

use super::chat::{CHAT_MESSAGE_PAYLOAD_KIND, CHAT_MESSAGE_SCHEMA};
use super::{LocalStore, PayloadRef, StoreError};

/// How many transcript rows one conversation keeps on disk.
///
/// Twice the runtime's in-memory ceiling, deliberately. The runtime's prune is
/// what the user actually sees, and the store must never be the tighter of the
/// two: if the two constants ever drift apart, the headroom is what stops a
/// store-side bound from silently deleting history a raised runtime bound would
/// have displayed.
pub const MAX_DURABLE_CHAT_MESSAGES: usize = 400;

impl LocalStore {
    /// Trims one conversation's durable transcript to whole turns within
    /// [`MAX_DURABLE_CHAT_MESSAGES`], and returns how many rows were removed.
    ///
    /// Applied automatically by [`LocalStore::append_chat_turn`]; exposed for an
    /// explicit sweep over a transcript an older build already grew.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be written. Nothing is
    /// removed unless the whole trim commits.
    pub fn enforce_chat_retention(&self, session_id: &str) -> Result<u64, StoreError> {
        let (removed, discarded) = {
            let mut connection = self.connection.lock();
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let outcome = prune_chat_session(&transaction, session_id)?;
            transaction.commit()?;
            outcome
        };
        self.discard_cas_files(&discarded);
        Ok(removed)
    }

    /// Unlinks the ciphertext of payloads whose registration has already been
    /// removed.
    ///
    /// Best effort by construction, and safe in that order only: the row is gone
    /// first, so a failure here leaves an unreferenced file wasting disk, while
    /// the reverse would leave a registered payload whose ciphertext is missing
    /// — exactly the state boot verification treats as damage.
    pub(crate) fn discard_cas_files(&self, references: &[PayloadRef]) {
        for reference in references {
            if let Ok(path) = self.cas_path(
                &reference.kind,
                &reference.schema_version,
                &reference.content_hash,
            ) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Trims one transcript inside a caller's transaction.
///
/// Returns the number of message rows removed and the payloads whose ciphertext
/// the caller must unlink *after* the transaction commits.
pub(crate) fn prune_chat_session(
    transaction: &rusqlite::Transaction<'_>,
    session_id: &str,
) -> Result<(u64, Vec<PayloadRef>), StoreError> {
    let ordered = {
        let mut statement = transaction.prepare(
            "SELECT ordinal, role FROM chat_messages WHERE session_id = ?1 ORDER BY ordinal ASC",
        )?;
        let rows = statement
            .query_map(params![session_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if ordered.len() <= MAX_DURABLE_CHAT_MESSAGES {
        return Ok((0, Vec::new()));
    }
    // The earliest turn boundary whose suffix fits the budget: the largest
    // retained window that is still made only of whole turns.
    let cut = ordered.iter().enumerate().position(|(index, (_, role))| {
        role.as_str() == "user" && ordered.len().saturating_sub(index) <= MAX_DURABLE_CHAT_MESSAGES
    });
    // No boundary fits, so one turn is longer than the entire budget. Keep it
    // whole: a fragment would wedge the conversation, and a wedged conversation
    // is a worse outcome than an oversized one.
    let Some((cutoff, _)) = cut.and_then(|index| ordered.get(index)) else {
        return Ok((0, Vec::new()));
    };
    let cutoff = *cutoff;

    let doomed = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT content_hash FROM chat_messages
             WHERE session_id = ?1 AND ordinal < ?2",
        )?;
        let hashes = statement
            .query_map(params![session_id, cutoff], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        hashes
    };
    let removed = transaction.execute(
        "DELETE FROM chat_messages WHERE session_id = ?1 AND ordinal < ?2",
        params![session_id, cutoff],
    )?;
    let discarded = deregister_unreferenced_payloads(transaction, &doomed)?;
    Ok((
        u64::try_from(removed).map_err(|_| StoreError::Inconsistent)?,
        discarded,
    ))
}

/// Drops the payload registrations that the removed messages were the last
/// holders of, and returns them so their ciphertext can be unlinked.
///
/// Content addressing means one payload can back several messages — the same
/// sentence sent twice is stored once — so a hash is only released when no
/// message anywhere still cites it. Evidence is checked too: a payload named by
/// an evidence event must stay registered or `verify_evidence_rows` reads the
/// deletion as a broken chain.
fn deregister_unreferenced_payloads(
    transaction: &rusqlite::Transaction<'_>,
    hashes: &[String],
) -> Result<Vec<PayloadRef>, StoreError> {
    let mut discarded = Vec::new();
    for content_hash in hashes {
        let still_cited: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM chat_messages WHERE content_hash = ?1)
                 OR EXISTS(SELECT 1 FROM evidence_events WHERE payload_hash = ?1)",
            params![content_hash],
            |row| row.get(0),
        )?;
        if still_cited {
            continue;
        }
        let reference = transaction
            .query_row(
                "SELECT byte_count, key_version FROM payloads
                 WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
                params![content_hash, CHAT_MESSAGE_PAYLOAD_KIND, CHAT_MESSAGE_SCHEMA],
                |row| {
                    Ok(PayloadRef {
                        content_hash: content_hash.clone(),
                        kind: CHAT_MESSAGE_PAYLOAD_KIND.to_owned(),
                        schema_version: CHAT_MESSAGE_SCHEMA.to_owned(),
                        byte_count: row.get(0)?,
                        key_version: row.get(1)?,
                    })
                },
            )
            .optional()?;
        let Some(reference) = reference else {
            continue;
        };
        transaction.execute(
            "DELETE FROM payloads
             WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
            params![
                reference.content_hash,
                reference.kind,
                reference.schema_version
            ],
        )?;
        discarded.push(reference);
    }
    Ok(discarded)
}
