#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use desktop_runtime::{
    canonical_hash, canonical_hash_without_field, canonical_json_bytes, SpecConsumptionRecord,
};
use parking_lot::Mutex;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;
use time::OffsetDateTime;
use ulid::Ulid;
use zeroize::{Zeroize, ZeroizeOnDrop};

const CAS_MAGIC: &[u8; 8] = b"SAPHCAS1";
const CAS_FORMAT_VERSION: u16 = 1;
const CAS_NONCE_BYTES: usize = 12;
const STORE_KEY_BYTES: usize = 32;
const LATEST_STORE_VERSION: u32 = 4;
const REQUIRED_TABLES: [&str; 6] = [
    "aggregates",
    "evidence_events",
    "outbox",
    "payloads",
    "spec_consumptions",
    "store_meta",
];
const INITIAL_SCHEMA_SQL: &str = "BEGIN IMMEDIATE;
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

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("the local store key could not be protected or recovered")]
    KeyProtection,
    #[error("the encrypted payload failed authentication")]
    Authentication,
    #[error("the encrypted payload header is unsupported")]
    UnsupportedPayload,
    #[error("the local store is inconsistent")]
    Inconsistent,
    #[error("the local store schema is newer than this desktop build")]
    UnsupportedStoreVersion,
    #[error("the approved spec was already consumed")]
    AlreadyConsumed,
    #[error("the aggregate version did not advance by exactly one")]
    StateConflict,
    #[error("local store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("local store database operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("local store serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub trait KeyProtector: Send + Sync {
    /// Protects store-key bytes for the current user.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when key protection cannot be completed.
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError>;

    /// Recovers store-key bytes for the current user.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when protected bytes cannot be authenticated or
    /// recovered.
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError>;
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub struct UserDpapiProtector;

#[cfg(windows)]
impl KeyProtector for UserDpapiProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        dpapi_protect(plaintext)
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        dpapi_unprotect(protected)
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct StoreKey([u8; STORE_KEY_BYTES]);

