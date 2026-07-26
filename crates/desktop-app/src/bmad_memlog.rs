//! Continuity across capability runs (P14): projecting a completed
//! `document_artifact` into memlog entries, and rendering the accumulated log
//! back as context for the next run against the same doc workspace.
//!
//! **What this is not.** Upstream appends "as each lands", during the
//! conversation, so its log is genuinely the source the artifact is distilled
//! from. Forge writes at the end of a single reviewed exchange, so *within one
//! run* the log is a projection of the artifact rather than its source. The
//! continuity benefit is real across runs — the second run against a doc
//! workspace sees what the first decided — but the within-run ordering is not
//! upstream's and this is not parity.

use std::collections::BTreeMap;

use desktop_runtime::{BmadDocumentArtifact, Memlog, MemlogEntryKind, UnixMillis};
use desktop_store::{StoredMemlogEntry, MAX_MEMLOG_ENTRY_BYTES};

/// Where the log for a capability accumulates. Host-owned, exactly like
/// [`crate::bmad_architecture`]'s run folder: upstream lets the user name a doc
/// workspace per run, Forge has no such picker, so the folder is derived from
/// the skill and kept stable so successive runs meet in the same log.
#[must_use]
pub(crate) fn doc_workspace_for(capability_id: &str) -> Option<String> {
    let (_, skill_name) = capability_id.split_once(':')?;
    let segment = skill_name.strip_prefix("bmad-").unwrap_or(skill_name);
    let path_safe = !segment.is_empty()
        && segment.len() <= 64
        && segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !segment.starts_with('-')
        && !segment.ends_with('-');
    path_safe.then(|| format!("docs/{segment}"))
}

/// One entry to append, before it is given a sequence by the store.
pub(crate) struct ProjectedEntry {
    pub kind: MemlogEntryKind,
    pub subject: Option<String>,
    pub text: String,
}

/// Marks text the host shortened, so a reader never mistakes a cut for the
/// whole of what the run decided.
const TRUNCATION_MARKER: &str = " […]";

/// Bounds one projected line to what the store accepts, cutting on a character
/// boundary so a multibyte scalar is never sliced in half.
fn bounded(text: &str) -> String {
    if text.len() <= MAX_MEMLOG_ENTRY_BYTES {
        return text.to_owned();
    }
    let mut cut = MAX_MEMLOG_ENTRY_BYTES - TRUNCATION_MARKER.len();
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut bounded = text[..cut].to_owned();
    bounded.push_str(TRUNCATION_MARKER);
    bounded
}

/// Condenses one section into a single entry-sized line.
///
/// Two reasons this is not a pass-through. A section body is bounded at 32 KiB
/// — eight times a memlog entry — so appending it raw would fail the append.
/// And upstream keeps entries to "one line each, the gist", so collapsing the
/// prose is the faithful shape rather than a workaround.
fn condense(subject: &str, body: &str) -> String {
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    bounded(&format!("{subject}: {body}"))
}

/// The decisions a doc workspace currently stands behind, replayed from its
/// log: a `decision` records or replaces one, an `override` withdraws it.
///
/// Replaying rather than scanning is what stops an override from being emitted
/// again on every later run — once a subject is withdrawn it is no longer live,
/// so there is nothing left to withdraw.
fn live_decisions(prior: &[StoredMemlogEntry]) -> BTreeMap<String, String> {
    let mut live: BTreeMap<String, String> = BTreeMap::new();
    for entry in prior {
        let Some(subject) = entry.subject.as_deref() else {
            continue;
        };
        match entry.kind {
            MemlogEntryKind::Decision => {
                live.insert(subject.to_owned(), entry.text.clone());
            }
            MemlogEntryKind::Override => {
                live.remove(subject);
            }
            _ => {}
        }
    }
    live
}

