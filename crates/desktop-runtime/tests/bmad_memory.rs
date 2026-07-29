#![allow(clippy::expect_used)]

//! Typed memory classes, compaction, and sensitivity policy.
//!
//! These are black-box proofs against the crate's public surface. The point of
//! the module is that authoritative state survives summarization, so the tests
//! exercise the boundary a caller actually holds rather than internals.

use desktop_runtime::generated_contracts::{
    BmadMemoryMemoryEntry, BmadMemoryMemoryEntryMemoryClass, BmadMemoryMemoryEntrySensitivity,
    BmadMemoryMemoryScopeKind, CommonContractId,
};
use desktop_runtime::{
    compact_bmad_memory, compute_resume_state, entries_for_scope, insert_entry, project_for_export,
    supersede_entry, BmadMemoryError,
};
use serde_json::json;

fn id(prefix: &str, tail: u8) -> CommonContractId {
    format!("{prefix}_01J0000000000000000000000{tail}")
        .parse()
        .expect("valid contract id")
}

fn entry(
    tail: u8,
    class: &str,
    origin: &str,
    sequence: i64,
    retention: &str,
    mutability: &str,
    sensitivity: &str,
) -> BmadMemoryMemoryEntry {
    serde_json::from_value(json!({
        "entryId": id("entry", tail).to_string(),
        "memoryClass": class,
        "scope": { "kind": "project", "workspaceId": id("workspace", 0).to_string() },
        "provenance": { "origin": origin, "recordedAtSequence": sequence },
        "sensitivity": sensitivity,
        "retention": retention,
        "mutability": mutability,
        "contentHash": format!("sha256:{}", format!("{tail}").repeat(64))
    }))
    .expect("valid memory entry")
}

/// One authoritative pinned fact plus one replaceable, non-authoritative note.
fn base_entries() -> Vec<BmadMemoryMemoryEntry> {
    vec![
        entry(
            1,
            "persistent_fact",
            "user_supplied",
            1,
            "pinned",
            "append_only",
            "ordinary",
        ),
        entry(
            2,
            "session_fact",
            "model_summarized",
            2,
            "session",
            "replaceable",
            "ordinary",
        ),
    ]
}

fn summary(tail: u8, sources: &[u8], sequence: i64) -> BmadMemoryMemoryEntry {
    let mut value = entry(
        tail,
        "compaction_summary",
        "model_summarized",
        sequence,
        "bounded",
        "replaceable",
        "ordinary",
    );
    value.provenance.source_entry_ids = sources.iter().map(|tail| id("entry", *tail)).collect();
    value
}

// Break caught: acceptance 1 and 2 at once. If any non-authoritative entry
// leaked into the resume computation, summarizing a conversation would
// silently change what the skill does next — the exact failure that makes
// compaction unsafe to run at all.
#[test]
fn bmad_memory_resume_state_is_unchanged_by_dropping_every_summary() {
    let mut entries = base_entries();
    entries.push(summary(3, &[2], 3));

    let with_summary = compute_resume_state(&entries).expect("resume state");

    let without_summaries: Vec<BmadMemoryMemoryEntry> = entries
        .iter()
        .filter(|entry| entry.memory_class != BmadMemoryMemoryEntryMemoryClass::CompactionSummary)
        .cloned()
        .collect();
    let without = compute_resume_state(&without_summaries).expect("resume state");

    assert_eq!(with_summary.state_hash, without.state_hash);
    assert_eq!(with_summary.next_sequence, without.next_sequence);
}

// Break caught: acceptance 2. A summary claiming to cover an authoritative or
// pinned fact would let a model-written sentence stand in for the record the
// user actually supplied.
#[test]
fn bmad_memory_compaction_cannot_erase_authoritative_or_pinned_state() {
    let entries = base_entries();

    // Entry 1 is user_supplied and pinned; either alone disqualifies it.
    let refused = compact_bmad_memory(&entries, summary(4, &[1], 4))
        .expect_err("an authoritative source must not be compacted away");
    assert!(matches!(
        refused,
        BmadMemoryError::AuthoritativeLoss | BmadMemoryError::InvalidEntry
    ));

    // A summary with no source links cannot be audited, so it is refused.
    let unsourced = compact_bmad_memory(&entries, summary(5, &[], 5))
        .expect_err("an unsourced summary must be refused");
    assert!(matches!(unsourced, BmadMemoryError::UnsourcedSummary));

    // The one permitted path: a replaceable, non-authoritative source.
    let compacted =
        compact_bmad_memory(&entries, summary(6, &[2], 6)).expect("eligible compaction");
    assert!(compacted
        .iter()
        .any(|entry| entry.memory_class == BmadMemoryMemoryEntryMemoryClass::CompactionSummary));

    // Even a permitted compaction leaves the resume state untouched.
    let before = compute_resume_state(&entries).expect("resume state");
    let after = compute_resume_state(&compacted).expect("resume state");
    assert_eq!(before.state_hash, after.state_hash);
}