impl StoreKey {
    fn from_bytes(mut bytes: Vec<u8>) -> Result<Self, StoreError> {
        if bytes.len() != STORE_KEY_BYTES {
            bytes.zeroize();
            return Err(StoreError::KeyProtection);
        }
        let mut key = [0_u8; STORE_KEY_BYTES];
        key.copy_from_slice(&bytes);
        bytes.zeroize();
        Ok(Self(key))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PayloadRef {
    pub content_hash: String,
    pub kind: String,
    pub schema_version: String,
    pub byte_count: u64,
    pub key_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceAppend {
    pub stream_id: String,
    pub event_type: String,
    pub payload_hash: String,
    pub payload_ref: Option<String>,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub redaction_level: String,
    pub retention_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRecord {
    pub event_id: String,
    pub stream_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub payload_hash: String,
    pub previous_event_hash: Option<String>,
    pub event_hash: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateRecord {
    pub version: u64,
    pub state_json: String,
}

#[derive(Debug)]
struct StoredEvidenceRow {
    event_id: String,
    stream_id: String,
    sequence: u64,
    event_type: String,
    payload_hash: String,
    payload_ref: Option<String>,
    previous_event_hash: Option<String>,
    event_hash: String,
    correlation_id: String,
    causation_id: Option<String>,
    redaction_level: String,
    retention_class: String,
    occurred_at: String,
}

#[derive(Debug)]
struct StoredConsumptionRow {
    consumption_id: String,
    spec_hash: String,
    candidate_hash: String,
    nonce_hash: String,
    audience_hash: String,
    execution_id: String,
    consumption_hash: String,
    record_json: String,
    consumed_at: String,
}

struct IntegritySnapshot {
    quick_check: String,
    outbox_link_errors: u64,
    events: Vec<StoredEvidenceRow>,
    payloads: Vec<PayloadRef>,
    consumptions: Vec<StoredConsumptionRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceHashInput<'a> {
    event_id: &'a str,
    stream_id: &'a str,
    sequence: u64,
    event_type: &'a str,
    payload_hash: &'a str,
    payload_ref: Option<&'a str>,
    previous_event_hash: Option<&'a str>,
    correlation_id: &'a str,
    causation_id: Option<&'a str>,
    redaction_level: &'a str,
    retention_class: &'a str,
    occurred_at: &'a str,
}

pub struct LocalStore {
    root: PathBuf,
    cas_root: PathBuf,
    store_id: String,
    key_version: u32,
    key: StoreKey,
    connection: Mutex<Connection>,
}

impl LocalStore {
    /// Opens or creates a local authority store and its encrypted CAS.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the root, key, database, configuration, or
    /// schema cannot be opened and verified safely.
    pub fn open(root: impl AsRef<Path>, protector: &dyn KeyProtector) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        let cas_root = root.join("cas");
        let key_path = root.join("store.key");
        let database_path = root.join("authority.sqlite3");
        let authority_data_exists = database_path.exists() || directory_has_entries(&cas_root)?;
        if authority_data_exists && !key_path.exists() {
            return Err(StoreError::KeyProtection);
        }
        fs::create_dir_all(&cas_root)?;
        let key = load_or_create_key(&key_path, protector)?;
        let connection = Connection::open(database_path)?;
        configure_connection(&connection)?;
        migrate(&connection)?;
        let store_id = load_or_create_store_id(&connection)?;
        Ok(Self {
            root,
            cas_root,
            store_id,
            key_version: 1,
            key,
            connection: Mutex::new(connection),
        })
    }

    #[must_use]
    pub fn store_id(&self) -> &str {
        &self.store_id
    }

    #[must_use]
    pub fn database_path(&self) -> PathBuf {
        self.root.join("authority.sqlite3")
    }

    /// Encrypts and durably registers a content-addressed payload.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when labels are invalid, encryption or durable I/O
    /// fails, or the stored payload does not authenticate against its metadata.
    pub fn put_payload(
        &self,
        kind: &str,
        schema_version: &str,
        plaintext: &[u8],
    ) -> Result<PayloadRef, StoreError> {
        validate_label(kind)?;
        validate_label(schema_version)?;
        let digest = Sha256::digest(plaintext);
        let content_hash = format!("sha256:{}", hex::encode(digest));
        let path = self.cas_path(kind, schema_version, &content_hash)?;
        if !path.exists() {
            let encrypted = self.encrypt(kind, schema_version, &content_hash, plaintext)?;
            Self::persist_cas(&path, &encrypted)?;
        }
        let existing = self.decrypt(
            kind,
            schema_version,
            &content_hash,
            self.key_version,
            &fs::read(&path)?,
        )?;
        if existing != plaintext {
            return Err(StoreError::Authentication);
        }

        let byte_count = u64::try_from(plaintext.len()).map_err(|_| StoreError::Inconsistent)?;
        let now = canonical_now();
        let connection = self.connection.lock();
        connection.execute(
            "INSERT OR IGNORE INTO payloads
             (content_hash, kind, schema_version, byte_count, key_version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                content_hash,
                kind,
                schema_version,
                byte_count,
                self.key_version,
                now
            ],
        )?;
        let stored = connection
            .query_row(
                "SELECT byte_count, key_version FROM payloads
                 WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
                params![content_hash, kind, schema_version],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
            )
            .optional()?;
        if stored != Some((byte_count, self.key_version)) {
            return Err(StoreError::Inconsistent);
        }
        Ok(PayloadRef {
            content_hash,
            kind: kind.to_owned(),
            schema_version: schema_version.to_owned(),
            byte_count,
            key_version: self.key_version,
        })
    }

    /// Loads and authenticates a registered content-addressed payload.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when metadata is inconsistent, the encrypted file
    /// cannot be read, or authenticated decryption fails.
    pub fn get_payload(&self, reference: &PayloadRef) -> Result<Vec<u8>, StoreError> {
        let stored = self
            .connection
            .lock()
            .query_row(
                "SELECT byte_count, key_version FROM payloads
                 WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
                params![
                    reference.content_hash,
                    reference.kind,
                    reference.schema_version
                ],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
            )
            .optional()?;
        if stored != Some((reference.byte_count, reference.key_version)) {
            return Err(StoreError::Inconsistent);
        }
        let path = self.cas_path(
            &reference.kind,
            &reference.schema_version,
            &reference.content_hash,
        )?;
        let encrypted = fs::read(path)?;
        let plaintext = self.decrypt(
            &reference.kind,
            &reference.schema_version,
            &reference.content_hash,
            reference.key_version,
            &encrypted,
        )?;
        let actual_hash = format!("sha256:{}", hex::encode(Sha256::digest(&plaintext)));
        if actual_hash != reference.content_hash {
            return Err(StoreError::Authentication);
        }
        Ok(plaintext)
    }

    /// Atomically advances an aggregate, evidence stream, and outbox record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when input validation, version ordering, canonical
    /// hashing, or the `SQLite` transaction fails.
    pub fn append_transition(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        aggregate_version: u64,
        aggregate_state_json: &str,
        event: &EvidenceAppend,
    ) -> Result<EvidenceRecord, StoreError> {
        validate_transition_input(aggregate_type, aggregate_id, aggregate_state_json, event)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if event.payload_ref.is_some() {
            let payload_exists = transaction.query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM payloads WHERE content_hash = ?1
                 )",
                params![event.payload_hash.as_str()],
                |row| row.get::<_, bool>(0),
            )?;
            if !payload_exists {
                return Err(StoreError::Inconsistent);
            }
        }
        let current_version = transaction
            .query_row(
                "SELECT version FROM aggregates
                 WHERE aggregate_type = ?1 AND aggregate_id = ?2",
                params![aggregate_type, aggregate_id],
                |row| row.get::<_, u64>(0),
            )
            .optional()?;
        let expected_version = match current_version {
            Some(version) => version.checked_add(1).ok_or(StoreError::Inconsistent)?,
            None => 1,
        };
        if aggregate_version != expected_version {
            return Err(StoreError::StateConflict);
        }
        let (sequence, previous_event_hash) =
            next_evidence_position(&transaction, &event.stream_id)?;
        let event_id = format!("event_{}", Ulid::new());
        let occurred_at = canonical_now();
        let event_hash = hash_event(&EvidenceHashInput {
            event_id: &event_id,
            stream_id: &event.stream_id,
            sequence,
            event_type: &event.event_type,
            payload_hash: &event.payload_hash,
            payload_ref: event.payload_ref.as_deref(),
            previous_event_hash: previous_event_hash.as_deref(),
            correlation_id: &event.correlation_id,
            causation_id: event.causation_id.as_deref(),
            redaction_level: &event.redaction_level,
            retention_class: &event.retention_class,
            occurred_at: &occurred_at,
        })?;

        transaction.execute(
            "INSERT INTO aggregates (aggregate_type, aggregate_id, version, state_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(aggregate_type, aggregate_id) DO UPDATE SET
               version = excluded.version,
               state_json = excluded.state_json,
               updated_at = excluded.updated_at",
            params![
                aggregate_type,
                aggregate_id,
                aggregate_version,
                aggregate_state_json,
                occurred_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO evidence_events
             (event_id, stream_id, sequence, event_type, payload_hash, payload_ref,
              previous_event_hash, event_hash, correlation_id, causation_id,
              redaction_level, retention_class, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                event_id,
                event.stream_id,
                sequence,
                event.event_type,
                event.payload_hash,
                event.payload_ref,
                previous_event_hash,
                event_hash,
                event.correlation_id,
                event.causation_id,
                event.redaction_level,
                event.retention_class,
                occurred_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO outbox (outbox_id, event_id, created_at) VALUES (?1, ?2, ?3)",
            params![format!("outbox_{}", Ulid::new()), event_id, occurred_at],
        )?;
        transaction.commit()?;

        Ok(EvidenceRecord {
            event_id,
            stream_id: event.stream_id.clone(),
            sequence,
            event_type: event.event_type.clone(),
            payload_hash: event.payload_hash.clone(),
            previous_event_hash,
            event_hash,
            occurred_at,
        })
    }

    /// Loads the latest durable aggregate projection, when present.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when identifiers are invalid or `SQLite` cannot
    /// complete the query.
    pub fn load_aggregate(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Option<AggregateRecord>, StoreError> {
        validate_label(aggregate_type)?;
        validate_label(aggregate_id)?;
        self.connection
            .lock()
            .query_row(
                "SELECT version, state_json FROM aggregates
                 WHERE aggregate_type = ?1 AND aggregate_id = ?2",
                params![aggregate_type, aggregate_id],
                |row| {
                    Ok(AggregateRecord {
                        version: row.get(0)?,
                        state_json: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Durably records a validated one-time spec consumption.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the record is invalid, serialization or
    /// storage fails, or the spec was already consumed.
    pub fn consume_spec_record(
        &self,
        record: &SpecConsumptionRecord,
    ) -> Result<String, StoreError> {
        record.verify().map_err(|_| StoreError::Inconsistent)?;
        let consumption_id = record.draft.consumption_id.as_str();
        let spec_hash = record.draft.spec_hash.to_string();
        let candidate_hash = record.draft.candidate_hash.to_string();
        let nonce_hash = record.draft.single_use_nonce_hash.to_string();
        let audience_hash = record.draft.executor_audience_hash.to_string();
        let execution_id = record.draft.execution_id.as_str();
        let consumption_hash = record.consumption_hash.to_string();
        let record_value = serde_json::to_value(record).map_err(|_| StoreError::Inconsistent)?;
        let consumed_at = record_value
            .as_object()
            .and_then(|object| object.get("consumedAt"))
            .and_then(serde_json::Value::as_str)
            .ok_or(StoreError::Inconsistent)?
            .to_owned();
        let record_json = String::from_utf8(
            canonical_json_bytes(&record_value).map_err(|_| StoreError::Inconsistent)?,
        )
        .map_err(|_| StoreError::Inconsistent)?;
        let result = self.connection.lock().execute(
            "INSERT INTO spec_consumptions
             (consumption_id, spec_hash, candidate_hash, nonce_hash, audience_hash, execution_id,
              consumption_hash, record_json, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                consumption_id,
                spec_hash,
                candidate_hash,
                nonce_hash,
                audience_hash,
                execution_id,
                consumption_hash,
                record_json,
                consumed_at
            ],
        );
        match result {
            Ok(_) => Ok(consumption_id.to_owned()),
            Err(error) if is_unique_violation(&error) => Err(StoreError::AlreadyConsumed),
            Err(error) => Err(StoreError::Sqlite(error)),
        }
    }

    /// Verifies `SQLite`, evidence, payload, and consumption integrity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when any durable relationship, canonical hash,
    /// payload authentication, or database invariant fails.
    pub fn verify_integrity(&self) -> Result<(), StoreError> {
        let IntegritySnapshot {
            quick_check,
            outbox_link_errors,
            events,
            payloads,
            consumptions,
        } = {
            let connection = self.connection.lock();
            load_integrity_snapshot(&connection)?
        };
        if quick_check != "ok" || outbox_link_errors != 0 {
            Err(StoreError::Inconsistent)
        } else {
            let registered_payload_hashes = payloads
                .iter()
                .map(|payload| payload.content_hash.as_str())
                .collect::<HashSet<_>>();
            verify_evidence_rows(&events, &registered_payload_hashes)?;
            for payload in &payloads {
                let plaintext = self.get_payload(payload)?;
                if u64::try_from(plaintext.len()).map_err(|_| StoreError::Inconsistent)?
                    != payload.byte_count
                {
                    return Err(StoreError::Inconsistent);
                }
            }
            verify_consumption_rows(&consumptions)?;
            Ok(())
        }
    }

    /// Runs a controlled truncating WAL checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when `SQLite` cannot checkpoint every log frame.
    pub fn checkpoint_wal(&self) -> Result<(), StoreError> {
        let (busy, log_frames, checkpointed_frames) =
            self.connection
                .lock()
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?;
        if busy != 0
            || log_frames < 0
            || checkpointed_frames < 0
            || log_frames != checkpointed_frames
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(())
    }

    fn encrypt(
        &self,
        kind: &str,
        schema_version: &str,
        content_hash: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, StoreError> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key.0).map_err(|_| StoreError::KeyProtection)?;
        let mut nonce = [0_u8; CAS_NONCE_BYTES];
        rand::rng().fill_bytes(&mut nonce);
        let aad = self.aad(kind, schema_version, content_hash, self.key_version);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StoreError::Authentication)?;
        let mut output = Vec::with_capacity(
            CAS_MAGIC.len() + std::mem::size_of::<u16>() + CAS_NONCE_BYTES + ciphertext.len(),
        );
        output.extend_from_slice(CAS_MAGIC);
        output.extend_from_slice(&CAS_FORMAT_VERSION.to_be_bytes());
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    fn decrypt(
        &self,
        kind: &str,
        schema_version: &str,
        content_hash: &str,
        key_version: u32,
        encrypted: &[u8],
    ) -> Result<Vec<u8>, StoreError> {
        let header_length = CAS_MAGIC.len() + std::mem::size_of::<u16>() + CAS_NONCE_BYTES;
        if encrypted.len() < header_length || &encrypted[..CAS_MAGIC.len()] != CAS_MAGIC {
            return Err(StoreError::UnsupportedPayload);
        }
        let version_start = CAS_MAGIC.len();
        let version_end = version_start + std::mem::size_of::<u16>();
        let version = u16::from_be_bytes(
            encrypted[version_start..version_end]
                .try_into()
                .map_err(|_| StoreError::UnsupportedPayload)?,
        );
        if version != CAS_FORMAT_VERSION || key_version != self.key_version {
            return Err(StoreError::UnsupportedPayload);
        }
        let nonce_end = version_end + CAS_NONCE_BYTES;
        let nonce = Nonce::from_slice(&encrypted[version_end..nonce_end]);
        let aad = self.aad(kind, schema_version, content_hash, key_version);
        Aes256Gcm::new_from_slice(&self.key.0)
            .map_err(|_| StoreError::KeyProtection)?
            .decrypt(
                nonce,
                Payload {
                    msg: &encrypted[nonce_end..],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StoreError::Authentication)
    }

    fn aad(&self, kind: &str, schema: &str, hash: &str, key_version: u32) -> String {
        format!(
            "sapphirus-cas\nstore={}\nkind={}\nschema={}\nhash={}\nkeyVersion={}",
            self.store_id, kind, schema, hash, key_version
        )
    }

    fn cas_path(
        &self,
        kind: &str,
        schema_version: &str,
        content_hash: &str,
    ) -> Result<PathBuf, StoreError> {
        validate_label(kind)?;
        validate_label(schema_version)?;
        validate_sha256(content_hash)?;
        let storage_preimage =
            format!("sapphirus:cas-storage:1\n{kind}\n{schema_version}\n{content_hash}");
        let digest = hex::encode(Sha256::digest(storage_preimage.as_bytes()));
        Ok(self.cas_root.join(&digest[..2]).join(digest))
    }

    fn persist_cas(destination: &Path, encrypted: &[u8]) -> Result<(), StoreError> {
        let parent = destination.parent().ok_or(StoreError::Inconsistent)?;
        fs::create_dir_all(parent)?;
        if destination.exists() {
            return Ok(());
        }
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(encrypted)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(destination) {
            Ok(_) => Ok(()),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(StoreError::Io(error.error)),
        }
    }
}

fn validate_transition_input(
    aggregate_type: &str,
    aggregate_id: &str,
    aggregate_state_json: &str,
    event: &EvidenceAppend,
) -> Result<(), StoreError> {
    validate_label(aggregate_type)?;
    validate_label(aggregate_id)?;
    validate_evidence_label(&event.stream_id)?;
    validate_evidence_label(&event.event_type)?;
    validate_evidence_label(&event.correlation_id)?;
    if let Some(causation_id) = &event.causation_id {
        validate_evidence_label(causation_id)?;
    }
    validate_label(&event.redaction_level)?;
    validate_label(&event.retention_class)?;
    validate_sha256(&event.payload_hash)?;
    if let Some(payload_ref) = &event.payload_ref {
        validate_bound_payload_reference(payload_ref, &event.payload_hash)?;
    }
    let state: serde_json::Value = serde_json::from_str(aggregate_state_json)?;
    if !state.is_object() {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn next_evidence_position(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: &str,
) -> Result<(u64, Option<String>), StoreError> {
    let previous = transaction
        .query_row(
            "SELECT sequence, event_hash FROM evidence_events
             WHERE stream_id = ?1 ORDER BY sequence DESC LIMIT 1",
            params![stream_id],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    match previous {
        Some((previous_sequence, previous_hash)) => Ok((
            previous_sequence
                .checked_add(1)
                .ok_or(StoreError::Inconsistent)?,
            Some(previous_hash),
        )),
        None => Ok((1, None)),
    }
}

fn load_integrity_snapshot(connection: &Connection) -> Result<IntegritySnapshot, StoreError> {
    require_outbox_event_uniqueness(connection)?;
    let quick_check = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    let outbox_link_errors = connection.query_row(
        "SELECT
           (SELECT COUNT(*)
              FROM evidence_events AS event
              LEFT JOIN outbox AS item ON item.event_id = event.event_id
             WHERE item.event_id IS NULL)
           +
           (SELECT COUNT(*)
              FROM outbox AS item
              LEFT JOIN evidence_events AS event ON event.event_id = item.event_id
             WHERE event.event_id IS NULL)",
        [],
        |row| row.get(0),
    )?;
    let mut event_statement = connection.prepare(
        "SELECT event_id, stream_id, sequence, event_type, payload_hash, payload_ref,
                previous_event_hash, event_hash, correlation_id, causation_id,
                redaction_level, retention_class, occurred_at
         FROM evidence_events ORDER BY stream_id, sequence",
    )?;
    let events = event_statement
        .query_map([], |row| {
            Ok(StoredEvidenceRow {
                event_id: row.get(0)?,
                stream_id: row.get(1)?,
                sequence: row.get(2)?,
                event_type: row.get(3)?,
                payload_hash: row.get(4)?,
                payload_ref: row.get(5)?,
                previous_event_hash: row.get(6)?,
                event_hash: row.get(7)?,
                correlation_id: row.get(8)?,
                causation_id: row.get(9)?,
                redaction_level: row.get(10)?,
                retention_class: row.get(11)?,
                occurred_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut payload_statement = connection.prepare(
        "SELECT content_hash, kind, schema_version, byte_count, key_version
         FROM payloads ORDER BY content_hash, kind, schema_version",
    )?;
    let payloads = payload_statement
        .query_map([], |row| {
            Ok(PayloadRef {
                content_hash: row.get(0)?,
                kind: row.get(1)?,
                schema_version: row.get(2)?,
                byte_count: row.get(3)?,
                key_version: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut consumption_statement = connection.prepare(
        "SELECT consumption_id, spec_hash, candidate_hash, nonce_hash, audience_hash,
                execution_id, consumption_hash, record_json, consumed_at
         FROM spec_consumptions ORDER BY consumption_id",
    )?;
    let consumptions = consumption_statement
        .query_map([], |row| {
            Ok(StoredConsumptionRow {
                consumption_id: row.get(0)?,
                spec_hash: row.get(1)?,
                candidate_hash: row.get(2)?,
                nonce_hash: row.get(3)?,
                audience_hash: row.get(4)?,
                execution_id: row.get(5)?,
                consumption_hash: row.get(6)?,
                record_json: row.get(7)?,
                consumed_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IntegritySnapshot {
        quick_check,
        outbox_link_errors,
        events,
        payloads,
        consumptions,
    })
}

fn configure_connection(connection: &Connection) -> Result<(), StoreError> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.pragma_update(None, "wal_autocheckpoint", 0_i64)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    let journal_mode: String =
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    let foreign_keys: i64 =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    let synchronous: i64 = connection.pragma_query_value(None, "synchronous", |row| row.get(0))?;
    let wal_autocheckpoint: i64 =
        connection.pragma_query_value(None, "wal_autocheckpoint", |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal")
        || foreign_keys != 1
        || synchronous != 2
        || wal_autocheckpoint != 0
    {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn migrate(connection: &Connection) -> Result<(), StoreError> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => {
            if !store_table_names(connection)?.is_empty() {
                return Err(StoreError::Inconsistent);
            }
            connection.execute_batch(INITIAL_SCHEMA_SQL)?;
        }
        1 | 2 => {
            require_store_tables(connection)?;
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
        }
        3 => {
            require_store_tables(connection)?;
            reject_duplicate_outbox_events(connection)?;
            connection.execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE UNIQUE INDEX outbox_event_once ON outbox(event_id);
                 PRAGMA user_version = 4;
                 COMMIT;",
            )?;
        }
        LATEST_STORE_VERSION => require_store_tables(connection)?,
        _ => return Err(StoreError::UnsupportedStoreVersion),
    }
    require_outbox_event_uniqueness(connection)?;
    Ok(())
}

fn store_table_names(connection: &Connection) -> Result<HashSet<String>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(StoreError::from)?;
    Ok(table_names)
}

fn require_store_tables(connection: &Connection) -> Result<(), StoreError> {
    let expected = REQUIRED_TABLES
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

fn require_outbox_event_uniqueness(connection: &Connection) -> Result<(), StoreError> {
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

fn load_or_create_store_id(connection: &Connection) -> Result<String, StoreError> {
    if let Some(value) = connection
        .query_row(
            "SELECT value FROM store_meta WHERE key = 'store_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(value);
    }
    let value = format!("store_{}", Ulid::new());
    connection.execute(
        "INSERT INTO store_meta (key, value) VALUES ('store_id', ?1)",
        params![value],
    )?;
    Ok(value)
}

fn load_or_create_key(path: &Path, protector: &dyn KeyProtector) -> Result<StoreKey, StoreError> {
    if path.exists() {
        return StoreKey::from_bytes(protector.unprotect(&fs::read(path)?)?);
    }
    let mut raw_key = vec![0_u8; STORE_KEY_BYTES];
    rand::rng().fill_bytes(&mut raw_key);
    let protected = protector.protect(&raw_key)?;
    let key = StoreKey::from_bytes(raw_key)?;
    let parent = path.parent().ok_or(StoreError::KeyProtection)?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(&protected)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| StoreError::Io(error.error))?;
    Ok(key)
}

fn hash_event(value: &EvidenceHashInput<'_>) -> Result<String, StoreError> {
    canonical_hash("local-evidence-event", 1, value)
        .map(|digest| digest.to_string())
        .map_err(|_| StoreError::Inconsistent)
}

fn verify_evidence_rows(
    rows: &[StoredEvidenceRow],
    registered_payload_hashes: &HashSet<&str>,
) -> Result<(), StoreError> {
    let mut stream_heads = HashMap::<&str, (u64, &str)>::new();
    for row in rows {
        validate_label(&row.event_id)?;
        validate_evidence_label(&row.stream_id)?;
        validate_evidence_label(&row.event_type)?;
        validate_sha256(&row.payload_hash)?;
        if let Some(payload_ref) = &row.payload_ref {
            validate_bound_payload_reference(payload_ref, &row.payload_hash)?;
            if !registered_payload_hashes.contains(row.payload_hash.as_str()) {
                return Err(StoreError::Inconsistent);
            }
        }
        validate_sha256(&row.event_hash)?;
        validate_evidence_label(&row.correlation_id)?;
        if let Some(causation_id) = &row.causation_id {
            validate_evidence_label(causation_id)?;
        }
        validate_label(&row.redaction_level)?;
        validate_label(&row.retention_class)?;

        let expected_previous = if let Some((previous_sequence, previous_hash)) =
            stream_heads.get(row.stream_id.as_str())
        {
            if row.sequence
                != previous_sequence
                    .checked_add(1)
                    .ok_or(StoreError::Inconsistent)?
            {
                return Err(StoreError::Inconsistent);
            }
            Some(*previous_hash)
        } else {
            if row.sequence != 1 {
                return Err(StoreError::Inconsistent);
            }
            None
        };
        if row.previous_event_hash.as_deref() != expected_previous {
            return Err(StoreError::Inconsistent);
        }
        let expected_hash = hash_event(&EvidenceHashInput {
            event_id: &row.event_id,
            stream_id: &row.stream_id,
            sequence: row.sequence,
            event_type: &row.event_type,
            payload_hash: &row.payload_hash,
            payload_ref: row.payload_ref.as_deref(),
            previous_event_hash: row.previous_event_hash.as_deref(),
            correlation_id: &row.correlation_id,
            causation_id: row.causation_id.as_deref(),
            redaction_level: &row.redaction_level,
            retention_class: &row.retention_class,
            occurred_at: &row.occurred_at,
        })?;
        if row.event_hash != expected_hash {
            return Err(StoreError::Inconsistent);
        }
        stream_heads.insert(&row.stream_id, (row.sequence, &row.event_hash));
    }
    Ok(())
}

fn verify_consumption_rows(rows: &[StoredConsumptionRow]) -> Result<(), StoreError> {
    for row in rows {
        validate_label(&row.consumption_id)?;
        validate_sha256(&row.spec_hash)?;
        validate_sha256(&row.candidate_hash)?;
        validate_sha256(&row.nonce_hash)?;
        validate_sha256(&row.audience_hash)?;
        validate_label(&row.execution_id)?;
        validate_sha256(&row.consumption_hash)?;

        let value: serde_json::Value = serde_json::from_str(&row.record_json)?;
        let object = value.as_object().ok_or(StoreError::Inconsistent)?;
        let string_field = |name: &str| {
            object
                .get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or(StoreError::Inconsistent)
        };
        if string_field("schemaVersion")? != "sapphirus.spec-consumption.v1"
            || string_field("deliveryModel")? != "windows_local"
            || string_field("consumptionId")? != row.consumption_id
            || string_field("specHash")? != row.spec_hash
            || string_field("candidateHash")? != row.candidate_hash
            || string_field("singleUseNonceHash")? != row.nonce_hash
            || string_field("executorAudienceHash")? != row.audience_hash
            || string_field("executionId")? != row.execution_id
            || string_field("consumedAt")? != row.consumed_at
            || string_field("consumptionHash")? != row.consumption_hash
            || object
                .get("attemptNumber")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
        {
            return Err(StoreError::Inconsistent);
        }
        let actual = canonical_hash_without_field("spec-consumption", 1, &value, "consumptionHash")
            .map_err(|_| StoreError::Inconsistent)?;
        if actual.to_string() != row.consumption_hash {
            return Err(StoreError::Inconsistent);
        }
        if canonical_json_bytes(&value).map_err(|_| StoreError::Inconsistent)?
            != row.record_json.as_bytes()
        {
            return Err(StoreError::Inconsistent);
        }
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<(), StoreError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn validate_evidence_label(value: &str) -> Result<(), StoreError> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
    {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), StoreError> {
    let digest = value
        .strip_prefix("sha256:")
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
        .ok_or(StoreError::Inconsistent)?;
    debug_assert_eq!(digest.len(), 64);
    Ok(())
}

fn validate_cas_reference(value: &str) -> Result<(), StoreError> {
    let digest = value
        .strip_prefix("cas://sha256/")
        .ok_or(StoreError::Inconsistent)?;
    validate_sha256(&format!("sha256:{digest}"))
}

fn validate_bound_payload_reference(
    payload_ref: &str,
    payload_hash: &str,
) -> Result<(), StoreError> {
    validate_cas_reference(payload_ref)?;
    validate_sha256(payload_hash)?;
    let referenced_digest = payload_ref
        .strip_prefix("cas://sha256/")
        .ok_or(StoreError::Inconsistent)?;
    let payload_digest = payload_hash
        .strip_prefix("sha256:")
        .ok_or(StoreError::Inconsistent)?;
    if referenced_digest != payload_digest {
        return Err(StoreError::Inconsistent);
    }
    Ok(())
}

fn directory_has_entries(path: &Path) -> Result<bool, StoreError> {
    match fs::read_dir(path) {
        Ok(mut entries) => Ok(entries.next().transpose()?.is_some()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(StoreError::Io(error)),
    }
}

fn canonical_now() -> String {
    let now = OffsetDateTime::now_utc();
    let value = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond()
    );
    value
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Windows DPAPI and LocalFree expose only unsafe FFI entry points"
)]
fn dpapi_protect(plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_len = u32::try_from(plaintext.len()).map_err(|_| StoreError::KeyProtection)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input points to `plaintext` for the duration of the call; output is initialized by
    // DPAPI and released exactly once with LocalFree below. UI is explicitly forbidden.
    unsafe {
        CryptProtectData(
            &raw const input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
        .map_err(|_| StoreError::KeyProtection)?;
        if output.pbData.is_null() {
            return Err(StoreError::KeyProtection);
        }
        let length = usize::try_from(output.cbData).map_err(|_| StoreError::KeyProtection)?;
        let protected = std::slice::from_raw_parts(output.pbData, length).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(protected)
    }
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Windows DPAPI and LocalFree expose only unsafe FFI entry points"
)]
fn dpapi_unprotect(protected: &[u8]) -> Result<Vec<u8>, StoreError> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input_len = u32::try_from(protected.len()).map_err(|_| StoreError::KeyProtection)?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: protected.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input points to `protected` for the call; DPAPI allocates output and it is copied then
    // released exactly once with LocalFree. No description or UI output is requested.
    unsafe {
        CryptUnprotectData(
            &raw const input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
        .map_err(|_| StoreError::KeyProtection)?;
        if output.pbData.is_null() {
            return Err(StoreError::KeyProtection);
        }
        let length = usize::try_from(output.cbData).map_err(|_| StoreError::KeyProtection)?;
        let plaintext = std::slice::from_raw_parts(output.pbData, length).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_runtime::{
        sha256_bytes, AuthorityRef, ContractId, DeliveryModel, SpecConsumptionRecordDraft,
        UnixMillis,
    };

    #[derive(Debug)]
    struct TestProtector;

    impl KeyProtector for TestProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(plaintext.iter().map(|byte| byte ^ 0xA5).collect())
        }

        fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(protected.iter().map(|byte| byte ^ 0xA5).collect())
        }
    }

    #[cfg(windows)]
    #[test]
    fn user_dpapi_protector_round_trips_and_rejects_tamper() -> Result<(), StoreError> {
        let plaintext = b"sapphirus user-scoped store key";
        let protector = UserDpapiProtector;
        let mut protected = protector.protect(plaintext)?;

        assert_ne!(protected, plaintext);
        assert_eq!(protector.unprotect(&protected)?, plaintext);

        let tamper_index = protected.len() / 2;
        let Some(byte) = protected.get_mut(tamper_index) else {
            return Err(StoreError::KeyProtection);
        };
        *byte ^= 0x01;
        assert!(matches!(
            protector.unprotect(&protected),
            Err(StoreError::KeyProtection)
        ));
        Ok(())
    }

    #[test]
    fn encrypted_cas_round_trips_and_detects_tamper() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let reference = store.put_payload("context", "context.v1", b"confidential source")?;
        assert_eq!(store.get_payload(&reference)?, b"confidential source");

        let path = store.cas_path(
            &reference.kind,
            &reference.schema_version,
            &reference.content_hash,
        )?;
        let mut bytes = fs::read(&path)?;
        let index = bytes.len().saturating_sub(1);
        if let Some(byte) = bytes.get_mut(index) {
            *byte ^= 0x01;
        }
        fs::write(path, bytes)?;
        assert!(matches!(
            store.get_payload(&reference),
            Err(StoreError::Authentication)
        ));
        Ok(())
    }

    #[test]
    fn cas_separates_equal_plaintext_across_purposes() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let context = store.put_payload("context", "context.v1", b"shared bytes")?;
        let checkpoint = store.put_payload("checkpoint", "checkpoint.v1", b"shared bytes")?;

        assert_eq!(store.get_payload(&context)?, b"shared bytes");
        assert_eq!(store.get_payload(&checkpoint)?, b"shared bytes");
        assert_ne!(
            store.cas_path(
                &context.kind,
                &context.schema_version,
                &context.content_hash
            )?,
            store.cas_path(
                &checkpoint.kind,
                &checkpoint.schema_version,
                &checkpoint.content_hash,
            )?
        );
        Ok(())
    }

    #[test]
    fn payload_registration_rejects_existing_metadata_drift() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let reference = store.put_payload("context", "context.v1", b"trusted bytes")?;
        store.connection.lock().execute(
            "UPDATE payloads SET byte_count = byte_count + 1
             WHERE content_hash = ?1 AND kind = ?2 AND schema_version = ?3",
            params![
                reference.content_hash,
                reference.kind,
                reference.schema_version
            ],
        )?;

        assert!(matches!(
            store.put_payload("context", "context.v1", b"trusted bytes"),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }

    #[test]
    fn state_event_and_outbox_commit_together() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let event = EvidenceAppend {
            stream_id: "run:run_01".to_owned(),
            event_type: "proposal.created".to_owned(),
            payload_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            payload_ref: None,
            correlation_id: "corr_01".to_owned(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        };
        let first = store.append_transition("run", "run_01", 1, "{}", &event)?;
        let duplicate_outbox = store.connection.lock().execute(
            "INSERT INTO outbox (outbox_id, event_id, created_at) VALUES (?1, ?2, ?3)",
            params![
                "outbox_duplicate",
                first.event_id.as_str(),
                first.occurred_at.as_str()
            ],
        );
        assert!(matches!(
            duplicate_outbox,
            Err(ref error) if is_unique_violation(error)
        ));
        let second = store.append_transition("run", "run_01", 2, "{}", &event)?;
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(
            second.previous_event_hash.as_deref(),
            Some(first.event_hash.as_str())
        );
        assert!(matches!(
            store.append_transition("run", "run_01", 4, "{}", &event),
            Err(StoreError::StateConflict)
        ));
        Ok(())
    }

    #[test]
    fn sqlite_safety_pragmas_are_verified_and_checkpoint_is_controlled() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let (journal_mode, foreign_keys, synchronous, wal_autocheckpoint) = {
            let connection = store.connection.lock();
            (
                connection
                    .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?,
                connection.pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))?,
                connection.pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))?,
                connection
                    .pragma_query_value(None, "wal_autocheckpoint", |row| row.get::<_, i64>(0))?,
            )
        };
        assert!(journal_mode.eq_ignore_ascii_case("wal"));
        assert_eq!(foreign_keys, 1);
        assert_eq!(synchronous, 2);
        assert_eq!(wal_autocheckpoint, 0);
        store.checkpoint_wal()?;
        Ok(())
    }

    #[test]
    fn spec_consumption_is_single_use() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let first = consumption_record("consume_1", "execution_1", b"nonce one")?;
        let replay_with_drift = consumption_record("consume_2", "execution_2", b"nonce two")?;
        store.consume_spec_record(&first)?;
        store.verify_integrity()?;
        assert!(matches!(
            store.consume_spec_record(&replay_with_drift),
            Err(StoreError::AlreadyConsumed)
        ));
        Ok(())
    }

    #[test]
    fn consumption_timestamp_is_authoritative_and_integrity_bound(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let record = consumption_record("consume_1", "execution_1", b"nonce one")?;
        store.consume_spec_record(&record)?;

        let (consumed_at, record_json): (String, String) = store.connection.lock().query_row(
            "SELECT consumed_at, record_json FROM spec_consumptions WHERE consumption_id = ?1",
            params![record.draft.consumption_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let value: serde_json::Value = serde_json::from_str(&record_json)?;
        assert_eq!(
            value.get("consumedAt").and_then(serde_json::Value::as_str),
            Some(consumed_at.as_str())
        );
        store.verify_integrity()?;

        store.connection.lock().execute(
            "UPDATE spec_consumptions SET consumed_at = '1970-01-01T00:00:00.999Z'",
            [],
        )?;
        assert!(matches!(
            store.verify_integrity(),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }

    fn consumption_record(
        consumption_id: &str,
        execution_id: &str,
        nonce: &[u8],
    ) -> Result<SpecConsumptionRecord, Box<dyn std::error::Error>> {
        Ok(SpecConsumptionRecordDraft {
            schema_version: "sapphirus.spec-consumption.v1".to_owned(),
            consumption_id: ContractId::new(consumption_id)?,
            delivery_model: DeliveryModel::WindowsLocal,
            authority_ref: AuthorityRef {
                authority_kind: "desktop_local_store".to_owned(),
                authority_id: ContractId::new("authority_1")?,
                installation_id: ContractId::new("installation_1")?,
                local_store_id: ContractId::new("store_1")?,
                authority_epoch: 1,
            },
            spec_id: ContractId::new("spec_1")?,
            spec_hash: sha256_bytes(b"same immutable spec"),
            candidate_hash: sha256_bytes(b"candidate"),
            single_use_nonce_hash: sha256_bytes(nonce),
            executor_audience_hash: sha256_bytes(b"native patch engine"),
            execution_id: ContractId::new(execution_id)?,
            attempt_number: 1,
            consumed_at: UnixMillis(1_000),
        }
        .seal()?)
    }

    #[test]
    fn missing_key_with_existing_authority_data_fails_closed() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let _ = store.put_payload("context", "context.v1", b"sensitive")?;
        drop(store);
        fs::remove_file(directory.path().join("store.key"))?;

        assert!(matches!(
            LocalStore::open(directory.path(), &TestProtector),
            Err(StoreError::KeyProtection)
        ));
        Ok(())
    }

    #[test]
    fn future_store_version_fails_closed() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store
            .connection
            .lock()
            .pragma_update(None, "user_version", 999_u32)?;
        drop(store);

        assert!(matches!(
            LocalStore::open(directory.path(), &TestProtector),
            Err(StoreError::UnsupportedStoreVersion)
        ));
        Ok(())
    }

    #[test]
    fn evidence_integrity_covers_metadata_fields() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let payload = store.put_payload("evidence", "evidence.v1", b"event payload")?;
        let payload_digest = payload
            .content_hash
            .strip_prefix("sha256:")
            .ok_or(StoreError::Inconsistent)?
            .to_owned();
        let event = EvidenceAppend {
            stream_id: "run:run_01".to_owned(),
            event_type: "proposal.created".to_owned(),
            payload_hash: payload.content_hash,
            payload_ref: Some(format!("cas://sha256/{payload_digest}")),
            correlation_id: "corr_01".to_owned(),
            causation_id: Some("cause_01".to_owned()),
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        };
        let _ = store.append_transition("run", "run_01", 1, "{}", &event)?;
        store.connection.lock().execute(
            "UPDATE evidence_events SET correlation_id = 'corr_tampered'",
            [],
        )?;

        assert!(matches!(
            store.verify_integrity(),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }

    #[test]
    fn evidence_payload_reference_must_match_a_registered_payload() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let payload = store.put_payload("evidence", "evidence.v1", b"registered")?;
        let payload_digest = payload
            .content_hash
            .strip_prefix("sha256:")
            .ok_or(StoreError::Inconsistent)?
            .to_owned();
        let mismatched = EvidenceAppend {
            stream_id: "run:run_01".to_owned(),
            event_type: "proposal.created".to_owned(),
            payload_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            payload_ref: Some(format!("cas://sha256/{payload_digest}")),
            correlation_id: "corr_01".to_owned(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        };
        assert!(matches!(
            store.append_transition("run", "run_01", 1, "{}", &mismatched),
            Err(StoreError::Inconsistent)
        ));

        let unregistered = EvidenceAppend {
            payload_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_owned(),
            payload_ref: Some(
                "cas://sha256/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            ),
            ..mismatched
        };
        assert!(matches!(
            store.append_transition("run", "run_01", 1, "{}", &unregistered),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }

    #[test]
    fn evidence_integrity_rejects_a_coherently_rehashed_unregistered_payload(
    ) -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let payload = store.put_payload("evidence", "evidence.v1", b"registered")?;
        let payload_digest = payload
            .content_hash
            .strip_prefix("sha256:")
            .ok_or(StoreError::Inconsistent)?;
        let event = EvidenceAppend {
            stream_id: "run:run_01".to_owned(),
            event_type: "proposal.created".to_owned(),
            payload_hash: payload.content_hash.clone(),
            payload_ref: Some(format!("cas://sha256/{payload_digest}")),
            correlation_id: "corr_01".to_owned(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        };
        let record = store.append_transition("run", "run_01", 1, "{}", &event)?;

        let unregistered_hash =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let unregistered_ref =
            "cas://sha256/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let tampered_event_hash = hash_event(&EvidenceHashInput {
            event_id: &record.event_id,
            stream_id: &record.stream_id,
            sequence: record.sequence,
            event_type: &record.event_type,
            payload_hash: unregistered_hash,
            payload_ref: Some(unregistered_ref),
            previous_event_hash: record.previous_event_hash.as_deref(),
            correlation_id: &event.correlation_id,
            causation_id: event.causation_id.as_deref(),
            redaction_level: &event.redaction_level,
            retention_class: &event.retention_class,
            occurred_at: &record.occurred_at,
        })?;
        store.connection.lock().execute(
            "UPDATE evidence_events
                SET payload_hash = ?1, payload_ref = ?2, event_hash = ?3
              WHERE event_id = ?4",
            params![
                unregistered_hash,
                unregistered_ref,
                tampered_event_hash,
                record.event_id
            ],
        )?;

        assert!(matches!(
            store.verify_integrity(),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }

    #[test]
    fn version_three_adds_outbox_uniqueness_without_rebuilding_tables() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.connection.lock().execute_batch(
            "DROP INDEX outbox_event_once;
             PRAGMA user_version = 3;",
        )?;
        drop(store);

        let reopened = LocalStore::open(directory.path(), &TestProtector)?;
        let version: u32 =
            reopened
                .connection
                .lock()
                .pragma_query_value(None, "user_version", |row| row.get(0))?;
        assert_eq!(version, LATEST_STORE_VERSION);
        require_outbox_event_uniqueness(&reopened.connection.lock())?;
        Ok(())
    }

    #[test]
    fn version_three_with_duplicate_outbox_links_fails_closed() -> Result<(), StoreError> {
        let directory = tempfile::tempdir()?;
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        let event = EvidenceAppend {
            stream_id: "run:run_01".to_owned(),
            event_type: "proposal.created".to_owned(),
            payload_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            payload_ref: None,
            correlation_id: "corr_01".to_owned(),
            causation_id: None,
            redaction_level: "summary".to_owned(),
            retention_class: "evidence".to_owned(),
        };
        let record = store.append_transition("run", "run_01", 1, "{}", &event)?;
        store.connection.lock().execute_batch(
            "DROP INDEX outbox_event_once;
             PRAGMA user_version = 3;",
        )?;
        store.connection.lock().execute(
            "INSERT INTO outbox (outbox_id, event_id, created_at) VALUES (?1, ?2, ?3)",
            params![
                "outbox_duplicate",
                record.event_id.as_str(),
                record.occurred_at.as_str()
            ],
        )?;
        drop(store);

        assert!(matches!(
            LocalStore::open(directory.path(), &TestProtector),
            Err(StoreError::Inconsistent)
        ));
        Ok(())
    }
}
