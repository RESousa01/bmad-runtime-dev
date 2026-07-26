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

use desktop_runtime::{BmadDocumentArtifact, MemlogEntryKind};
use desktop_store::StoredMemlogEntry;

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
    pub text: String,
}

/// The heading a `decision` entry was written for, so a later run can tell
/// which of its sections are new, changed, or withdrawn.
const DECISION_SEPARATOR: &str = ": ";

fn decision_heading(text: &str) -> Option<&str> {
    text.split_once(DECISION_SEPARATOR)
        .map(|(heading, _)| heading)
}

/// Projects a completed artifact into the entries to append.
///
/// `prior` is the doc workspace's existing log, which decides both the intent
/// recorded on the `event` entry and which sections count as changed.
///
/// **On `change` vs `override`:** a reviewed document diff carries no explicit
/// overrule signal, so these are separated structurally rather than by the
/// author's intent — a heading whose text was revised is a `change`, a heading
/// that a prior run decided and this one dropped is an `override`, because the
/// earlier decision no longer stands. Neither infers that the author meant to
/// overrule anything.
pub(crate) fn completion_entries(
    capability_id: &str,
    artifact: &BmadDocumentArtifact,
    prior: &[StoredMemlogEntry],
) -> Vec<ProjectedEntry> {
    let prior_decisions: Vec<(&str, &str)> = prior
        .iter()
        .filter(|entry| entry.kind == MemlogEntryKind::Decision.code())
        .filter_map(|entry| {
            decision_heading(&entry.text).map(|heading| (heading, entry.text.as_str()))
        })
        .collect();
    let intent = if prior.is_empty() { "create" } else { "update" };

    let mut entries = vec![ProjectedEntry {
        kind: MemlogEntryKind::Event,
        text: format!("{capability_id} {intent}: {}", artifact.title),
    }];

    for section in &artifact.sections {
        let text = format!("{}{DECISION_SEPARATOR}{}", section.heading, section.body);
        // A revised heading is recorded as a change *before* its new decision,
        // so the log reads as "this moved, and here is where it moved to".
        if let Some((_, previous)) = prior_decisions
            .iter()
            .rfind(|(heading, _)| *heading == section.heading)
        {
            if *previous == text {
                // Unchanged sections are not re-recorded; the log stays a
                // record of what moved, not a copy of the document per run.
                continue;
            }
            entries.push(ProjectedEntry {
                kind: MemlogEntryKind::Change,
                text: format!("{} revised", section.heading),
            });
        }
        entries.push(ProjectedEntry {
            kind: MemlogEntryKind::Decision,
            text,
        });
    }

    let mut withdrawn: Vec<&str> = Vec::new();
    for (heading, _) in &prior_decisions {
        let still_present = artifact
            .sections
            .iter()
            .any(|section| section.heading == *heading);
        if !still_present && !withdrawn.contains(heading) {
            withdrawn.push(heading);
            entries.push(ProjectedEntry {
                kind: MemlogEntryKind::Override,
                text: format!("{heading} no longer carried by {}", artifact.title),
            });
        }
    }

    for question in &artifact.open_questions {
        entries.push(ProjectedEntry {
            kind: MemlogEntryKind::Question,
            text: question.clone(),
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
/// On overflow the *most recent* entries are kept behind an explicit notice —
/// a silently shortened history would let the next run contradict a decision it
/// was never shown.
#[must_use]
pub(crate) fn render_for_context(doc_workspace: &str, entries: &[StoredMemlogEntry]) -> String {
    let header = format!("# Memory log — {doc_workspace}\n\n");
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| {
            let tag = match (entry.kind.as_str(), entry.author.as_deref()) {
                ("", None) => String::new(),
                ("", Some(by)) => format!("(by {by}) "),
                (kind, None) => format!("({kind}) "),
                (kind, Some(by)) => format!("({kind} by {by}) "),
            };
            format!("- {tag}{}\n", entry.text)
        })
        .collect();

    let total: usize = header.len() + lines.iter().map(String::len).sum::<usize>();
    if total <= RENDERED_LOG_LIMIT {
        let mut rendered = header;
        rendered.extend(lines);
        return rendered;
    }

    let budget = RENDERED_LOG_LIMIT.saturating_sub(header.len() + TRUNCATION_NOTICE.len());
    let mut kept: Vec<&String> = Vec::new();
    let mut used = 0usize;
    for line in lines.iter().rev() {
        if used + line.len() > budget {
            break;
        }
        used += line.len();
        kept.push(line);
    }
    let mut rendered = header;
    rendered.push_str(TRUNCATION_NOTICE);
    for line in kept.iter().rev() {
        rendered.push_str(line);
    }
    rendered
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{completion_entries, doc_workspace_for, render_for_context, RENDERED_LOG_LIMIT};
    use desktop_runtime::{BmadDocumentArtifact, BmadDocumentSection, MemlogEntryKind};
    use desktop_store::StoredMemlogEntry;

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

    fn stored(sequence: u64, kind: MemlogEntryKind, text: &str) -> StoredMemlogEntry {
        StoredMemlogEntry {
            sequence,
            kind: kind.code().to_owned(),
            author: None,
            text: text.to_owned(),
            recorded_at_ms: 1_700_000_000_000,
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
        assert_eq!(entries[3].text, "Which PSP?");
    }

    #[test]
    fn a_second_run_records_only_what_moved() {
        let prior = vec![
            stored(
                0,
                MemlogEntryKind::Event,
                "bmm:bmad-prd create: Payments PRD",
            ),
            stored(1, MemlogEntryKind::Decision, "Goals: Ship checkout"),
            stored(2, MemlogEntryKind::Decision, "Scope: EU only"),
        ];
        let entries = completion_entries(
            "bmm:bmad-prd",
            &artifact(
                "Payments PRD",
                // Goals unchanged, Scope revised, Risks new, and the prior
                // "Scope" heading stays so nothing is withdrawn.
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

    #[test]
    fn a_withdrawn_heading_is_recorded_as_an_override() {
        let prior = vec![
            stored(0, MemlogEntryKind::Decision, "Goals: Ship checkout"),
            stored(1, MemlogEntryKind::Decision, "Scope: EU only"),
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

    #[test]
    fn an_oversized_log_keeps_the_most_recent_entries_and_says_so() {
        let entries: Vec<StoredMemlogEntry> = (0..2_000)
            .map(|index| {
                stored(
                    index,
                    MemlogEntryKind::Decision,
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

    #[test]
    fn a_small_log_renders_whole_with_no_notice() {
        let entries = vec![
            stored(
                0,
                MemlogEntryKind::Event,
                "bmm:bmad-prd create: Payments PRD",
            ),
            stored(1, MemlogEntryKind::Decision, "Goals: Ship checkout"),
        ];
        let rendered = render_for_context("docs/prd", &entries);
        assert!(!rendered.contains("Earlier entries omitted"));
        assert!(rendered.starts_with("# Memory log — docs/prd\n\n"));
        assert!(rendered.contains("- (event) bmm:bmad-prd create: Payments PRD\n"));
        assert!(rendered.contains("- (decision) Goals: Ship checkout\n"));
    }
}
