use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};

use super::StoreError;

pub(crate) const LATEST_STORE_VERSION: u32 = 5;

const V4_TABLES: [&str; 6] = [
    "aggregates",
    "evidence_events",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
const V5_TABLES: [&str; 10] = [
    "aggregates",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "evidence_events",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

const V4_BASE_SCHEMA_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE IF NOT EXISTS store_meta (
   key TEXT PRIMARY KEY,
   value TEXT NOT NULL
 ) STRICT;
 CREATE TABLE IF NOT EXISTS payloads (
   content_hash TEXT NOT NULL,
   kind TEXT NOT NULL,
   schema_version TEXT NOT NULL,
   byte_count INTEGER NOT NULL CHECK(byte_count >= 0),
   key_version INTEGER NOT NULL CHECK(key_version >= 1),
   created_at TEXT NOT NULL,
   PRIMARY KEY (content_hash, kind, schema_version)
 ) STRICT;
 CREATE TABLE IF NOT EXISTS aggregates (
   aggregate_type TEXT NOT NULL,
   aggregate_id TEXT NOT NULL,
   version INTEGER NOT NULL CHECK(version >= 1),
   state_json TEXT NOT NULL,
   updated_at TEXT NOT NULL,
   PRIMARY KEY (aggregate_type, aggregate_id)
 ) STRICT;
 CREATE TABLE IF NOT EXISTS evidence_events (
   event_id TEXT PRIMARY KEY,
   stream_id TEXT NOT NULL,
   sequence INTEGER NOT NULL CHECK(sequence >= 1),
   event_type TEXT NOT NULL,
   payload_hash TEXT NOT NULL,
   payload_ref TEXT,
   previous_event_hash TEXT,
   event_hash TEXT NOT NULL,
   correlation_id TEXT NOT NULL,
   causation_id TEXT,
   redaction_level TEXT NOT NULL,
   retention_class TEXT NOT NULL,
   occurred_at TEXT NOT NULL,
   UNIQUE(stream_id, sequence)
 ) STRICT;
 CREATE TABLE IF NOT EXISTS outbox (
   outbox_id TEXT PRIMARY KEY,
   event_id TEXT NOT NULL REFERENCES evidence_events(event_id),
   created_at TEXT NOT NULL,
   dispatched_at TEXT
 ) STRICT;
 CREATE UNIQUE INDEX outbox_event_once ON outbox(event_id);
 CREATE TABLE IF NOT EXISTS spec_consumptions (
   consumption_id TEXT PRIMARY KEY,
   spec_hash TEXT NOT NULL UNIQUE,
   candidate_hash TEXT NOT NULL,
   nonce_hash TEXT NOT NULL,
   audience_hash TEXT NOT NULL,
   execution_id TEXT NOT NULL,
   consumption_hash TEXT NOT NULL UNIQUE,
   record_json TEXT NOT NULL,
   consumed_at TEXT NOT NULL
 ) STRICT;
 PRAGMA user_version = 4;
 COMMIT;";

const V4_TO_V5_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_method_artifacts (
   content_hash TEXT PRIMARY KEY,
   content_kind TEXT NOT NULL,
   content_schema_version TEXT NOT NULL,
   expectation_id TEXT NOT NULL,
   artifact_kind TEXT NOT NULL,
   media_type TEXT NOT NULL,
   content_schema_hash TEXT,
   evidence_class TEXT NOT NULL,
   session_id TEXT NOT NULL,
   owner_scope_ref TEXT NOT NULL,
   project_id TEXT NOT NULL,
   run_id TEXT NOT NULL,
   authority_id TEXT NOT NULL,
   binding_ordinal INTEGER NOT NULL CHECK(binding_ordinal >= 1),
   binding_hash TEXT NOT NULL,
   decision_id TEXT NOT NULL,
   invocation_id TEXT NOT NULL,
   FOREIGN KEY(content_hash, content_kind, content_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE TABLE bmad_method_sessions (
   session_id TEXT PRIMARY KEY,
   owner_scope_ref TEXT NOT NULL,
   project_id TEXT NOT NULL,
   run_id TEXT NOT NULL,
   authority_id TEXT NOT NULL,
   version INTEGER NOT NULL CHECK(version >= 1),
   state TEXT NOT NULL,
   state_content_hash TEXT NOT NULL,
   state_kind TEXT NOT NULL,
   state_schema_version TEXT NOT NULL,
   state_byte_count INTEGER NOT NULL CHECK(state_byte_count >= 0),
   state_key_version INTEGER NOT NULL CHECK(state_key_version >= 1),
   updated_at TEXT NOT NULL,
   UNIQUE(owner_scope_ref, project_id, run_id, session_id),
   FOREIGN KEY(state_content_hash, state_kind, state_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE INDEX bmad_method_sessions_scope
   ON bmad_method_sessions(owner_scope_ref, project_id, run_id, session_id);
 CREATE TABLE bmad_method_checkpoints (
   checkpoint_id TEXT PRIMARY KEY,
   session_id TEXT NOT NULL REFERENCES bmad_method_sessions(session_id),
   turn_ordinal INTEGER NOT NULL CHECK(turn_ordinal >= 1),
   checkpoint_hash TEXT NOT NULL,
   state_content_hash TEXT NOT NULL,
   state_kind TEXT NOT NULL,
   state_schema_version TEXT NOT NULL,
   recorded_at TEXT NOT NULL,
   UNIQUE(session_id, turn_ordinal),
   FOREIGN KEY(state_content_hash, state_kind, state_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE INDEX bmad_method_checkpoints_session
   ON bmad_method_checkpoints(session_id, turn_ordinal);
 CREATE TABLE bmad_method_decision_consumptions (
   consumption_id TEXT PRIMARY KEY,
   decision_id TEXT NOT NULL UNIQUE,
   invocation_id TEXT NOT NULL UNIQUE,
   idempotency_key TEXT NOT NULL,
   session_id TEXT NOT NULL REFERENCES bmad_method_sessions(session_id),
   owner_scope_ref TEXT NOT NULL,
   project_id TEXT NOT NULL,
   run_id TEXT NOT NULL,
   authority_id TEXT NOT NULL,
   receipt_json TEXT NOT NULL,
   receipt_hash TEXT NOT NULL,
   consumed_at TEXT NOT NULL,
   UNIQUE(session_id, idempotency_key)
 ) STRICT;
 CREATE INDEX bmad_method_consumptions_scope
   ON bmad_method_decision_consumptions(
     owner_scope_ref, project_id, run_id, authority_id, session_id
   );
 PRAGMA user_version = 5;
 COMMIT;";

pub(crate) fn migrate(connection: &Connection) -> Result<(), StoreError> {
    loop {
        let version = schema_version(connection)?;
        match version {
            0 => {
                if !store_table_names(connection)?.is_empty() {
                    return Err(StoreError::Inconsistent);
                }
                connection.execute_batch(V4_BASE_SCHEMA_SQL)?;
            }
            1 | 2 => migrate_legacy_consumptions_to_v4(connection)?,
            3 => migrate_outbox_index_to_v4(connection)?,
            4 => {
                require_store_tables(connection, &V4_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                connection.execute_batch(V4_TO_V5_SQL)?;
            }
            LATEST_STORE_VERSION => {
                require_store_tables(connection, &V5_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                return Ok(());
            }
            _ => return Err(StoreError::UnsupportedStoreVersion),
        }
    }
}

pub(crate) fn schema_version(connection: &Connection) -> Result<u32, StoreError> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

pub(crate) fn store_table_names(connection: &Connection) -> Result<HashSet<String>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()?;
    Ok(names)
}

fn migrate_legacy_consumptions_to_v4(connection: &Connection) -> Result<(), StoreError> {
    require_store_tables(connection, &V4_TABLES)?;
    reject_duplicate_outbox_events(connection)?;
    let existing_consumptions: u64 =
        connection.query_row("SELECT COUNT(*) FROM spec_consumptions", [], |row| {
            row.get(0)
        })?;
    if existing_consumptions != 0 {
        return Err(StoreError::Inconsistent);
    }
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         DROP TABLE spec_consumptions;
         CREATE TABLE spec_consumptions (
           consumption_id TEXT PRIMARY KEY,
           spec_hash TEXT NOT NULL UNIQUE,
           candidate_hash TEXT NOT NULL,
           nonce_hash TEXT NOT NULL,
           audience_hash TEXT NOT NULL,
           execution_id TEXT NOT NULL,
           consumption_hash TEXT NOT NULL UNIQUE,
           record_json TEXT NOT NULL,
           consumed_at TEXT NOT NULL
         ) STRICT;
         CREATE UNIQUE INDEX outbox_event_once ON outbox(event_id);
         PRAGMA user_version = 4;
         COMMIT;",
    )?;
    Ok(())
}

fn migrate_outbox_index_to_v4(connection: &Connection) -> Result<(), StoreError> {
    require_store_tables(connection, &V4_TABLES)?;
    reject_duplicate_outbox_events(connection)?;
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE UNIQUE INDEX outbox_event_once ON outbox(event_id);
         PRAGMA user_version = 4;
         COMMIT;",
    )?;
    Ok(())
}

fn require_store_tables(connection: &Connection, required: &[&str]) -> Result<(), StoreError> {
    let expected = required
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<HashSet<_>>();
    if store_table_names(connection)? != expected {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn reject_duplicate_outbox_events(connection: &Connection) -> Result<(), StoreError> {
    let duplicate_count: u64 = connection.query_row(
        "SELECT COUNT(*) FROM (
           SELECT event_id FROM outbox GROUP BY event_id HAVING COUNT(*) > 1
         )",
        [],
        |row| row.get(0),
    )?;
    if duplicate_count != 0 {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

pub(crate) fn require_outbox_event_uniqueness(connection: &Connection) -> Result<(), StoreError> {
    let index_flags = connection
        .query_row(
            "SELECT \"unique\", partial
             FROM pragma_index_list('outbox')
             WHERE name = 'outbox_event_once'",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    if index_flags != Some((1, 0)) {
        return Err(StoreError::Inconsistent);
    }
    let mut statement = connection
        .prepare("SELECT name FROM pragma_index_info('outbox_event_once') ORDER BY seqno")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if columns != vec!["event_id".to_owned()] {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}
