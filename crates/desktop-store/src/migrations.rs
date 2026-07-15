use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};

use super::StoreError;

pub(crate) const LATEST_STORE_VERSION: u32 = 8;

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
const V6_TABLES: [&str; 13] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
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
const V7_TABLES: [&str; 14] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
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
const V8_TABLES: [&str; 15] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_help_run_creations",
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

const V5_TO_V6_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_builder_drafts (
   draft_id TEXT PRIMARY KEY,
   owner_scope_ref TEXT NOT NULL,
   project_id TEXT NOT NULL,
   authoring_session_id TEXT NOT NULL,
   authority_id TEXT NOT NULL,
   version INTEGER NOT NULL CHECK(version >= 1),
   state TEXT NOT NULL,
   state_content_hash TEXT NOT NULL,
   state_kind TEXT NOT NULL,
   state_schema_version TEXT NOT NULL,
   state_byte_count INTEGER NOT NULL CHECK(state_byte_count >= 0),
   state_key_version INTEGER NOT NULL CHECK(state_key_version >= 1),
   updated_at TEXT NOT NULL,
   UNIQUE(owner_scope_ref, project_id, authoring_session_id, draft_id),
   FOREIGN KEY(state_content_hash, state_kind, state_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE INDEX bmad_builder_drafts_scope
   ON bmad_builder_drafts(owner_scope_ref, project_id, authoring_session_id, draft_id);
 CREATE TABLE bmad_builder_revisions (
   revision_id TEXT PRIMARY KEY,
   draft_id TEXT NOT NULL REFERENCES bmad_builder_drafts(draft_id),
   ordinal INTEGER NOT NULL CHECK(ordinal >= 1),
   revision_hash TEXT NOT NULL,
   source_inventory_hash TEXT NOT NULL,
   host_inventory_hash TEXT NOT NULL,
   content_hash TEXT NOT NULL,
   content_kind TEXT NOT NULL,
   content_schema_version TEXT NOT NULL,
   recorded_at TEXT NOT NULL,
   UNIQUE(draft_id, revision_id),
   UNIQUE(draft_id, ordinal),
   UNIQUE(draft_id, revision_hash),
   FOREIGN KEY(content_hash, content_kind, content_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE INDEX bmad_builder_revisions_draft
   ON bmad_builder_revisions(draft_id, ordinal);
 CREATE TABLE bmad_builder_analyses (
   analysis_id TEXT PRIMARY KEY,
   draft_id TEXT NOT NULL,
   revision_id TEXT NOT NULL,
   revision_hash TEXT NOT NULL,
   analysis_kind TEXT NOT NULL,
   context_decision_id TEXT UNIQUE,
   invocation_id TEXT UNIQUE,
   decision_consumption_hash TEXT UNIQUE,
   content_hash TEXT NOT NULL,
   content_kind TEXT NOT NULL,
   content_schema_version TEXT NOT NULL,
   recorded_at TEXT NOT NULL,
   FOREIGN KEY(draft_id, revision_id)
     REFERENCES bmad_builder_revisions(draft_id, revision_id),
   FOREIGN KEY(content_hash, content_kind, content_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE INDEX bmad_builder_analyses_draft
   ON bmad_builder_analyses(draft_id, revision_id, analysis_id);
 PRAGMA user_version = 6;
 COMMIT;";

const V6_TO_V7_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_builder_analysis_decisions (
   decision_id TEXT PRIMARY KEY,
   draft_id TEXT NOT NULL REFERENCES bmad_builder_drafts(draft_id),
   revision_id TEXT NOT NULL,
   revision_hash TEXT NOT NULL,
   scope_hash TEXT NOT NULL,
   invocation_id TEXT NOT NULL UNIQUE,
   decision_hash TEXT NOT NULL UNIQUE,
   disposition TEXT NOT NULL CHECK(disposition IN ('pending', 'consumed', 'invalidated')),
   content_hash TEXT NOT NULL,
   content_kind TEXT NOT NULL,
   content_schema_version TEXT NOT NULL,
   recorded_at TEXT NOT NULL,
   consumed_analysis_id TEXT UNIQUE,
   consumption_id TEXT UNIQUE,
   consumption_hash TEXT UNIQUE,
   consumed_at TEXT,
   invalidation_reason TEXT,
   invalidation_version INTEGER CHECK(invalidation_version IS NULL OR invalidation_version >= 1),
   invalidation_hash TEXT UNIQUE,
   invalidated_at TEXT,
   FOREIGN KEY(draft_id, revision_id)
     REFERENCES bmad_builder_revisions(draft_id, revision_id),
   FOREIGN KEY(content_hash, content_kind, content_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version),
   CHECK (
     (disposition = 'pending'
       AND consumed_analysis_id IS NULL AND consumption_id IS NULL
       AND consumption_hash IS NULL AND consumed_at IS NULL
       AND invalidation_reason IS NULL AND invalidation_version IS NULL
       AND invalidation_hash IS NULL AND invalidated_at IS NULL)
     OR
     (disposition = 'consumed'
       AND consumed_analysis_id IS NOT NULL AND consumption_id IS NOT NULL
       AND consumption_hash IS NOT NULL AND consumed_at IS NOT NULL
       AND invalidation_reason IS NULL AND invalidation_version IS NULL
       AND invalidation_hash IS NULL AND invalidated_at IS NULL)
     OR
     (disposition = 'invalidated'
       AND consumed_analysis_id IS NULL AND consumption_id IS NULL
       AND consumption_hash IS NULL AND consumed_at IS NULL
       AND invalidation_reason IS NOT NULL AND invalidation_version IS NOT NULL
       AND invalidation_hash IS NOT NULL AND invalidated_at IS NOT NULL)
   )
 ) STRICT;
 CREATE INDEX bmad_builder_analysis_decisions_draft
   ON bmad_builder_analysis_decisions(draft_id, revision_id, decision_id);
 PRAGMA user_version = 7;
 COMMIT;";

const V7_TO_V8_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_help_run_creations (
   owner_scope_ref TEXT NOT NULL,
   installation_id TEXT NOT NULL,
   request_id TEXT NOT NULL,
   request_fingerprint TEXT NOT NULL,
   session_id TEXT NOT NULL UNIQUE REFERENCES bmad_method_sessions(session_id),
   project_id TEXT NOT NULL,
   run_id TEXT NOT NULL,
   authority_id TEXT NOT NULL,
   authority_epoch INTEGER NOT NULL
     CHECK(authority_epoch >= 1 AND authority_epoch <= 9007199254740991),
   local_store_id TEXT NOT NULL,
   workspace_id TEXT NOT NULL,
   workspace_grant_epoch INTEGER NOT NULL
     CHECK(workspace_grant_epoch >= 1 AND workspace_grant_epoch <= 9007199254740991),
   workspace_catalog_version INTEGER NOT NULL
     CHECK(workspace_catalog_version >= 1 AND workspace_catalog_version <= 9007199254740991),
   workspace_root_identity_hash TEXT NOT NULL,
   capability_catalog_hash TEXT NOT NULL,
   foundation_binding_hash TEXT NOT NULL,
   intent_hash TEXT NOT NULL,
   accepted_at INTEGER NOT NULL
     CHECK(accepted_at >= 0 AND accepted_at <= 9007199254740991),
   PRIMARY KEY(owner_scope_ref, installation_id, request_id)
 ) STRICT;
 CREATE INDEX bmad_help_run_creations_scope
   ON bmad_help_run_creations(owner_scope_ref, project_id, run_id, session_id);
 PRAGMA user_version = 8;
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
            5 => {
                require_store_tables(connection, &V5_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                connection.execute_batch(V5_TO_V6_SQL)?;
            }
            6 => {
                require_store_tables(connection, &V6_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                reject_untrusted_v6_model_analysis_history(connection)?;
                connection.execute_batch(V6_TO_V7_SQL)?;
            }
            7 => {
                require_store_tables(connection, &V7_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                connection.execute_batch(V7_TO_V8_SQL)?;
            }
            LATEST_STORE_VERSION => {
                require_store_tables(connection, &V8_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                return Ok(());
            }
            _ => return Err(StoreError::UnsupportedStoreVersion),
        }
    }
}

fn reject_untrusted_v6_model_analysis_history(connection: &Connection) -> Result<(), StoreError> {
    let model_analysis_count: u64 = connection.query_row(
        "SELECT COUNT(*) FROM bmad_builder_analyses WHERE analysis_kind = 'model_lens'",
        [],
        |row| row.get(0),
    )?;
    if model_analysis_count == 0 {
        Ok(())
    } else {
        Err(StoreError::Inconsistent)
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