// Break caught: an append-only memlog entry rewritten in place destroys the
// record explaining how the session reached its current state.
#[test]
fn bmad_memory_append_only_entries_cannot_be_replaced_in_place() {
    let entries = vec![entry(
        1,
        "memlog_entry",
        "host_observed",
        1,
        "bounded",
        "append_only",
        "ordinary",
    )];
    let replacement = entry(
        2,
        "memlog_entry",
        "host_observed",
        2,
        "bounded",
        "append_only",
        "ordinary",
    );

    let refused = supersede_entry(&entries, &id("entry", 1), replacement)
        .expect_err("append-only entries are never replaced in place");
    assert!(matches!(refused, BmadMemoryError::AppendOnlyViolation));
    assert_eq!(entries.len(), 1);
}

// Break caught: acceptance 3. A restricted fact reaching an export or renderer
// surface is a policy breach no later redaction can undo.
#[test]
fn bmad_memory_export_projection_drops_restricted_entries_and_exposes_only_hashes() {
    let mut entries = base_entries();
    entries.push(entry(
        7,
        "sanctum_entry",
        "user_supplied",
        7,
        "pinned",
        "append_only",
        "restricted",
    ));

    let projected = project_for_export(&entries);

    assert!(projected
        .iter()
        .all(|entry| entry.sensitivity != BmadMemoryMemoryEntrySensitivity::Restricted));
    assert!(!projected
        .iter()
        .any(|entry| entry.entry_id == id("entry", 7)));

    // The projection carries hashes only; there is no raw content field to leak.
    let wire = serde_json::to_value(&projected).expect("projection serializes");
    let rendered = serde_json::to_string(&wire).expect("projection string");
    assert!(!rendered.contains("\"content\":"));
    assert!(rendered.contains("ontentHash"));
}

// Break caught: one installed agent reading another's sanctum, or a project
// scope bleeding across workspaces.
#[test]
fn bmad_memory_scope_queries_isolate_unrelated_owners() {
    let mut entries = base_entries();
    let mut other = entry(
        8,
        "sanctum_entry",
        "user_supplied",
        8,
        "pinned",
        "append_only",
        "ordinary",
    );
    other.scope = serde_json::from_value(json!({
        "kind": "installed_agent",
        "installedAgentId": id("agent", 9).to_string()
    }))
    .expect("valid scope");
    entries.push(other);

    let project_scoped = entries_for_scope(
        &entries,
        BmadMemoryMemoryScopeKind::Project,
        Some(&id("workspace", 0)),
    );
    assert_eq!(project_scoped.len(), 2);
    assert!(!project_scoped
        .iter()
        .any(|entry| entry.entry_id == id("entry", 8)));

    let foreign_agent = entries_for_scope(
        &entries,
        BmadMemoryMemoryScopeKind::InstalledAgent,
        Some(&id("agent", 1)),
    );
    assert!(foreign_agent.is_empty());
}

// Break caught: a duplicate entry identifier silently overwriting history.
#[test]
fn bmad_memory_insert_rejects_a_duplicate_entry_identifier() {
    let entries = base_entries();
    let duplicate = entry(
        1,
        "persistent_fact",
        "user_supplied",
        9,
        "pinned",
        "append_only",
        "ordinary",
    );

    let refused = insert_entry(&entries, duplicate).expect_err("entry identifiers are unique");
    assert!(matches!(refused, BmadMemoryError::InvalidEntry));

    let accepted = insert_entry(
        &entries,
        entry(
            9,
            "persistent_fact",
            "skill_declared",
            9,
            "bounded",
            "append_only",
            "ordinary",
        ),
    )
    .expect("a distinct entry appends");
    assert_eq!(accepted.len(), 3);
}
