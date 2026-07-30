//! The method's working memory: an append-only, chronological run log.
//!
//! `bmad-spec` states why this matters more than any single artifact:
//!
//! > "Deriving the contract from a living log instead of editing the contract
//! > in place is what lets the steps around the spec run in any order and feed
//! > the same spec without merge drift: the log only accumulates, the artifact
//! > is re-rendered."
//!
//! So a memlog is not a deliverable. It is the record every deliverable is
//! *derived* from, which is what makes a run resumable and order-independent.
//! Upstream implements it as a Python script writing `.memlog.md`; Forge owns
//! it natively so appends go through the same governed path as any other
//! workspace write.
//!
//! Three invariants, taken from the upstream script and enforced here:
//!
//! 1. **Append-only, chronological.** Entries land at the end in the order
//!    they happen. There is no edit and no delete — deliberately, so history
//!    is never rewritten.
//! 2. **Write-only.** An append never needs to read the log back; the caller
//!    reads it only to resume.
//! 3. **No lifecycle status.** Whether work is done, blocked, or paused is
//!    itself something that happened, so it is an `event` entry — never
//!    frontmatter the log would have to mutate.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::UnixMillis;

/// One entry's text ceiling. Entries are one-liners by construction: upstream
/// collapses whitespace precisely to keep the log dense.
const MAX_ENTRY_BYTES: usize = 4_096;
const MAX_ENTRIES: usize = 4_096;
const MAX_FIELD_BYTES: usize = 1_024;
const MAX_FIELDS: usize = 32;
/// The run folder's memlog is always this file, matching upstream addressing.
pub const MEMLOG_FILE_NAME: &str = ".memlog.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemlogError {
    /// Empty, whitespace-only, or over the reviewed ceiling.
    BoundsViolation,
    /// Entries must not move backwards in time within one log.
    NonMonotonicClock,
    /// A frontmatter key outside the reviewed shape.
    InvalidField,
}

/// What kind of thing an entry records. Upstream keeps this vocabulary open
/// ("the host skill supplies the vocabulary by how it calls `append` — the tool
/// stays neutral"), so this is the reviewed set covering every `--type` the
/// shipped skills actually use, plus a free `Note`.
///
/// `Change` and `Override` are what make the log an audit trail rather than a
/// notebook: upstream describes the memlog as "the run's canonical memory and
/// audit trail — every decision, change, and override … lands as one
/// append-only line", and `bmad-product-brief`, `bmad-prd` and `bmad-ux` all
/// append them. `Capability` is part of `bmad-spec`'s documented vocabulary,
/// where each capability carries a stable `CAP-N`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemlogEntryKind {
    Idea,
    Insight,
    Question,
    Decision,
    Direction,
    Assumption,
    Constraint,
    Capability,
    Change,
    Override,
    Version,
    Gap,
    Technique,
    Event,
    Note,
}

impl MemlogEntryKind {
    /// Every kind, in vocabulary order. `code`/`from_code` round-trip across
    /// this set, which is what lets a durably stored log be rebuilt into a
    /// `Memlog` rather than re-rendered by a second, drifting copy of the
    /// formatting rules.
    pub const ALL: [Self; 15] = [
        Self::Idea,
        Self::Insight,
        Self::Question,
        Self::Decision,
        Self::Direction,
        Self::Assumption,
        Self::Constraint,
        Self::Capability,
        Self::Change,
        Self::Override,
        Self::Version,
        Self::Gap,
        Self::Technique,
        Self::Event,
        Self::Note,
    ];

    /// The kind for a stored `code()`. `""` is [`Self::Note`], matching the
    /// untagged line upstream renders.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Insight => "insight",
            Self::Question => "question",
            Self::Decision => "decision",
            Self::Direction => "direction",
            Self::Assumption => "assumption",
            Self::Constraint => "constraint",
            Self::Capability => "capability",
            Self::Change => "change",
            Self::Override => "override",
            Self::Version => "version",
            Self::Gap => "gap",
            Self::Technique => "technique",
            Self::Event => "event",
            // A plain note renders with no tag at all, matching upstream.
            Self::Note => "",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemlogEntry {
    pub kind: MemlogEntryKind,
    /// Who the entry came from (`user`, `coach`, an agent code), if it matters.
    pub by: Option<String>,
    pub text: String,
    pub recorded_at: UnixMillis,
}

impl MemlogEntry {
    /// Renders the inline tag exactly as upstream does: `(idea)`,
    /// `(idea by user)`, `(by coach)`, or nothing.
    #[must_use]
    pub fn tag(&self) -> String {
        let kind = self.kind.code();
        match (kind.is_empty(), self.by.as_deref()) {
            (true, None) => String::new(),
            (true, Some(by)) => format!("(by {by}) "),
            (false, None) => format!("({kind}) "),
            (false, Some(by)) => format!("({kind} by {by}) "),
        }
    }
}

/// One run's memory. Append-only; the artifact is derived from it, never the
/// other way round.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Memlog {
    /// Descriptive frontmatter (`topic`, `goal`, `scope`, …). Carries no
    /// lifecycle status by design.
    fields: BTreeMap<String, String>,
    entries: Vec<MemlogEntry>,
}

