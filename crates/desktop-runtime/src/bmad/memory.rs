//! Typed memory classes and compaction (ENG-10).
//!
//! Durable resume and Builder memory agents both read *authoritative* state:
//! entries whose `provenance.origin` is `user_supplied`, `skill_declared`, or
//! `host_observed`. A `compaction_summary` is `model_summarized` — a model's
//! restatement of prior entries — and must never become the thing resume
//! depends on. Without the separation enforced here, compacting a session
//! could silently discard the fact that made the next step correct, or
//! rewriting a `memlog_entry` in place could erase the history a Builder
//! memory agent relies on to explain how the session got where it is.
//!
//! This module is pure: it only rearranges `BmadMemoryMemoryEntry` values
//! already produced by validated contracts. It has no filesystem, database,
//! or model port, and persistence lives in `desktop-store`.

use crate::generated_contracts::{
    BmadMemoryMemoryEntry, BmadMemoryMemoryEntryMemoryClass, BmadMemoryMemoryEntryMutability,
    BmadMemoryMemoryEntryRetention, BmadMemoryMemoryEntrySensitivity,
    BmadMemoryMemoryProvenanceOrigin, BmadMemoryMemoryScope, BmadMemoryMemoryScopeKind,
    CommonContractId, CommonSha256,
};
use crate::{canonical_hash, Sha256Digest};

/// Matches the contract's `entries` bound (`bmad-memory.schema.json`).
pub const MAX_MEMORY_ENTRIES: usize = 4096;
/// Matches the contract's `provenance.sourceEntryIds` bound.
pub const MAX_SOURCE_ENTRY_IDS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadMemoryError {
    /// Compaction attempted to drop an authoritative or pinned entry, or one
    /// that is not `replaceable`.
    AuthoritativeLoss,
    /// A `memlog_entry` (or any `append_only` entry) was targeted for
    /// in-place replacement or deletion.
    AppendOnlyViolation,
    /// A `compaction_summary` carried no `sourceEntryIds`.
    UnsourcedSummary,
    /// An entry's shape contradicts its declared class or mutability.
    InvalidEntry,
    /// A `sourceEntryIds` reference does not name an entry in the set.
    UnknownSourceEntry,
    /// The entry set would exceed the contract's `maxItems` bound.
    TooManyEntries,
    /// The resume-state hash could not be computed.
    HashFailure,
}

/// `origin` values resume and compaction treat as ground truth.
#[must_use]
pub fn is_authoritative_origin(origin: BmadMemoryMemoryProvenanceOrigin) -> bool {
    matches!(
        origin,
        BmadMemoryMemoryProvenanceOrigin::UserSupplied
            | BmadMemoryMemoryProvenanceOrigin::SkillDeclared
            | BmadMemoryMemoryProvenanceOrigin::HostObserved
    )
}

fn shape_is_valid(entry: &BmadMemoryMemoryEntry) -> bool {
    // A memlog_entry is the durable chronology; letting one carry
    // `replaceable` would silently reopen the door compaction closes below.
    if entry.memory_class == BmadMemoryMemoryEntryMemoryClass::MemlogEntry
        && entry.mutability != BmadMemoryMemoryEntryMutability::AppendOnly
    {
        return false;
    }
    if entry.memory_class == BmadMemoryMemoryEntryMemoryClass::CompactionSummary
        && entry.provenance.origin != BmadMemoryMemoryProvenanceOrigin::ModelSummarized
    {
        return false;
    }
    entry.provenance.source_entry_ids.len() <= MAX_SOURCE_ENTRY_IDS
}

/// Inserts one new entry, validating its shape against the rest of the set.
///
/// # Errors
///
/// Returns [`BmadMemoryError::TooManyEntries`] over the contract bound,
/// [`BmadMemoryError::InvalidEntry`] for a duplicate `entryId` or a shape
/// that contradicts its declared class, and
/// [`BmadMemoryError::UnsourcedSummary`] / [`BmadMemoryError::UnknownSourceEntry`]
/// for an unlinked or dangling `compaction_summary`.
pub fn insert_entry(
    entries: &[BmadMemoryMemoryEntry],
    entry: BmadMemoryMemoryEntry,
) -> Result<Vec<BmadMemoryMemoryEntry>, BmadMemoryError> {
    if entries.len() >= MAX_MEMORY_ENTRIES {
        return Err(BmadMemoryError::TooManyEntries);
    }
    if !shape_is_valid(&entry) {
        return Err(BmadMemoryError::InvalidEntry);
    }
    if entries
        .iter()
        .any(|existing| existing.entry_id == entry.entry_id)
    {
        return Err(BmadMemoryError::InvalidEntry);
    }
    if entry.memory_class == BmadMemoryMemoryEntryMemoryClass::CompactionSummary {
        validate_summary_sources(entries, &entry)?;
    }
    let mut next = entries.to_vec();
    next.push(entry);
    Ok(next)
}