/// Projects a completed artifact into the entries to append.
///
/// `prior` is the doc workspace's existing log, which decides both the intent
/// recorded on the `event` entry and which sections count as changed.
///
/// **On intent:** the host can separate a first run against a folder from a
/// later one, so it records `create` or `update`. Upstream's third intent,
/// `validate`, is a critique that revises nothing, and no signal on a reviewed
/// run distinguishes it from an update — so it is never claimed here rather
/// than guessed at.
///
/// **On `change` vs `override`:** a reviewed document diff carries no explicit
/// overrule signal, so these record what structurally happened rather than what
/// the author intended — a heading whose text was revised is a `change`, a
/// heading the run stopped carrying is an `override`, because the earlier
/// decision no longer stands. Neither infers that anyone meant to overrule
/// anything.
pub(crate) fn completion_entries(
    capability_id: &str,
    artifact: &BmadDocumentArtifact,
    prior: &[StoredMemlogEntry],
) -> Vec<ProjectedEntry> {
    let live = live_decisions(prior);
    let intent = if prior.is_empty() { "create" } else { "update" };

    let mut entries = vec![ProjectedEntry {
        kind: MemlogEntryKind::Event,
        subject: None,
        text: bounded(&format!("{capability_id} {intent}: {}", artifact.title)),
    }];

    for section in &artifact.sections {
        let text = condense(&section.heading, &section.body);
        match live.get(&section.heading) {
            // Unchanged sections are not re-recorded; the log stays a record of
            // what moved, not a copy of the document once per run.
            Some(previous) if *previous == text => continue,
            // A revised heading is recorded as a change *before* its new
            // decision, so the log reads as "this moved, and here is where to".
            Some(_) => entries.push(ProjectedEntry {
                kind: MemlogEntryKind::Change,
                subject: Some(section.heading.clone()),
                text: bounded(&format!("{} revised", section.heading)),
            }),
            None => {}
        }
        entries.push(ProjectedEntry {
            kind: MemlogEntryKind::Decision,
            subject: Some(section.heading.clone()),
            text,
        });
    }

    for subject in live.keys() {
        let still_carried = artifact
            .sections
            .iter()
            .any(|section| section.heading == *subject);
        if !still_carried {
            entries.push(ProjectedEntry {
                kind: MemlogEntryKind::Override,
                subject: Some(subject.clone()),
                text: bounded(&format!(
                    "{subject} no longer carried by {}",
                    artifact.title
                )),
            });
        }
    }

    for question in &artifact.open_questions {
        entries.push(ProjectedEntry {
            kind: MemlogEntryKind::Question,
            subject: None,
            text: bounded(question),
        });
    }
    entries
}

/// Ceiling for the rendered log injected as context, matching
/// `RENDERED_STEP_LIMIT`.
const RENDERED_LOG_LIMIT: usize = 32 * 1_024;
const TRUNCATION_NOTICE: &str =
    "> Earlier entries omitted: this log exceeds the reviewed context bound.\n";

/// Renders a stored log for injection as one reviewed context candidate.
///
/// Rendering goes through the runtime [`Memlog`] rather than a second copy of
/// the formatting rules, so the context a run reads and the `.memlog.md` a run
/// proposes can never drift apart.
///
/// On overflow the *most recent* entries are kept behind an explicit notice —
/// a silently shortened history would let the next run contradict a decision it
/// was never shown.
#[must_use]
pub(crate) fn render_for_context(doc_workspace: &str, entries: &[StoredMemlogEntry]) -> String {
    // Choose what fits before building the log, so the rendered document is
    // always a well-formed memlog rather than a truncated one.
    let mut kept = entries.len();
    loop {
        let rendered = render_exactly(doc_workspace, &entries[entries.len() - kept..]);
        let notice = if kept == entries.len() {
            0
        } else {
            TRUNCATION_NOTICE.len()
        };
        if rendered.len() + notice <= RENDERED_LOG_LIMIT || kept == 0 {
            if kept == entries.len() {
                return rendered;
            }
            return format!("{TRUNCATION_NOTICE}{rendered}");
        }
        // Halve rather than step: a 2048-entry log would otherwise re-render
        // once per dropped entry.
        kept = kept.saturating_sub(1 + kept / 8);
    }
}

