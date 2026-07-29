use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};

use super::StoreError;

pub(crate) const LATEST_STORE_VERSION: u32 = 23;

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
const V9_TABLES: [&str; 15] = V8_TABLES;
const V10_TABLES: [&str; 18] = [
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
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
const V11_TABLES: [&str; 20] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
pub(crate) const V12_TABLES: [&str; 22] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
pub(crate) const V13_TABLES: [&str; 23] = [
    "aggregates",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
/// v14 adds a column to an existing table, so the table set is unchanged.
pub(crate) const V14_TABLES: [&str; 23] = V13_TABLES;
pub(crate) const V15_TABLES: [&str; 25] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// The table set is unchanged from v15 — v16 only widens a CHECK constraint.
pub(crate) const V16_TABLES: [&str; 25] = V15_TABLES;

/// v17 adds the quarantine ledger.
pub(crate) const V17_TABLES: [&str; 26] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// Records the payloads boot verification found unreadable, so one damaged
/// record stops taking the whole product offline.
///
/// Before this table an undecryptable payload — a bad sector, an interrupted
/// write, a half-written blob — made `verify_integrity` return `Err`, and
/// `HostState::initialize` turns any such error into global read-only recovery.
/// A single corrupt chat message therefore cost the user every workspace, every
/// capability and every conversation they had. Damage that is confined to one
/// row is now recorded as confined to one row: the payload stays unreadable and
/// every read of it still fails closed, but the store opens.
///
/// Deliberately carries no foreign key to `payloads`. Retention may drop the
/// payload row later, and the fact that a record was found damaged outlives the
/// record — a quarantine entry the store silently forgot would be worse than
/// none, because there would be no way to learn what was lost.
///
/// Nothing surfaces these rows yet: `quarantined_payloads()` has no caller
/// outside this crate's tests. The ledger is durable and correct; reaching the
/// user needs a host command and a projection field that do not exist.
///
/// `reason` is a closed vocabulary so that it *can* be surfaced safely: it says
/// which failure mode was seen, never what the payload held.
const V16_TO_V17_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE payload_quarantine (
   content_hash TEXT NOT NULL,
   kind TEXT NOT NULL,
   schema_version TEXT NOT NULL,
   reason TEXT NOT NULL
     CHECK(reason IN ('authentication', 'unsupported_payload', 'unreadable')),
   detected_at TEXT NOT NULL,
   PRIMARY KEY(content_hash, kind, schema_version)
 ) STRICT;
 PRAGMA user_version = 17;
 COMMIT;";

/// v18 adds a column to an existing table, so the table set is unchanged.
pub(crate) const V18_TABLES: [&str; 26] = V17_TABLES;

/// Verification state moves onto the row it describes.
///
/// The boot sweep used to skip anything at or below a single persisted
/// watermark, and that watermark was a **wall-clock** key built from
/// `created_at`. Two ways that permanently disables detection:
///
/// * a payload written after the clock steps *backwards* sorts below the mark
///   and is skipped by every later boot -- and this codebase has already been
///   bitten once by a backwards step, in the IPC rate limiter;
/// * worse, a payload written while the clock is *ahead* -- a fast RTC, a VM
///   restored from a snapshot -- drags the mark into the future, and everything
///   written until real time catches up is skipped.
///
/// A per-row mark is monotonic in nothing, so it is unaffected by the clock, by
/// insertion order, and by rowid reuse after a retention delete (which a naive
/// rowid cursor would have reintroduced, because `retention.rs` does delete
/// payloads and `SQLite` hands the freed rowid to the next insert).
///
/// The old key is dropped rather than reinterpreted: an unmarked payload is
/// simply verified again, so the first boot after this migration is one full
/// sweep. That is the safe direction -- the alternative is silently trusting a
/// value whose meaning changed.
const V17_TO_V18_SQL: &str = "BEGIN IMMEDIATE;
 ALTER TABLE payloads ADD COLUMN verified_at TEXT;
 DELETE FROM store_meta WHERE key = 'payload_verification_watermark';
 PRAGMA user_version = 18;
 COMMIT;";

/// v19 adds the model catalog snapshot and the model selection history.
pub(crate) const V19_TABLES: [&str; 28] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "model_catalog_snapshots",
    "model_profile_selections",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// The one durable BMAD skill lifecycle (ENG-05).
///
/// `bmad_sessions` holds exactly one row per session: the current state, its
/// optimistic `version`, the evidence-chain head, and the bound source,
/// package, config, customization, model, task, workspace, and policy
/// identities. `bmad_session_events` is append-only — a row per accepted
/// transition, keyed so the same version cannot be written twice. Restart
/// recovery reads the row and replays nothing: the row *is* the accepted
/// state, and the event log explains how it got there.
pub(crate) const V19_TO_V20_SQL: &str = "\
 BEGIN IMMEDIATE;
 CREATE TABLE bmad_sessions (
   session_id TEXT PRIMARY KEY,
   capability_id TEXT NOT NULL CHECK(length(capability_id) > 0),
   workspace_id TEXT NOT NULL CHECK(length(workspace_id) > 0),
   state TEXT NOT NULL CHECK(state IN (
     'created', 'resolving', 'input_required', 'ready', 'running',
     'awaiting_approval', 'paused', 'validating', 'completed', 'failed',
     'cancelled')),
   version INTEGER NOT NULL CHECK(version >= 1 AND version <= 1048576),
   evidence_hash TEXT NOT NULL
     CHECK(length(evidence_hash) = 71 AND evidence_hash GLOB 'sha256:*'),
   source_id TEXT NOT NULL CHECK(length(source_id) > 0),
   package_id TEXT NOT NULL CHECK(length(package_id) > 0),
   config_hash TEXT NOT NULL
     CHECK(length(config_hash) = 71 AND config_hash GLOB 'sha256:*'),
   customization_hash TEXT NOT NULL
     CHECK(length(customization_hash) = 71 AND customization_hash GLOB 'sha256:*'),
   model_id TEXT NOT NULL CHECK(length(model_id) > 0),
   task_id TEXT NOT NULL CHECK(length(task_id) > 0),
   policy_hash TEXT NOT NULL
     CHECK(length(policy_hash) = 71 AND policy_hash GLOB 'sha256:*'),
   session_json TEXT NOT NULL CHECK(length(session_json) > 0),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   updated_at_ms INTEGER NOT NULL
     CHECK(updated_at_ms >= 0 AND updated_at_ms <= 9007199254740991),
   CHECK(updated_at_ms >= created_at_ms)
 ) STRICT;
 CREATE INDEX bmad_sessions_by_workspace
   ON bmad_sessions(workspace_id, updated_at_ms DESC, session_id);
 CREATE TABLE bmad_session_events (
   session_id TEXT NOT NULL REFERENCES bmad_sessions(session_id),
   to_version INTEGER NOT NULL CHECK(to_version >= 2 AND to_version <= 1048576),
   from_version INTEGER NOT NULL CHECK(from_version >= 1),
   command TEXT NOT NULL CHECK(length(command) > 0),
   from_state TEXT NOT NULL CHECK(length(from_state) > 0),
   to_state TEXT NOT NULL CHECK(length(to_state) > 0),
   previous_evidence_hash TEXT NOT NULL
     CHECK(length(previous_evidence_hash) = 71
       AND previous_evidence_hash GLOB 'sha256:*'),
   evidence_hash TEXT NOT NULL
     CHECK(length(evidence_hash) = 71 AND evidence_hash GLOB 'sha256:*'),
   recorded_at_ms INTEGER NOT NULL
     CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
   CHECK(to_version = from_version + 1),
   PRIMARY KEY(session_id, to_version)
 ) STRICT;
 CREATE INDEX bmad_session_events_by_session
   ON bmad_session_events(session_id, to_version);
 PRAGMA user_version = 20;
 COMMIT;";

pub(crate) const V20_TABLES: [&str; 30] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "bmad_session_events",
    "bmad_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "model_catalog_snapshots",
    "model_profile_selections",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// Durable artifact lifecycle and provenance (ENG-07).
///
/// `bmad_artifacts` holds exactly one row per artifact: the current lifecycle
/// state, its optimistic `revision`, the split between raw model output and
/// validated content, the validation block, and the lineage that ties it back
/// to the source, template, and inputs that produced it. `bmad_artifact_events`
/// is append-only — a row per accepted transition, keyed so the same revision
/// cannot be written twice. Restart recovery reads the row and replays
/// nothing, mirroring `bmad_sessions`/`bmad_session_events` (v19->v20): the
/// row *is* the accepted state, and the event log only explains how it got
/// there.
pub(crate) const V20_TO_V21_SQL: &str = "\
 BEGIN IMMEDIATE;
 CREATE TABLE bmad_artifacts (
   artifact_id TEXT PRIMARY KEY,
   session_id TEXT NOT NULL CHECK(length(session_id) > 0),
   workspace_id TEXT NOT NULL CHECK(length(workspace_id) > 0),
   state TEXT NOT NULL CHECK(state IN (
     'draft', 'revised', 'validated', 'approved', 'written', 'superseded',
     'rolled_back', 'rejected')),
   sequence INTEGER NOT NULL CHECK(sequence >= 1 AND sequence <= 1048576),
   revision INTEGER NOT NULL CHECK(revision >= 1 AND revision <= 65536),
   content_hash TEXT NOT NULL
     CHECK(length(content_hash) = 71 AND content_hash GLOB 'sha256:*'),
   raw_output_hash TEXT
     CHECK(raw_output_hash IS NULL
       OR (length(raw_output_hash) = 71 AND raw_output_hash GLOB 'sha256:*')),
   evidence_hash TEXT NOT NULL
     CHECK(length(evidence_hash) = 71 AND evidence_hash GLOB 'sha256:*'),
   source_id TEXT NOT NULL CHECK(length(source_id) > 0),
   template_hash TEXT NOT NULL
     CHECK(length(template_hash) = 71 AND template_hash GLOB 'sha256:*'),
   inputs_hash TEXT NOT NULL
     CHECK(length(inputs_hash) = 71 AND inputs_hash GLOB 'sha256:*'),
   artifact_json TEXT NOT NULL CHECK(length(artifact_json) > 0),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   updated_at_ms INTEGER NOT NULL
     CHECK(updated_at_ms >= 0 AND updated_at_ms <= 9007199254740991),
   CHECK(updated_at_ms >= created_at_ms)
 ) STRICT;
 CREATE INDEX bmad_artifacts_by_session
   ON bmad_artifacts(session_id, updated_at_ms DESC, artifact_id);
 CREATE INDEX bmad_artifacts_by_workspace
   ON bmad_artifacts(workspace_id, updated_at_ms DESC, artifact_id);
 CREATE TABLE bmad_artifact_events (
   artifact_id TEXT NOT NULL REFERENCES bmad_artifacts(artifact_id),
   to_revision INTEGER NOT NULL CHECK(to_revision >= 1 AND to_revision <= 65536),
   from_revision INTEGER NOT NULL CHECK(from_revision >= 1),
   command TEXT NOT NULL CHECK(length(command) > 0),
   from_state TEXT NOT NULL CHECK(length(from_state) > 0),
   to_state TEXT NOT NULL CHECK(length(to_state) > 0),
   recorded_at_ms INTEGER NOT NULL
     CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
   CHECK(to_revision = from_revision + 1),
   PRIMARY KEY(artifact_id, to_revision)
 ) STRICT;
 CREATE INDEX bmad_artifact_events_by_artifact
   ON bmad_artifact_events(artifact_id, to_revision);
 PRAGMA user_version = 21;
 COMMIT;";

pub(crate) const V21_TABLES: [&str; 32] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_artifact_events",
    "bmad_artifacts",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "bmad_session_events",
    "bmad_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "model_catalog_snapshots",
    "model_profile_selections",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// Typed, durable memory entries for resume and Builder memory (ENG-10).
///
/// One row per entry, append-only in the same sense as `bmad_memlog_entries`:
/// a `memlog_entry` row is only ever inserted, never updated. `replaceable`
/// entries (session/persistent facts and compaction summaries) may have
/// their own row's `superseded_by_entry_id` and `entry_json` updated when a
/// compaction commits, but the superseding is always a *new* row referencing
/// the old one — the old row is retired, not deleted, until retention purge.
///
/// `entry_json` carries the full validated contract entry; `content_hash`
/// and `recorded_at_sequence` are duplicated as indexed columns so a reader
/// can detect a row that drifted from what it was indexed under without
/// deserializing every row up front (`StoreError::Inconsistent`).
pub(crate) const V21_TO_V22_SQL: &str = "\
 BEGIN IMMEDIATE;
 CREATE TABLE bmad_memory_entries (
   entry_id TEXT PRIMARY KEY,
   workspace_id TEXT NOT NULL CHECK(length(workspace_id) > 0),
   memory_class TEXT NOT NULL CHECK(memory_class IN (
     'session_fact', 'persistent_fact', 'memlog_entry', 'compaction_summary',
     'sanctum_entry')),
   scope_kind TEXT NOT NULL CHECK(scope_kind IN (
     'session', 'project', 'user', 'installed_agent')),
   scope_session_id TEXT,
   scope_workspace_id TEXT,
   scope_installed_agent_id TEXT,
   origin TEXT NOT NULL CHECK(origin IN (
     'user_supplied', 'skill_declared', 'model_summarized', 'host_observed')),
   sensitivity TEXT NOT NULL CHECK(sensitivity IN ('ordinary', 'personal', 'restricted')),
   retention TEXT NOT NULL CHECK(retention IN ('session', 'bounded', 'pinned')),
   mutability TEXT NOT NULL CHECK(mutability IN ('append_only', 'replaceable')),
   content_hash TEXT NOT NULL
     CHECK(length(content_hash) = 71 AND content_hash GLOB 'sha256:*'),
   recorded_at_sequence INTEGER NOT NULL
     CHECK(recorded_at_sequence >= 0 AND recorded_at_sequence <= 1048576),
   superseded_by_entry_id TEXT,
   entry_json TEXT NOT NULL CHECK(length(entry_json) > 0),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991)
 ) STRICT;
 CREATE INDEX bmad_memory_entries_by_workspace_scope
   ON bmad_memory_entries(workspace_id, scope_kind, recorded_at_sequence);
 CREATE INDEX bmad_memory_entries_by_retention
   ON bmad_memory_entries(workspace_id, retention);
 PRAGMA user_version = 22;
 COMMIT;";

pub(crate) const V22_TABLES: [&str; 33] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_artifact_events",
    "bmad_artifacts",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_memory_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "bmad_session_events",
    "bmad_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "model_catalog_snapshots",
    "model_profile_selections",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// The package promotion lifecycle (ENG-15): validated through uninstalled.
///
/// `bmad_builder_lifecycles` is the accepted-state row, one per package
/// identity (`subject_hash`), advanced the same way `bmad_sessions` is — an
/// optimistic compare-and-swap on `version` that appends the transition's
/// receipt in the same immediate transaction, so a crash between the two
/// never leaves a lifecycle without the evidence that explains its state.
/// `bmad_builder_lifecycle_events` is that append-only receipt log; it
/// exists so "why is this package active" is answered by a row, not by
/// trusting the current state column.
pub(crate) const V22_TO_V23_SQL: &str = "\
 BEGIN IMMEDIATE;
 CREATE TABLE bmad_builder_lifecycles (
   lifecycle_id TEXT PRIMARY KEY,
   draft_id TEXT NOT NULL,
   subject_hash TEXT NOT NULL
     CHECK(length(subject_hash) = 71 AND subject_hash GLOB 'sha256:*'),
   state TEXT NOT NULL CHECK(state IN (
     'draft', 'validated', 'analysed', 'evaluated', 'reviewed', 'approved',
     'staged', 'installed', 'active', 'superseded', 'rolled_back',
     'uninstalled')),
   version INTEGER NOT NULL CHECK(version >= 1),
   state_content_hash TEXT NOT NULL
     CHECK(length(state_content_hash) = 71 AND state_content_hash GLOB 'sha256:*'),
   state_kind TEXT NOT NULL CHECK(length(state_kind) > 0),
   state_schema_version TEXT NOT NULL CHECK(length(state_schema_version) > 0),
   state_byte_count INTEGER NOT NULL CHECK(state_byte_count >= 0),
   state_key_version INTEGER NOT NULL CHECK(state_key_version >= 0),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   updated_at_ms INTEGER NOT NULL
     CHECK(updated_at_ms >= 0 AND updated_at_ms <= 9007199254740991)
 ) STRICT;
 CREATE UNIQUE INDEX bmad_builder_lifecycles_by_subject
   ON bmad_builder_lifecycles(subject_hash);
 CREATE TABLE bmad_builder_lifecycle_events (
   lifecycle_id TEXT NOT NULL REFERENCES bmad_builder_lifecycles(lifecycle_id),
   to_version INTEGER NOT NULL CHECK(to_version >= 2),
   from_version INTEGER NOT NULL CHECK(from_version >= 1),
   promotion_id TEXT NOT NULL,
   from_state TEXT NOT NULL,
   to_state TEXT NOT NULL,
   stage TEXT NOT NULL CHECK(stage IN (
     'validation', 'analysis', 'evaluation', 'review', 'approval', 'staging',
     'installation', 'activation', 'rollback', 'uninstallation')),
   receipt_hash TEXT NOT NULL
     CHECK(length(receipt_hash) = 71 AND receipt_hash GLOB 'sha256:*'),
   event_hash TEXT NOT NULL
     CHECK(length(event_hash) = 71 AND event_hash GLOB 'sha256:*'),
   recorded_at_ms INTEGER NOT NULL
     CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
   CHECK(to_version = from_version + 1),
   PRIMARY KEY(lifecycle_id, to_version)
 ) STRICT;
 CREATE INDEX bmad_builder_lifecycle_events_by_promotion
   ON bmad_builder_lifecycle_events(promotion_id);
 PRAGMA user_version = 23;
 COMMIT;";

pub(crate) const V23_TABLES: [&str; 35] = [
    "aggregates",
    "bmad_architecture_session_steps",
    "bmad_architecture_sessions",
    "bmad_artifact_events",
    "bmad_artifacts",
    "bmad_builder_analyses",
    "bmad_builder_analysis_decisions",
    "bmad_builder_drafts",
    "bmad_builder_lifecycle_events",
    "bmad_builder_lifecycles",
    "bmad_builder_revisions",
    "bmad_capability_results",
    "bmad_capability_runs",
    "bmad_help_run_creations",
    "bmad_memlog_entries",
    "bmad_memory_entries",
    "bmad_method_artifacts",
    "bmad_method_checkpoints",
    "bmad_method_decision_consumptions",
    "bmad_method_sessions",
    "bmad_session_events",
    "bmad_sessions",
    "chat_messages",
    "chat_sessions",
    "effect_journals",
    "evidence_events",
    "execution_checkpoints",
    "execution_results",
    "model_catalog_snapshots",
    "model_profile_selections",
    "outbox",
    "payload_quarantine",
    "payloads",
    "spec_consumptions",
    "store_meta",
];

/// Which model answers, and which catalog said so (FND-041).
///
/// Two tables, because they are two different authorities with two different
/// lifetimes.
///
/// `model_catalog_snapshots` is **metadata only**. It deliberately carries no
/// profile, no display text, and no `ETag`. That is the whole point: a restarted
/// desktop can say "you were last on catalog `v7`, generated at `T`, expiring at
/// `U`" and render that as stale while it refetches, but it cannot reconstruct a
/// catalog from local state and it cannot authorize anything. A cached catalog
/// that could authorize a request is exactly the failure this shape forbids —
/// there is nothing here to authorize with. It carries no validator either, so a
/// restart always refetches unconditionally rather than letting month-old local
/// state shape a conditional request.
///
/// `model_profile_selections` is the user's own history. It stores only the
/// stable organization-chosen identifiers and the identity of the catalog the
/// choice was made from. A deployment name, an endpoint, a subscription, or a
/// credential is not merely absent — there is no column that could hold one, so
/// no future write can introduce one without a migration and a review.
///
/// Both are scoped by `workspace_id` like `chat_sessions` and
/// `bmad_capability_runs`, each with a matching index, because a workspace is
/// this product's unit of isolation and a model chosen in one workspace must not
/// silently become the model of another.
///
/// `model_profile_selections_one_current` is a *partial* unique index rather
/// than a flag the writer promises to maintain: at most one row per workspace
/// may be current, and the database enforces it even if a future writer forgets
/// to demote the previous one. History rows survive demotion, which is what lets
/// "the profile you had selected is gone" be answered with the next best prior
/// choice instead of an arbitrary one.
const V18_TO_V19_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE model_catalog_snapshots (
   workspace_id TEXT NOT NULL CHECK(length(workspace_id) > 0),
   subject_hash TEXT NOT NULL CHECK(length(subject_hash) > 0),
   catalog_version TEXT NOT NULL
     CHECK(length(catalog_version) >= 5 AND length(catalog_version) <= 40),
   catalog_hash TEXT NOT NULL
     CHECK(length(catalog_hash) = 71 AND catalog_hash GLOB 'sha256:*'),
   catalog_state TEXT NOT NULL CHECK(catalog_state IN ('fresh', 'stale_usable')),
   profile_count INTEGER NOT NULL CHECK(profile_count >= 0 AND profile_count <= 64),
   default_profile_id TEXT
     CHECK(default_profile_id IS NULL
       OR (length(default_profile_id) >= 3 AND length(default_profile_id) <= 64)),
   generated_at_ms INTEGER NOT NULL
     CHECK(generated_at_ms >= 0 AND generated_at_ms <= 9007199254740991),
   expires_at_ms INTEGER NOT NULL
     CHECK(expires_at_ms >= 0 AND expires_at_ms <= 9007199254740991),
   verified_at_ms INTEGER NOT NULL
     CHECK(verified_at_ms >= 0 AND verified_at_ms <= 9007199254740991),
   CHECK(expires_at_ms > generated_at_ms),
   PRIMARY KEY(workspace_id, subject_hash)
 ) STRICT;
 CREATE INDEX model_catalog_snapshots_by_workspace
   ON model_catalog_snapshots(workspace_id, verified_at_ms, subject_hash);
 CREATE TABLE model_profile_selections (
   workspace_id TEXT NOT NULL CHECK(length(workspace_id) > 0),
   profile_id TEXT NOT NULL CHECK(length(profile_id) >= 3 AND length(profile_id) <= 64),
   variant_id TEXT NOT NULL CHECK(length(variant_id) >= 2 AND length(variant_id) <= 32),
   catalog_version TEXT NOT NULL
     CHECK(length(catalog_version) >= 5 AND length(catalog_version) <= 40),
   catalog_hash TEXT NOT NULL
     CHECK(length(catalog_hash) = 71 AND catalog_hash GLOB 'sha256:*'),
   selected_at_ms INTEGER NOT NULL
     CHECK(selected_at_ms >= 0 AND selected_at_ms <= 9007199254740991),
   recorded_at_ms INTEGER NOT NULL
     CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
   is_current INTEGER NOT NULL CHECK(is_current IN (0, 1)),
   PRIMARY KEY(workspace_id, profile_id, variant_id)
 ) STRICT;
 CREATE INDEX model_profile_selections_by_workspace
   ON model_profile_selections(workspace_id, selected_at_ms DESC, profile_id, variant_id);
 CREATE UNIQUE INDEX model_profile_selections_one_current
   ON model_profile_selections(workspace_id) WHERE is_current = 1;
 PRAGMA user_version = 19;
 COMMIT;";

/// Widens `chat_messages.role` to admit the two tool roles (ADR-0009).
///
/// A tool-using turn is `user -> (tool_call, tool_result)* -> assistant`, so the
/// strict two-role alternation the v12 table encoded can no longer be expressed
/// as a `CHECK`. `SQLite` cannot alter a `CHECK` constraint, so the table is
/// rebuilt and copied.
///
/// The copy preserves `ordinal` rather than renumbering: ordinals are the
/// transcript's order and are referenced by the primary key, so renumbering
/// would silently reorder existing conversations. Ordering the insert by
/// `(session_id, ordinal)` keeps the rebuild deterministic.
const V15_TO_V16_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE chat_messages_v16 (
   session_id TEXT NOT NULL REFERENCES chat_sessions(session_id),
   ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
   role TEXT NOT NULL
     CHECK(role IN ('user', 'assistant', 'tool_call', 'tool_result')),
   content_hash TEXT NOT NULL,
   payload_kind TEXT NOT NULL CHECK(payload_kind = 'chat_message'),
   schema_version TEXT NOT NULL CHECK(schema_version = 'sapphirus.chat-message.v1'),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   PRIMARY KEY(session_id, ordinal),
   FOREIGN KEY(content_hash, payload_kind, schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 INSERT INTO chat_messages_v16
   (session_id, ordinal, role, content_hash, payload_kind, schema_version, created_at_ms)
 SELECT session_id, ordinal, role, content_hash, payload_kind, schema_version, created_at_ms
   FROM chat_messages ORDER BY session_id, ordinal;
 DROP TABLE chat_messages;
 ALTER TABLE chat_messages_v16 RENAME TO chat_messages;
 PRAGMA user_version = 16;
 COMMIT;";

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

const V8_TO_V9_SQL: &str = "BEGIN IMMEDIATE;
 DROP INDEX bmad_help_run_creations_scope;
 ALTER TABLE bmad_help_run_creations RENAME TO bmad_help_run_creations_v8;
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
   renderer_projection_state TEXT NOT NULL
     CHECK(renderer_projection_state IN ('legacy_unretained', 'retained')),
   renderer_projection_content_hash TEXT,
   renderer_projection_kind TEXT,
   renderer_projection_schema_version TEXT,
   renderer_projection_byte_count INTEGER,
   renderer_projection_key_version INTEGER,
   renderer_projection_binding_hash TEXT,
   creation_ordinal INTEGER NOT NULL UNIQUE
     CHECK(creation_ordinal >= 1 AND creation_ordinal <= 9007199254740991),
   accepted_at INTEGER NOT NULL
     CHECK(accepted_at >= 0 AND accepted_at <= 9007199254740991),
   PRIMARY KEY(owner_scope_ref, installation_id, request_id),
   FOREIGN KEY(renderer_projection_content_hash, renderer_projection_kind,
               renderer_projection_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version),
   CHECK (
     (renderer_projection_state = 'legacy_unretained'
       AND renderer_projection_content_hash IS NULL
       AND renderer_projection_kind IS NULL
       AND renderer_projection_schema_version IS NULL
       AND renderer_projection_byte_count IS NULL
       AND renderer_projection_key_version IS NULL
       AND renderer_projection_binding_hash IS NULL)
     OR
     (renderer_projection_state = 'retained'
       AND renderer_projection_content_hash IS NOT NULL
       AND renderer_projection_kind = 'bmad_help_run_renderer_projection'
       AND renderer_projection_schema_version = 'sapphirus.bmad-help-run-renderer-projection.v1'
       AND renderer_projection_byte_count >= 1
       AND renderer_projection_byte_count <= 66560
       AND renderer_projection_key_version >= 1
       AND renderer_projection_binding_hash IS NOT NULL)
   )
 ) STRICT;
 INSERT INTO bmad_help_run_creations
   (owner_scope_ref, installation_id, request_id, request_fingerprint,
    session_id, project_id, run_id, authority_id, authority_epoch,
    local_store_id, workspace_id, workspace_grant_epoch, workspace_catalog_version,
    workspace_root_identity_hash, capability_catalog_hash, foundation_binding_hash,
    intent_hash, renderer_projection_state, renderer_projection_content_hash,
    renderer_projection_kind, renderer_projection_schema_version,
    renderer_projection_byte_count, renderer_projection_key_version,
    renderer_projection_binding_hash, creation_ordinal, accepted_at)
 SELECT owner_scope_ref, installation_id, request_id, request_fingerprint,
        session_id, project_id, run_id, authority_id, authority_epoch,
        local_store_id, workspace_id, workspace_grant_epoch, workspace_catalog_version,
        workspace_root_identity_hash, capability_catalog_hash, foundation_binding_hash,
        intent_hash, 'legacy_unretained', NULL, NULL, NULL, NULL, NULL, NULL,
        ROW_NUMBER() OVER (ORDER BY rowid), accepted_at
 FROM bmad_help_run_creations_v8;
 DROP TABLE bmad_help_run_creations_v8;
 CREATE INDEX bmad_help_run_creations_scope
   ON bmad_help_run_creations(owner_scope_ref, project_id, run_id, session_id);
 CREATE INDEX bmad_help_run_creations_workspace_latest
   ON bmad_help_run_creations(
     owner_scope_ref, installation_id, workspace_id, creation_ordinal DESC
   );
 PRAGMA user_version = 9;
 COMMIT;";

const V9_TO_V10_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE execution_checkpoints (
   checkpoint_id TEXT PRIMARY KEY,
   workspace_target_hash TEXT NOT NULL,
   candidate_hash TEXT NOT NULL,
   manifest_hash TEXT NOT NULL UNIQUE,
   entry_count INTEGER NOT NULL CHECK(entry_count >= 0),
   state_content_hash TEXT NOT NULL,
   state_kind TEXT NOT NULL CHECK(state_kind = 'execution_checkpoint'),
   state_schema_version TEXT NOT NULL
     CHECK(state_schema_version = 'sapphirus.local-checkpoint.v1'),
   recorded_at TEXT NOT NULL,
   FOREIGN KEY(state_content_hash, state_kind, state_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 CREATE TABLE effect_journals (
   journal_id TEXT PRIMARY KEY,
   execution_id TEXT NOT NULL UNIQUE,
   checkpoint_id TEXT NOT NULL REFERENCES execution_checkpoints(checkpoint_id),
   candidate_hash TEXT NOT NULL,
   spec_hash TEXT NOT NULL,
   consumption_hash TEXT NOT NULL UNIQUE,
   workspace_id TEXT NOT NULL,
   workspace_grant_epoch INTEGER NOT NULL
     CHECK(workspace_grant_epoch >= 1 AND workspace_grant_epoch <= 9007199254740991),
   state TEXT NOT NULL CHECK(state IN (
     'prepared', 'checkpoint_durable', 'preconditions_verified', 'applying',
     'effects_applied', 'postimages_verified', 'result_recorded', 'completed',
     'recovery_required', 'restoring', 'recovered', 'manual_review')),
   journal_json TEXT NOT NULL,
   created_at TEXT NOT NULL,
   updated_at TEXT NOT NULL
 ) STRICT;
 CREATE INDEX effect_journals_open
   ON effect_journals(state, created_at, journal_id);
 CREATE TABLE execution_results (
   execution_id TEXT PRIMARY KEY,
   journal_id TEXT NOT NULL UNIQUE REFERENCES effect_journals(journal_id),
   checkpoint_id TEXT NOT NULL REFERENCES execution_checkpoints(checkpoint_id),
   candidate_hash TEXT NOT NULL,
   spec_hash TEXT NOT NULL UNIQUE,
   consumption_hash TEXT NOT NULL UNIQUE,
   result_hash TEXT NOT NULL UNIQUE,
   result_json TEXT NOT NULL,
   file_count INTEGER NOT NULL CHECK(file_count >= 1),
   completed_at TEXT NOT NULL
 ) STRICT;
 PRAGMA user_version = 10;
 COMMIT;";

const V10_TO_V11_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_capability_runs (
   run_id TEXT PRIMARY KEY,
   capability_id TEXT NOT NULL,
   workspace_id TEXT NOT NULL,
   instruction_hash TEXT NOT NULL,
   context_manifest_hash TEXT NOT NULL,
   output_schema_id TEXT NOT NULL CHECK(output_schema_id IN (
     'sapphirus.bmad-document-artifact.v1',
     'sapphirus.bmad-governed-change-set.v1',
     'sapphirus.bmad-inactive-builder-draft.v1')),
   consent_evidence_id TEXT NOT NULL UNIQUE,
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991)
 ) STRICT;
 CREATE INDEX bmad_capability_runs_by_workspace
   ON bmad_capability_runs(workspace_id, created_at_ms, run_id);
 CREATE TABLE bmad_capability_results (
   run_id TEXT PRIMARY KEY REFERENCES bmad_capability_runs(run_id),
   result_kind TEXT NOT NULL CHECK(result_kind IN (
     'document_artifact', 'governed_change_set', 'inactive_builder_draft')),
   result_content_hash TEXT NOT NULL,
   result_payload_kind TEXT NOT NULL
     CHECK(result_payload_kind = 'bmad_capability_result'),
   result_schema_version TEXT NOT NULL CHECK(result_schema_version IN (
     'sapphirus.bmad-document-artifact.v1',
     'sapphirus.bmad-governed-change-set.v1',
     'sapphirus.bmad-inactive-builder-draft.v1')),
   recorded_at TEXT NOT NULL,
   FOREIGN KEY(result_content_hash, result_payload_kind, result_schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 PRAGMA user_version = 11;
 COMMIT;";

/// Durable chat transcripts. Chat is the app's default lane, so a conversation
/// must survive a restart like any other local state. Messages are stored as
/// encrypted payloads through the same CAS the other reviewed records use, so
/// a transcript is protected at rest exactly like a capability result.
const V11_TO_V12_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE chat_sessions (
   session_id TEXT PRIMARY KEY,
   workspace_id TEXT NOT NULL,
   agent_code TEXT,
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   updated_at_ms INTEGER NOT NULL
     CHECK(updated_at_ms >= 0 AND updated_at_ms <= 9007199254740991)
 ) STRICT;
 CREATE INDEX chat_sessions_by_workspace
   ON chat_sessions(workspace_id, updated_at_ms, session_id);
 CREATE TABLE chat_messages (
   session_id TEXT NOT NULL REFERENCES chat_sessions(session_id),
   ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
   role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
   content_hash TEXT NOT NULL,
   payload_kind TEXT NOT NULL CHECK(payload_kind = 'chat_message'),
   schema_version TEXT NOT NULL CHECK(schema_version = 'sapphirus.chat-message.v1'),
   created_at_ms INTEGER NOT NULL
     CHECK(created_at_ms >= 0 AND created_at_ms <= 9007199254740991),
   PRIMARY KEY(session_id, ordinal),
   FOREIGN KEY(content_hash, payload_kind, schema_version)
     REFERENCES payloads(content_hash, kind, schema_version)
 ) STRICT;
 PRAGMA user_version = 12;
 COMMIT;";

/// The method's working memory (P14).
///
/// Upstream's memlog is "the dense, chronological record of everything that
/// mattered in a piece of work … It persists ACROSS sessions, so a fresh
/// session can load it and continue", and downstream artifacts are *derived*
/// from it. Two properties drive this shape:
///
/// * **Append-only.** Upstream has "no edit or delete subcommand by design;
///   history is never rewritten." There is no update or delete API over this
///   table, and `sequence` is allocated inside the appending transaction.
/// * **Scoped to a doc workspace, not a run.** Upstream binds a memlog to a run
///   folder that several runs append to over time, which is where the
///   continuity comes from.
///
/// Entry text is short prose the model already produced under review, so it is
/// stored inline rather than through the CAS: unlike a chat message or a
/// capability result there is no separate payload to authenticate.
const V12_TO_V13_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_memlog_entries (
   entry_id TEXT PRIMARY KEY,
   workspace_id TEXT NOT NULL,
   doc_workspace TEXT NOT NULL,
   sequence INTEGER NOT NULL CHECK(sequence >= 0),
   kind TEXT NOT NULL,
   author TEXT,
   text TEXT NOT NULL CHECK(length(text) > 0),
   recorded_at_ms INTEGER NOT NULL
     CHECK(recorded_at_ms >= 0 AND recorded_at_ms <= 9007199254740991),
   UNIQUE(workspace_id, doc_workspace, sequence)
 ) STRICT;
 CREATE INDEX bmad_memlog_entries_by_doc_workspace
   ON bmad_memlog_entries(workspace_id, doc_workspace, sequence);
 PRAGMA user_version = 13;
 COMMIT;";

/// Records *what an entry is about* — for a document capability, the section
/// heading.
///
/// v13 shipped without it and recovered the subject by splitting the rendered
/// text on its separator, which is ambiguous the moment a heading contains one
/// (`Phase 1: Discovery`). A later run then attributed a change or an override
/// to the wrong heading. Carrying the subject explicitly makes the match exact.
///
/// A separate version rather than an edit to `V12_TO_V13_SQL`: a migration that
/// has been committed may already have run somewhere, and rewriting one in
/// place leaves such a store permanently disagreeing with its own version
/// number.
const V13_TO_V14_SQL: &str = "BEGIN IMMEDIATE;
 ALTER TABLE bmad_memlog_entries ADD COLUMN subject TEXT;
 PRAGMA user_version = 14;
 COMMIT;";

/// The architecture progression's durable spine (P14 finding 1).
///
/// The progression lived only in `BmadArchitectureCoordinator.active`, so a
/// restart lost which step a workspace had reached and reopened at step one —
/// while upstream's whole claim for a memlog is that a fresh session can load it
/// and continue.
///
/// Only the *boundary* is recorded here. Each accepted step's artifact is
/// already durable as its own capability run, so a session is a session id, the
/// workspace it reasoned about, and the ordered runs that answered its steps.
/// Restoring re-reads those runs and re-parses them through the same strict
/// constructors that accepted them, so a progression can never resume onto
/// anything the store and the archetype parser have not both accepted.
///
/// `finalized` closes a session: a spine belongs to the workspace it was
/// reasoned about and is never continued, so a finalized session is never
/// resumed and the next run opens a fresh progression.
const V14_TO_V15_SQL: &str = "BEGIN IMMEDIATE;
 CREATE TABLE bmad_architecture_sessions (
   session_id TEXT PRIMARY KEY,
   workspace_id TEXT NOT NULL,
   opened_at_ms INTEGER NOT NULL
     CHECK(opened_at_ms >= 0 AND opened_at_ms <= 9007199254740991),
   finalized INTEGER NOT NULL CHECK(finalized IN (0, 1))
 ) STRICT;
 CREATE TABLE bmad_architecture_session_steps (
   session_id TEXT NOT NULL REFERENCES bmad_architecture_sessions(session_id),
   ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
   run_id TEXT NOT NULL REFERENCES bmad_capability_runs(run_id),
   accepted_at_ms INTEGER NOT NULL
     CHECK(accepted_at_ms >= 0 AND accepted_at_ms <= 9007199254740991),
   PRIMARY KEY(session_id, ordinal),
   UNIQUE(run_id)
 ) STRICT;
 CREATE INDEX bmad_architecture_sessions_by_workspace
   ON bmad_architecture_sessions(workspace_id, finalized, opened_at_ms);
 PRAGMA user_version = 15;
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
            7 => run_migration_step(connection, &V7_TABLES, V7_TO_V8_SQL)?,
            8 => run_migration_step(connection, &V8_TABLES, V8_TO_V9_SQL)?,
            9 => run_migration_step(connection, &V9_TABLES, V9_TO_V10_SQL)?,
            10 => run_migration_step(connection, &V10_TABLES, V10_TO_V11_SQL)?,
            11 => run_migration_step(connection, &V11_TABLES, V11_TO_V12_SQL)?,
            12 => run_migration_step(connection, &V12_TABLES, V12_TO_V13_SQL)?,
            13 => run_migration_step(connection, &V13_TABLES, V13_TO_V14_SQL)?,
            14 => run_migration_step(connection, &V14_TABLES, V14_TO_V15_SQL)?,
            15 => run_migration_step(connection, &V15_TABLES, V15_TO_V16_SQL)?,
            16 => run_migration_step(connection, &V16_TABLES, V16_TO_V17_SQL)?,
            17 => run_migration_step(connection, &V17_TABLES, V17_TO_V18_SQL)?,
            18 => run_migration_step(connection, &V18_TABLES, V18_TO_V19_SQL)?,
            19 => run_migration_step(connection, &V19_TABLES, V19_TO_V20_SQL)?,
            20 => run_migration_step(connection, &V20_TABLES, V20_TO_V21_SQL)?,
            21 => run_migration_step(connection, &V21_TABLES, V21_TO_V22_SQL)?,
            22 => run_migration_step(connection, &V22_TABLES, V22_TO_V23_SQL)?,
            LATEST_STORE_VERSION => {
                require_store_tables(connection, &V23_TABLES)?;
                require_outbox_event_uniqueness(connection)?;
                return Ok(());
            }
            _ => return Err(StoreError::UnsupportedStoreVersion),
        }
    }
}

/// Applies one linear migration step shared by every version that only needs
/// the standard guard-then-execute shape (no bespoke pre-check).
fn run_migration_step(
    connection: &Connection,
    required_tables: &[&str],
    sql: &str,
) -> Result<(), StoreError> {
    require_store_tables(connection, required_tables)?;
    require_outbox_event_uniqueness(connection)?;
    connection.execute_batch(sql)?;
    Ok(())
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