fn validate_summary_sources(
    entries: &[BmadMemoryMemoryEntry],
    summary: &BmadMemoryMemoryEntry,
) -> Result<(), BmadMemoryError> {
    if summary.provenance.source_entry_ids.is_empty() {
        return Err(BmadMemoryError::UnsourcedSummary);
    }
    for source_id in &summary.provenance.source_entry_ids {
        if !entries
            .iter()
            .any(|existing| &existing.entry_id == source_id)
        {
            return Err(BmadMemoryError::UnknownSourceEntry);
        }
    }
    Ok(())
}

/// Replaces (marks superseded, never deletes) an existing `replaceable`
/// entry with a new one, appending the new entry to the set.
///
/// # Errors
///
/// Returns [`BmadMemoryError::AppendOnlyViolation`] when the target entry is
/// `append_only`, and [`BmadMemoryError::UnknownSourceEntry`] when the target
/// is not present.
pub fn supersede_entry(
    entries: &[BmadMemoryMemoryEntry],
    target_entry_id: &CommonContractId,
    replacement: BmadMemoryMemoryEntry,
) -> Result<Vec<BmadMemoryMemoryEntry>, BmadMemoryError> {
    let Some(target) = entries
        .iter()
        .find(|existing| &existing.entry_id == target_entry_id)
    else {
        return Err(BmadMemoryError::UnknownSourceEntry);
    };
    if target.mutability != BmadMemoryMemoryEntryMutability::Replaceable {
        return Err(BmadMemoryError::AppendOnlyViolation);
    }
    if entries.len() >= MAX_MEMORY_ENTRIES {
        return Err(BmadMemoryError::TooManyEntries);
    }
    if !shape_is_valid(&replacement) {
        return Err(BmadMemoryError::InvalidEntry);
    }
    let mut next = Vec::with_capacity(entries.len() + 1);
    for existing in entries {
        if &existing.entry_id == target_entry_id {
            let mut superseded = existing.clone();
            superseded.superseded_by_entry_id = Some(replacement.entry_id.clone());
            next.push(superseded);
        } else {
            next.push(existing.clone());
        }
    }
    next.push(replacement);
    Ok(next)
}

/// Commits one compaction: marks its authoritative-safe sources superseded
/// and appends the summary. This is the sole way a `compaction_summary` may
/// enter the set.
///
/// # Errors
///
/// Returns [`BmadMemoryError::InvalidEntry`] when `summary` is not a
/// `compaction_summary` from `model_summarized` provenance,
/// [`BmadMemoryError::UnsourcedSummary`] when it names no sources,
/// [`BmadMemoryError::UnknownSourceEntry`] when a named source is absent, and
/// [`BmadMemoryError::AuthoritativeLoss`] when a named source is
/// authoritative-origin, `pinned`, or not `replaceable` — compaction may only
/// ever retire entries that were never the ground truth to begin with.
pub fn compact(
    entries: &[BmadMemoryMemoryEntry],
    summary: BmadMemoryMemoryEntry,
) -> Result<Vec<BmadMemoryMemoryEntry>, BmadMemoryError> {
    if summary.memory_class != BmadMemoryMemoryEntryMemoryClass::CompactionSummary
        || summary.provenance.origin != BmadMemoryMemoryProvenanceOrigin::ModelSummarized
    {
        return Err(BmadMemoryError::InvalidEntry);
    }
    validate_summary_sources(entries, &summary)?;
    if entries.len() >= MAX_MEMORY_ENTRIES {
        return Err(BmadMemoryError::TooManyEntries);
    }

    for source_id in &summary.provenance.source_entry_ids {
        // Presence was already proven by validate_summary_sources.
        let source = entries
            .iter()
            .find(|existing| &existing.entry_id == source_id)
            .ok_or(BmadMemoryError::UnknownSourceEntry)?;
        let protected = is_authoritative_origin(source.provenance.origin)
            || source.retention == BmadMemoryMemoryEntryRetention::Pinned
            || source.mutability != BmadMemoryMemoryEntryMutability::Replaceable;
        if protected {
            return Err(BmadMemoryError::AuthoritativeLoss);
        }
    }

    let mut next = Vec::with_capacity(entries.len() + 1);
    for existing in entries {
        if summary
            .provenance
            .source_entry_ids
            .iter()
            .any(|source_id| source_id == &existing.entry_id)
        {
            let mut superseded = existing.clone();
            superseded.superseded_by_entry_id = Some(summary.entry_id.clone());
            next.push(superseded);
        } else {
            next.push(existing.clone());
        }
    }
    next.push(summary);
    Ok(next)
}