fn render_exactly(doc_workspace: &str, entries: &[StoredMemlogEntry]) -> String {
    let mut log = Memlog::open();
    // `Memlog` rejects lifecycle keys by design; a doc workspace is descriptive.
    let _ = log.set_field("doc_workspace", doc_workspace);
    for entry in entries {
        // A stored entry already satisfied these bounds on the way in, so a
        // rejection here would mean the row was tampered with; skipping keeps
        // the rest of the history readable instead of losing all of it.
        let _ = log.append(
            entry.kind,
            entry.author.as_deref(),
            &entry.text,
            UnixMillis(entry.recorded_at_ms),
        );
    }
    log.render()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{completion_entries, doc_workspace_for, render_for_context, RENDERED_LOG_LIMIT};
    use desktop_runtime::{BmadDocumentArtifact, BmadDocumentSection, MemlogEntryKind};
    use desktop_store::{StoredMemlogEntry, MAX_MEMLOG_ENTRY_BYTES};

    fn artifact(
        title: &str,
        sections: &[(&str, &str)],
        questions: &[&str],
    ) -> BmadDocumentArtifact {
        BmadDocumentArtifact::new(
            title.to_owned(),
            sections
                .iter()
                .map(|(heading, body)| BmadDocumentSection {
                    heading: (*heading).to_owned(),
                    body: (*body).to_owned(),
                })
                .collect(),
            Vec::new(),
            questions.iter().map(|q| (*q).to_owned()).collect(),
            None,
        )
        .expect("artifact")
    }

    fn stored(
        sequence: u64,
        kind: MemlogEntryKind,
        subject: Option<&str>,
        text: &str,
    ) -> StoredMemlogEntry {
        StoredMemlogEntry {
            sequence,
            kind,
            author: None,
            subject: subject.map(str::to_owned),
            text: text.to_owned(),
            recorded_at_ms: 1_700_000_000_000,
        }
    }

    /// Replays one artifact's projection into the shape the store would hold,
    /// so a multi-run sequence can be exercised without a database.
    fn append_to(prior: &mut Vec<StoredMemlogEntry>, artifact: &BmadDocumentArtifact) {
        for entry in completion_entries("bmm:bmad-prd", artifact, prior) {
            let sequence = prior.len() as u64;
            prior.push(StoredMemlogEntry {
                sequence,
                kind: entry.kind,
                author: None,
                subject: entry.subject,
                text: entry.text,
                recorded_at_ms: 1_700_000_000_000,
            });
        }
    }

    #[test]
    fn the_doc_workspace_is_stable_and_path_safe() {
        assert_eq!(
            doc_workspace_for("bmm:bmad-architecture").as_deref(),
            Some("docs/architecture")
        );
        assert_eq!(
            doc_workspace_for("bmm:bmad-prd").as_deref(),
            Some("docs/prd")
        );
        assert_eq!(
            doc_workspace_for("bmm:bmad-product-brief").as_deref(),
            Some("docs/product-brief")
        );
        // Nothing that could escape the folder or name a parent.
        assert_eq!(doc_workspace_for("bmm:../etc"), None);
        assert_eq!(doc_workspace_for("bmm:bmad-"), None);
        assert_eq!(doc_workspace_for("no-namespace"), None);
    }

    #[test]
    fn a_first_run_records_creation_decisions_and_questions() {
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact(
                "Payments PRD",
                &[("Goals", "Ship checkout"), ("Scope", "EU only")],
                &["Which PSP?"],
            ),
            &[],
        );
        let kinds: Vec<MemlogEntryKind> = entries.iter().map(|entry| entry.kind).collect();
        assert_eq!(
            kinds,
            vec![
                MemlogEntryKind::Event,
                MemlogEntryKind::Decision,
                MemlogEntryKind::Decision,
                MemlogEntryKind::Question,
            ]
        );
        assert!(entries[0].text.contains("create"));
        assert_eq!(entries[1].text, "Goals: Ship checkout");
        assert_eq!(entries[1].subject.as_deref(), Some("Goals"));
        assert_eq!(entries[3].text, "Which PSP?");
    }

    #[test]
    fn a_second_run_records_only_what_moved() {
        let prior = vec![
            stored(
                0,
                MemlogEntryKind::Event,
                None,
                "bmm:bmad-prd create: Payments PRD",
            ),
            stored(
                1,
                MemlogEntryKind::Decision,
                Some("Goals"),
                "Goals: Ship checkout",
            ),
            stored(
                2,
                MemlogEntryKind::Decision,
                Some("Scope"),
                "Scope: EU only",
            ),
        ];
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact(
                "Payments PRD",
                &[
                    ("Goals", "Ship checkout"),
                    ("Scope", "EU and UK"),
                    ("Risks", "PSP outage"),
                ],
                &[],
            ),
            &prior,
        );
        let rendered: Vec<(MemlogEntryKind, &str)> = entries
            .iter()
            .map(|entry| (entry.kind, entry.text.as_str()))
            .collect();
        assert_eq!(
            rendered,
            vec![
                (MemlogEntryKind::Event, "bmm:bmad-prd update: Payments PRD"),
                (MemlogEntryKind::Change, "Scope revised"),
                (MemlogEntryKind::Decision, "Scope: EU and UK"),
                (MemlogEntryKind::Decision, "Risks: PSP outage"),
            ]
        );
    }

    /// Regression: matching by splitting the rendered text on its separator
    /// attributed a change to the wrong heading whenever a heading contained
    /// one. `Phase 1: Discovery` is an entirely ordinary heading.
    #[test]
    fn headings_containing_the_separator_are_matched_exactly() {
        let mut log = Vec::new();
        append_to(
            &mut log,
            &artifact(
                "Plan",
                &[
                    ("Phase 1: Discovery", "Interviews"),
                    ("Phase 2: Build", "Ship it"),
                ],
                &[],
            ),
        );
        // Only Phase 2 moves. Splitting on the first ": " would have collapsed
        // both headings to "Phase 1"/"Phase 2" incorrectly and mismatched here.
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact(
                "Plan",
                &[
                    ("Phase 1: Discovery", "Interviews"),
                    ("Phase 2: Build", "Ship it later"),
                ],
                &[],
            ),
            &log,
        );
        let moved: Vec<(MemlogEntryKind, &str)> = entries
            .iter()
            .skip(1)
            .map(|entry| (entry.kind, entry.text.as_str()))
            .collect();
        assert_eq!(
            moved,
            vec![
                (MemlogEntryKind::Change, "Phase 2: Build revised"),
                (MemlogEntryKind::Decision, "Phase 2: Build: Ship it later"),
            ]
        );
    }

    #[test]
    fn a_withdrawn_heading_is_recorded_as_an_override() {
        let prior = vec![
            stored(
                0,
                MemlogEntryKind::Decision,
                Some("Goals"),
                "Goals: Ship checkout",
            ),
            stored(
                1,
                MemlogEntryKind::Decision,
                Some("Scope"),
                "Scope: EU only",
            ),
        ];
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact("Payments PRD", &[("Goals", "Ship checkout")], &[]),
            &prior,
        );
        let overrides: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.kind == MemlogEntryKind::Override)
            .map(|entry| entry.text.as_str())
            .collect();
        assert_eq!(overrides, vec!["Scope no longer carried by Payments PRD"]);
    }

    /// Regression: scanning prior decisions rather than replaying them
    /// re-emitted an override on *every* run after the withdrawal, so one
    /// dropped heading grew an unbounded tail of identical entries.
    #[test]
    fn a_withdrawal_is_recorded_once_not_on_every_later_run() {
        let mut log = Vec::new();
        let full = artifact("PRD", &[("Goals", "Ship"), ("Scope", "EU")], &[]);
        let trimmed = artifact("PRD", &[("Goals", "Ship")], &[]);
        append_to(&mut log, &full);
        append_to(&mut log, &trimmed);
        assert_eq!(override_count(&log), 1);

        // Two further runs that still do not carry Scope add nothing about it.
        append_to(&mut log, &trimmed);
        append_to(&mut log, &trimmed);
        assert_eq!(override_count(&log), 1);

        // Re-adding it decides it afresh, and dropping it again overrides again.
        append_to(&mut log, &full);
        append_to(&mut log, &trimmed);
        assert_eq!(override_count(&log), 2);
    }

    fn override_count(log: &[StoredMemlogEntry]) -> usize {
        log.iter()
            .filter(|entry| entry.kind == MemlogEntryKind::Override)
            .count()
    }

    /// Regression: a section body is bounded at 32 KiB and a memlog entry at
    /// 4 KiB, so passing the body through raw made the store reject the append
    /// — which failed a capability run whose result was already durable.
    #[test]
    fn a_long_section_body_is_condensed_to_one_entry_sized_line() {
        // Well inside the artifact's own 32 KiB section bound, and well over
        // the memlog's 4 KiB entry bound — the gap this test exists for.
        let body = "sentence ".repeat(3_000);
        assert!(body.len() > MAX_MEMLOG_ENTRY_BYTES);
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact("PRD", &[("Goals", &body)], &[]),
            &[],
        );
        for entry in &entries {
            assert!(
                entry.text.len() <= MAX_MEMLOG_ENTRY_BYTES,
                "every projected entry must fit the store's per-entry bound",
            );
        }
        let decision = entries
            .iter()
            .find(|entry| entry.kind == MemlogEntryKind::Decision)
            .expect("decision");
        assert!(decision.text.starts_with("Goals: sentence"));
        assert!(decision.text.ends_with(" […]"));
    }

    /// A multibyte body must not be cut mid-scalar.
    #[test]
    fn condensing_cuts_on_a_character_boundary() {
        let body = "é".repeat(4_000);
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact("PRD", &[("Goals", &body)], &[]),
            &[],
        );
        let decision = entries
            .iter()
            .find(|entry| entry.kind == MemlogEntryKind::Decision)
            .expect("decision");
        assert!(decision.text.len() <= MAX_MEMLOG_ENTRY_BYTES);
        assert!(decision.text.ends_with(" […]"));
    }

    #[test]
    fn an_oversized_log_keeps_the_most_recent_entries_and_says_so() {
        let entries: Vec<StoredMemlogEntry> = (0..2_000)
            .map(|index| {
                stored(
                    index,
                    MemlogEntryKind::Decision,
                    Some("Heading"),
                    &format!("Heading {index}: {}", "x".repeat(64)),
                )
            })
            .collect();
        let rendered = render_for_context("docs/prd", &entries);
        assert!(rendered.len() <= RENDERED_LOG_LIMIT);
        assert!(rendered.contains("Earlier entries omitted"));
        // The newest entry survives; the oldest is the one dropped.
        assert!(rendered.contains("Heading 1999:"));
        assert!(!rendered.contains("Heading 0:"));
    }

    /// The rendered context is the same document the `.memlog.md` run file is,
    /// because both go through `Memlog::render`.
    #[test]
    fn a_small_log_renders_as_an_upstream_shaped_memlog() {
        let entries = vec![
            stored(
                0,
                MemlogEntryKind::Event,
                None,
                "bmm:bmad-prd create: Payments PRD",
            ),
            stored(
                1,
                MemlogEntryKind::Decision,
                Some("Goals"),
                "Goals: Ship checkout",
            ),
        ];
        let rendered = render_for_context("docs/prd", &entries);
        assert!(!rendered.contains("Earlier entries omitted"));
        assert_eq!(
            rendered,
            "---\ndoc_workspace: docs/prd\n---\n\n\
             - (event) bmm:bmad-prd create: Payments PRD\n\
             - (decision) Goals: Ship checkout\n"
        );
    }

    #[test]
    fn an_empty_log_still_renders_a_well_formed_document() {
        assert_eq!(
            render_for_context("docs/prd", &[]),
            "---\ndoc_workspace: docs/prd\n---\n"
        );
    }
}