impl Memlog {
    #[must_use]
    pub fn open() -> Self {
        Self {
            fields: BTreeMap::new(),
            entries: Vec::new(),
        }
    }

    /// Sets one descriptive frontmatter field.
    ///
    /// # Errors
    ///
    /// Returns [`MemlogError::InvalidField`] for an empty or non-descriptive
    /// key, and [`MemlogError::BoundsViolation`] past the reviewed ceilings.
    pub fn set_field(&mut self, key: &str, value: &str) -> Result<(), MemlogError> {
        let key = key.trim();
        if key.is_empty()
            || key.len() > 64
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(MemlogError::InvalidField);
        }
        // A memory log has no completion flag: status is an `event` entry.
        if matches!(key, "status" | "state" | "complete" | "completed") {
            return Err(MemlogError::InvalidField);
        }
        let value = collapse_whitespace(value);
        if value.is_empty() || value.len() > MAX_FIELD_BYTES {
            return Err(MemlogError::BoundsViolation);
        }
        if !self.fields.contains_key(key) && self.fields.len() >= MAX_FIELDS {
            return Err(MemlogError::BoundsViolation);
        }
        self.fields.insert(key.to_owned(), value);
        Ok(())
    }

    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }

    #[must_use]
    pub fn entries(&self) -> &[MemlogEntry] {
        &self.entries
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Appends one entry at the end. There is no edit or delete counterpart,
    /// by design.
    ///
    /// # Errors
    ///
    /// Returns [`MemlogError::BoundsViolation`] for empty or oversized text or
    /// a full log, and [`MemlogError::NonMonotonicClock`] if the entry would
    /// land before the previous one.
    pub fn append(
        &mut self,
        kind: MemlogEntryKind,
        by: Option<&str>,
        text: &str,
        recorded_at: UnixMillis,
    ) -> Result<&MemlogEntry, MemlogError> {
        // Upstream collapses newlines and runs so an entry stays one dense
        // line: the chronology is the structure, not prose.
        let text = collapse_whitespace(text);
        if text.is_empty() || text.len() > MAX_ENTRY_BYTES || self.entries.len() >= MAX_ENTRIES {
            return Err(MemlogError::BoundsViolation);
        }
        if let Some(last) = self.entries.last() {
            if recorded_at < last.recorded_at {
                return Err(MemlogError::NonMonotonicClock);
            }
        }
        let by = match by.map(collapse_whitespace) {
            None => None,
            Some(value) if value.is_empty() || value.len() > 64 => {
                return Err(MemlogError::BoundsViolation);
            }
            Some(value) => Some(value),
        };
        self.entries.push(MemlogEntry {
            kind,
            by,
            text,
            recorded_at,
        });
        self.entries.last().ok_or(MemlogError::BoundsViolation)
    }

    /// Every entry of one kind, in the order it happened. This is how a
    /// derived artifact is built: filter the log, never mutate it.
    #[must_use]
    pub fn entries_of(&self, kind: MemlogEntryKind) -> Vec<&MemlogEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == kind)
            .collect()
    }

    /// Renders the `.memlog.md` document: frontmatter, blank line, then one
    /// line per entry in order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut document = String::from("---\n");
        for (key, value) in &self.fields {
            let _ = writeln!(document, "{key}: {value}");
        }
        document.push_str("---\n");
        if !self.entries.is_empty() {
            document.push('\n');
            for entry in &self.entries {
                let _ = writeln!(document, "- {}{}", entry.tag(), entry.text);
            }
        }
        document
    }
}