/// Authoritative, still-active entries ordered by `recordedAtSequence` (ties
/// broken by `entryId`, which is itself unique). This is precisely the input
/// [`compute_resume_state`] hashes, and precisely the set compaction can
/// never touch.
#[must_use]
pub fn authoritative_entries(entries: &[BmadMemoryMemoryEntry]) -> Vec<&BmadMemoryMemoryEntry> {
    let mut active: Vec<&BmadMemoryMemoryEntry> = entries
        .iter()
        .filter(|entry| {
            is_authoritative_origin(entry.provenance.origin)
                && entry.superseded_by_entry_id.is_none()
        })
        .collect();
    active.sort_by(|left, right| {
        left.provenance
            .recorded_at_sequence
            .cmp(&right.provenance.recorded_at_sequence)
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    active
}

/// Deterministic resume state: a content hash of the ordered authoritative
/// entries plus the sequence they extend to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadResumeState {
    pub next_sequence: i64,
    pub state_hash: Sha256Digest,
}

#[derive(serde::Serialize)]
struct ResumeStateEntry<'a> {
    entry_id: &'a CommonContractId,
    memory_class: BmadMemoryMemoryEntryMemoryClass,
    content_hash: &'a CommonSha256,
    recorded_at_sequence: i64,
}

/// Computes the "what happens next" state purely from authoritative entries.
///
/// Dropping every `compaction_summary` (or any other non-authoritative
/// entry) from `entries` must never change this value — that equivalence is
/// exactly what "authoritative state lives outside model summaries" means,
/// and is proven by a test in `tests/bmad_memory.rs`.
///
/// # Errors
///
/// Returns [`BmadMemoryError::HashFailure`] when the ordered projection
/// cannot be canonically hashed.
pub fn compute_resume_state(
    entries: &[BmadMemoryMemoryEntry],
) -> Result<BmadResumeState, BmadMemoryError> {
    let ordered = authoritative_entries(entries);
    let next_sequence = ordered
        .last()
        .map_or(0, |entry| entry.provenance.recorded_at_sequence);
    let projection: Vec<ResumeStateEntry<'_>> = ordered
        .iter()
        .map(|entry| ResumeStateEntry {
            entry_id: &entry.entry_id,
            memory_class: entry.memory_class,
            content_hash: &entry.content_hash,
            recorded_at_sequence: entry.provenance.recorded_at_sequence,
        })
        .collect();
    let state_hash = canonical_hash("bmad-memory-resume-state", 1, &projection)
        .map_err(|_| BmadMemoryError::HashFailure)?;
    Ok(BmadResumeState {
        next_sequence,
        state_hash,
    })
}

/// One entry as it may leave the domain toward a renderer or export surface.
/// Carries no field a `restricted` entry could smuggle through, and nothing
/// beyond the entry's own `contentHash` for a `personal` one — the raw
/// content behind that hash never lived in this type to begin with.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadMemoryProjectedEntry {
    pub entry_id: CommonContractId,
    pub memory_class: BmadMemoryMemoryEntryMemoryClass,
    pub scope: BmadMemoryMemoryScope,
    pub retention: BmadMemoryMemoryEntryRetention,
    pub mutability: BmadMemoryMemoryEntryMutability,
    pub sensitivity: BmadMemoryMemoryEntrySensitivity,
    pub content_hash: CommonSha256,
}

/// Projects entries for a renderer or export surface, applying sensitivity
/// policy (acceptance 3): `restricted` entries are dropped entirely, and
/// every surviving entry exposes only its hash, never raw content.
#[must_use]
pub fn project_for_export(entries: &[BmadMemoryMemoryEntry]) -> Vec<BmadMemoryProjectedEntry> {
    entries
        .iter()
        .filter(|entry| entry.sensitivity != BmadMemoryMemoryEntrySensitivity::Restricted)
        .map(|entry| BmadMemoryProjectedEntry {
            entry_id: entry.entry_id.clone(),
            memory_class: entry.memory_class,
            scope: entry.scope.clone(),
            retention: entry.retention,
            mutability: entry.mutability,
            sensitivity: entry.sensitivity,
            content_hash: entry.content_hash.clone(),
        })
        .collect()
}

/// Entries visible to a query for exactly one scope. An `installed_agent`
/// scope is isolated by `installedAgentId`, a `session` scope by
/// `sessionId`, and a `project` scope by `workspaceId`; `user` scope has no
/// further identifier and is a single shared bucket.
#[must_use]
pub fn entries_for_scope<'a>(
    entries: &'a [BmadMemoryMemoryEntry],
    kind: BmadMemoryMemoryScopeKind,
    identifier: Option<&CommonContractId>,
) -> Vec<&'a BmadMemoryMemoryEntry> {
    entries
        .iter()
        .filter(|entry| {
            if entry.scope.kind != kind {
                return false;
            }
            match kind {
                BmadMemoryMemoryScopeKind::Session => entry.scope.session_id.as_ref() == identifier,
                BmadMemoryMemoryScopeKind::Project => {
                    entry.scope.workspace_id.as_ref() == identifier
                }
                BmadMemoryMemoryScopeKind::InstalledAgent => {
                    entry.scope.installed_agent_id.as_ref() == identifier
                }
                BmadMemoryMemoryScopeKind::User => true,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_contracts::{BmadMemoryMemoryProvenance, BmadMemoryMemoryScope};

    fn id(text: &str) -> CommonContractId {
        CommonContractId::try_from(text).expect("valid contract id in test fixture")
    }

    fn hash(byte: u8) -> CommonSha256 {
        let digest = crate::sha256_bytes(&[byte]);
        CommonSha256::try_from(digest.to_string()).expect("valid sha256 in test fixture")
    }

    fn session_scope() -> BmadMemoryMemoryScope {
        BmadMemoryMemoryScope {
            installed_agent_id: None,
            kind: BmadMemoryMemoryScopeKind::Session,
            session_id: Some(id("sess_01ARZ3NDEKTSV4RRFFQ69G5FAV")),
            workspace_id: None,
        }
    }

    fn fact(
        entry_num: u8,
        origin: BmadMemoryMemoryProvenanceOrigin,
        sequence: i64,
    ) -> BmadMemoryMemoryEntry {
        BmadMemoryMemoryEntry {
            content_hash: hash(entry_num),
            entry_id: id(&format!("entry_01ARZ3NDEKTSV4RRFFQ69G5FA{entry_num}")),
            memory_class: BmadMemoryMemoryEntryMemoryClass::SessionFact,
            mutability: BmadMemoryMemoryEntryMutability::Replaceable,
            provenance: BmadMemoryMemoryProvenance {
                origin,
                recorded_at_sequence: sequence,
                source_entry_ids: Vec::new(),
                source_session_id: None,
            },
            retention: BmadMemoryMemoryEntryRetention::Bounded,
            scope: session_scope(),
            sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
            superseded_by_entry_id: None,
        }
    }

    fn memlog(entry_num: u8, sequence: i64) -> BmadMemoryMemoryEntry {
        BmadMemoryMemoryEntry {
            content_hash: hash(entry_num),
            entry_id: id(&format!("entry_01ARZ3NDEKTSV4RRFFQ69G5FA{entry_num}")),
            memory_class: BmadMemoryMemoryEntryMemoryClass::MemlogEntry,
            mutability: BmadMemoryMemoryEntryMutability::AppendOnly,
            provenance: BmadMemoryMemoryProvenance {
                origin: BmadMemoryMemoryProvenanceOrigin::HostObserved,
                recorded_at_sequence: sequence,
                source_entry_ids: Vec::new(),
                source_session_id: None,
            },
            retention: BmadMemoryMemoryEntryRetention::Bounded,
            scope: session_scope(),
            sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
            superseded_by_entry_id: None,
        }
    }

    fn summary_over(entry_num: u8, sources: &[BmadMemoryMemoryEntry]) -> BmadMemoryMemoryEntry {
        BmadMemoryMemoryEntry {
            content_hash: hash(entry_num),
            entry_id: id(&format!("entry_01ARZ3NDEKTSV4RRFFQ69G5FA{entry_num}")),
            memory_class: BmadMemoryMemoryEntryMemoryClass::CompactionSummary,
            mutability: BmadMemoryMemoryEntryMutability::Replaceable,
            provenance: BmadMemoryMemoryProvenance {
                origin: BmadMemoryMemoryProvenanceOrigin::ModelSummarized,
                recorded_at_sequence: sources
                    .iter()
                    .map(|source| source.provenance.recorded_at_sequence)
                    .max()
                    .unwrap_or(0),
                source_entry_ids: sources
                    .iter()
                    .map(|source| source.entry_id.clone())
                    .collect(),
                source_session_id: None,
            },
            retention: BmadMemoryMemoryEntryRetention::Bounded,
            scope: session_scope(),
            sensitivity: BmadMemoryMemoryEntrySensitivity::Ordinary,
            superseded_by_entry_id: None,
        }
    }

    // Break caught: a compaction summary that names an authoritative fact as
    // one of its sources must not be accepted — accepting it would let a
    // model summary silently retire the state resume depends on.
    #[test]
    fn compact_rejects_a_summary_that_sources_an_authoritative_fact() {
        let authoritative = fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1);
        let entries = vec![authoritative.clone()];
        let summary = summary_over(2, &[authoritative]);
        assert!(matches!(
            compact(&entries, summary),
            Err(BmadMemoryError::AuthoritativeLoss)
        ));
    }

    // Break caught: a compaction summary that names a `pinned` entry, even
    // one with model-summarized-eligible mutability, must not be accepted.
    #[test]
    fn compact_rejects_a_summary_that_sources_a_pinned_entry() {
        let mut pinned = fact(1, BmadMemoryMemoryProvenanceOrigin::ModelSummarized, 1);
        pinned.retention = BmadMemoryMemoryEntryRetention::Pinned;
        let entries = vec![pinned.clone()];
        let summary = summary_over(2, &[pinned]);
        assert!(matches!(
            compact(&entries, summary),
            Err(BmadMemoryError::AuthoritativeLoss)
        ));
    }

    // Break caught: a compaction summary with an empty sourceEntryIds list
    // would let compaction happen without any traceable link back to what it
    // claims to summarize.
    #[test]
    fn compact_rejects_an_unsourced_summary() {
        let entries = vec![fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1)];
        let mut summary = summary_over(2, &[]);
        summary.provenance.source_entry_ids.clear();
        assert!(matches!(
            compact(&entries, summary),
            Err(BmadMemoryError::UnsourcedSummary)
        ));
    }

    // Break caught: compacting a replaceable, model-summarized entry is the
    // one case compaction exists for — it must succeed and mark the source
    // superseded rather than deleting it.
    #[test]
    fn compact_supersedes_an_eligible_replaceable_entry() -> Result<(), BmadMemoryError> {
        let replaceable = fact(1, BmadMemoryMemoryProvenanceOrigin::ModelSummarized, 1);
        let entries = vec![replaceable.clone()];
        let summary = summary_over(2, &[replaceable.clone()]);
        let summary_id = summary.entry_id.clone();
        let next = compact(&entries, summary)?;
        let source = next
            .iter()
            .find(|entry| entry.entry_id == replaceable.entry_id)
            .expect("source entry retained");
        assert_eq!(source.superseded_by_entry_id, Some(summary_id));
        assert_eq!(next.len(), 2);
        Ok(())
    }

    // Break caught: superseding a memlog_entry (append_only) through the
    // mutation path must fail closed rather than silently rewriting history.
    #[test]
    fn supersede_entry_rejects_an_append_only_target() {
        let log_entry = memlog(1, 1);
        let entries = vec![log_entry.clone()];
        let replacement = memlog(2, 2);
        assert!(matches!(
            supersede_entry(&entries, &log_entry.entry_id, replacement),
            Err(BmadMemoryError::AppendOnlyViolation)
        ));
    }

    // Break caught: resume state must be a pure function of authoritative
    // entries — dropping every compaction_summary (or other non-authoritative
    // entry) from the set must not change the computed resume state. This is
    // the sharpest available proof that summaries cannot smuggle in a
    // different "next step" than the authoritative facts already determine.
    #[test]
    fn resume_state_is_unchanged_by_dropping_every_summary() -> Result<(), BmadMemoryError> {
        let fact_one = fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1);
        let fact_two = fact(2, BmadMemoryMemoryProvenanceOrigin::HostObserved, 2);
        let replaceable = fact(3, BmadMemoryMemoryProvenanceOrigin::ModelSummarized, 3);
        let with_summary = compact(
            &[fact_one.clone(), fact_two.clone(), replaceable.clone()],
            summary_over(4, &[replaceable]),
        )?;

        let without_summary: Vec<BmadMemoryMemoryEntry> = with_summary
            .iter()
            .filter(|entry| {
                entry.memory_class != BmadMemoryMemoryEntryMemoryClass::CompactionSummary
            })
            .cloned()
            .collect();

        let with_state = compute_resume_state(&with_summary)?;
        let without_state = compute_resume_state(&without_summary)?;
        assert_eq!(with_state, without_state);
        assert_eq!(with_state.next_sequence, 2);
        Ok(())
    }

    // Break caught: resume state must also survive dropping the superseded
    // (now-inactive) replaceable source itself, since it is no longer
    // authoritative-active once compacted — only the two untouched
    // authoritative facts should ever drive the hash.
    #[test]
    fn resume_state_ignores_superseded_and_non_authoritative_entries() -> Result<(), BmadMemoryError>
    {
        let fact_one = fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1);
        let fact_two = fact(2, BmadMemoryMemoryProvenanceOrigin::HostObserved, 2);
        let baseline = compute_resume_state(&[fact_one.clone(), fact_two.clone()])?;

        let replaceable = fact(3, BmadMemoryMemoryProvenanceOrigin::ModelSummarized, 3);
        let compacted = compact(
            &[fact_one, fact_two, replaceable.clone()],
            summary_over(4, &[replaceable]),
        )?;
        let after_compaction = compute_resume_state(&compacted)?;
        assert_eq!(baseline, after_compaction);
        Ok(())
    }

    // Break caught: a restricted entry reaching the export projection would
    // defeat the whole point of the sensitivity class — it must be dropped,
    // not merely redacted.
    #[test]
    fn project_for_export_drops_restricted_entries() {
        let mut restricted = fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1);
        restricted.sensitivity = BmadMemoryMemoryEntrySensitivity::Restricted;
        let mut personal = fact(2, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 2);
        personal.sensitivity = BmadMemoryMemoryEntrySensitivity::Personal;
        let entries = vec![restricted, personal.clone()];

        let projected = project_for_export(&entries);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].entry_id, personal.entry_id);
        assert_eq!(projected[0].content_hash, personal.content_hash);
    }

    // Break caught: an entry scoped to one installed agent must not be
    // reachable from a query naming a different installed agent, or the
    // per-agent sanctum boundary is worthless.
    #[test]
    fn entries_for_scope_isolates_by_installed_agent_id() {
        let mut agent_a = fact(1, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 1);
        agent_a.memory_class = BmadMemoryMemoryEntryMemoryClass::SanctumEntry;
        agent_a.scope = BmadMemoryMemoryScope {
            installed_agent_id: Some(id("agent_01ARZ3NDEKTSV4RRFFQ69G5FAA")),
            kind: BmadMemoryMemoryScopeKind::InstalledAgent,
            session_id: None,
            workspace_id: None,
        };
        let mut agent_b = fact(2, BmadMemoryMemoryProvenanceOrigin::UserSupplied, 2);
        agent_b.memory_class = BmadMemoryMemoryEntryMemoryClass::SanctumEntry;
        agent_b.scope = BmadMemoryMemoryScope {
            installed_agent_id: Some(id("agent_01ARZ3NDEKTSV4RRFFQ69G5FAB")),
            kind: BmadMemoryMemoryScopeKind::InstalledAgent,
            session_id: None,
            workspace_id: None,
        };
        let entries = vec![agent_a.clone(), agent_b];

        let visible = entries_for_scope(
            &entries,
            BmadMemoryMemoryScopeKind::InstalledAgent,
            Some(&id("agent_01ARZ3NDEKTSV4RRFFQ69G5FAA")),
        );
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].entry_id, agent_a.entry_id);
    }

    // Break caught: inserting a memlog_entry declared `replaceable` would
    // contradict the class's append-only contract; the shape check must
    // reject it before it ever reaches storage.
    #[test]
    fn insert_entry_rejects_a_replaceable_memlog_entry() {
        let mut malformed = memlog(1, 1);
        malformed.mutability = BmadMemoryMemoryEntryMutability::Replaceable;
        let result = insert_entry(&[], malformed);
        assert!(matches!(result, Err(BmadMemoryError::InvalidEntry)));
    }
}