/// Collapses every whitespace run to one space and trims, so an entry is one
/// dense line regardless of how the caller formatted it.
fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads one labelled field a model wrote inside prose.
///
/// Structured signals sometimes have nowhere else to travel: the reviewed wire
/// contract is closed, so a field the schema does not carry has to arrive in a
/// body the schema does. The architecture lane already reads `Binds`,
/// `Prevents` and `Rule` this way, and the memlog reads `Supersedes` — the
/// author's own statement that a section overrules an earlier decision.
///
/// Tolerates the shapes a model actually writes — `Binds: x`, `**Binds:** x`,
/// `- **Binds** — x` — because the label carries the meaning, not the markup.
#[must_use]
pub fn labelled_body_part(body: &str, label: &str) -> Option<String> {
    for line in body.lines() {
        let stripped = line
            .trim()
            .trim_start_matches(['-', '*', '+'])
            .trim()
            .trim_start_matches("**")
            .trim_start();
        let Some(rest) = stripped.get(..label.len()) else {
            continue;
        };
        if !rest.eq_ignore_ascii_case(label) {
            continue;
        }
        // Trim whitespace between each marker, so `Binds:`, `**Binds:**` and
        // `- **Binds** — ` all reduce to the same value.
        let value = stripped[label.len()..]
            .trim_start()
            .trim_start_matches("**")
            .trim_start()
            .trim_start_matches([':', '—', '-', '–'])
            .trim_start()
            .trim_start_matches("**")
            .trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{Memlog, MemlogEntryKind, MemlogError};
    use crate::UnixMillis;

    /// The vocabulary must cover every `--type` the shipped upstream skills
    /// actually append. `change` and `override` are what make the log an audit
    /// trail (`bmad-product-brief`, `bmad-prd`, `bmad-ux`), and `capability` is
    /// part of `bmad-spec`'s documented set — all three were missing, so those
    /// entries had nowhere faithful to land.
    /// A kind that stores as a code it cannot be read back from would strand
    /// every entry of that kind in the durable log.
    #[test]
    fn every_kind_round_trips_through_its_stored_code() {
        for kind in MemlogEntryKind::ALL {
            assert_eq!(MemlogEntryKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(MemlogEntryKind::from_code("not-a-kind"), None);
        // The untagged line upstream renders is a note, not an unknown kind.
        assert_eq!(MemlogEntryKind::from_code(""), Some(MemlogEntryKind::Note));
    }

    #[test]
    fn the_kind_vocabulary_covers_every_upstream_type() {
        let codes = MemlogEntryKind::ALL.map(MemlogEntryKind::code);

        // Observed in the sealed upstream sources under
        // `source/method/6.10.0/src`, plus `bmad-spec`'s documented vocabulary.
        for upstream in [
            "event",
            "idea",
            "decision",
            "insight",
            "change",
            "override",
            "technique",
            "capability",
            "constraint",
            "assumption",
            "question",
            "direction",
        ] {
            assert!(
                codes.contains(&upstream),
                "upstream appends `--type {upstream}` but the vocabulary has no kind for it",
            );
        }
        // A plain note is the only kind that renders without a tag.
        assert_eq!(MemlogEntryKind::Note.code(), "");
    }

    #[test]
    fn the_log_is_append_only_and_chronological() {
        let mut log = Memlog::open();
        log.append(
            MemlogEntryKind::Idea,
            None,
            "skip the signup wall",
            UnixMillis(10),
        )
        .expect("first entry");
        log.append(
            MemlogEntryKind::Decision,
            Some("user"),
            "lead with one pre-categorized account",
            UnixMillis(20),
        )
        .expect("second entry");

        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "skip the signup wall");
        assert_eq!(entries[1].by.as_deref(), Some("user"));
        // There is no edit or delete: the type exposes neither.
        assert_eq!(
            log.append(MemlogEntryKind::Note, None, "backdated", UnixMillis(19)),
            Err(MemlogError::NonMonotonicClock)
        );
    }

    #[test]
    fn entries_render_with_the_upstream_inline_tag() {
        let mut log = Memlog::open();
        log.append(MemlogEntryKind::Idea, None, "an idea", UnixMillis(1))
            .expect("idea");
        log.append(
            MemlogEntryKind::Idea,
            Some("user"),
            "their idea",
            UnixMillis(2),
        )
        .expect("attributed idea");
        log.append(
            MemlogEntryKind::Note,
            Some("coach"),
            "a nudge",
            UnixMillis(3),
        )
        .expect("attributed note");
        log.append(MemlogEntryKind::Note, None, "a plain note", UnixMillis(4))
            .expect("plain note");

        let rendered = log.render();
        assert!(rendered.contains("- (idea) an idea\n"));
        assert!(rendered.contains("- (idea by user) their idea\n"));
        assert!(rendered.contains("- (by coach) a nudge\n"));
        assert!(rendered.contains("- a plain note\n"));
    }

    #[test]
    fn multi_line_text_collapses_to_one_dense_line() {
        let mut log = Memlog::open();
        log.append(
            MemlogEntryKind::Insight,
            None,
            "  show real   data,\n\npre-categorized  ",
            UnixMillis(1),
        )
        .expect("entry");

        assert_eq!(log.entries()[0].text, "show real data, pre-categorized");
        assert_eq!(log.render().matches('\n').count(), 4);
    }

    #[test]
    fn the_log_carries_no_lifecycle_status() {
        let mut log = Memlog::open();
        // Completion is something that happened, so it is an event entry...
        log.append(
            MemlogEntryKind::Event,
            None,
            "session complete",
            UnixMillis(1),
        )
        .expect("event");
        assert_eq!(log.entries_of(MemlogEntryKind::Event).len(), 1);

        // ...never a frontmatter flag the log would have to mutate.
        for forbidden in ["status", "state", "complete", "completed"] {
            assert_eq!(
                log.set_field(forbidden, "final"),
                Err(MemlogError::InvalidField)
            );
        }
    }

    #[test]
    fn descriptive_frontmatter_renders_before_the_entries() {
        let mut log = Memlog::open();
        log.set_field("topic", "Onboarding flow").expect("topic");
        log.set_field("goal", "lift week-1 retention")
            .expect("goal");
        log.append(MemlogEntryKind::Note, None, "first", UnixMillis(1))
            .expect("entry");

        let rendered = log.render();
        assert!(
            rendered.starts_with("---\ngoal: lift week-1 retention\ntopic: Onboarding flow\n---\n")
        );
        assert!(rendered.ends_with("- first\n"));
    }

    #[test]
    fn an_empty_log_renders_only_its_frontmatter() {
        let mut log = Memlog::open();
        log.set_field("topic", "Nothing yet").expect("topic");

        assert_eq!(log.render(), "---\ntopic: Nothing yet\n---\n");
        assert!(log.is_empty());
    }

    #[test]
    fn bounds_and_field_shapes_fail_closed() {
        let mut log = Memlog::open();
        for empty in ["", "   ", "\n\t "] {
            assert_eq!(
                log.append(MemlogEntryKind::Note, None, empty, UnixMillis(1)),
                Err(MemlogError::BoundsViolation)
            );
        }
        assert_eq!(
            log.append(
                MemlogEntryKind::Note,
                None,
                &"x".repeat(4_097),
                UnixMillis(1)
            ),
            Err(MemlogError::BoundsViolation)
        );
        assert_eq!(
            log.append(MemlogEntryKind::Note, Some(""), "text", UnixMillis(1)),
            Err(MemlogError::BoundsViolation)
        );
        for bad_key in ["", "Topic", "with space", "with-dash"] {
            assert_eq!(log.set_field(bad_key, "v"), Err(MemlogError::InvalidField));
        }
        assert_eq!(
            log.set_field("topic", "  "),
            Err(MemlogError::BoundsViolation)
        );
        assert!(log.is_empty());
    }

    #[test]
    fn a_derived_artifact_reads_the_log_and_never_mutates_it() {
        let mut log = Memlog::open();
        log.append(
            MemlogEntryKind::Decision,
            None,
            "boring technology",
            UnixMillis(1),
        )
        .expect("decision");
        log.append(
            MemlogEntryKind::Question,
            None,
            "which tenant model?",
            UnixMillis(2),
        )
        .expect("question");
        log.append(
            MemlogEntryKind::Decision,
            None,
            "one shared queue",
            UnixMillis(3),
        )
        .expect("decision");

        let decisions = log.entries_of(MemlogEntryKind::Decision);
        let questions = log.entries_of(MemlogEntryKind::Question);

        assert_eq!(decisions.len(), 2);
        assert_eq!(questions.len(), 1);
        // Deriving twice gives the same answer: the log is unchanged by reading.
        assert_eq!(log.entries_of(MemlogEntryKind::Decision).len(), 2);
        assert_eq!(log.entries().len(), 3);
    }
}
