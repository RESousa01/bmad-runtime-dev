//! Ordered architecture-creation progression — the state-machine packet the
//! sealed `architecture-create` guidance requires before the capability may be
//! offered.
//!
//! Every other reviewed capability answers one reviewed request and terminates.
//! The sealed architecture guidance is different: it instructs the model to
//! "work only from the reviewed context snapshot and current host-selected
//! step" and states that "the host validates progression". This module is that
//! validator. It owns the ordered step list, refuses to skip, repeat, or
//! finalize early, assigns the stable decision identifiers the guidance
//! requires, and distills the accepted steps into one inert spine artifact.
//!
//! The session carries no authority. It accepts only already-validated
//! [`BmadDocumentArtifact`] values — the same bounded archetype the generic
//! capability lane verifies untrusted model output into — and it produces one
//! more of the same. Nothing here reads, writes, or proposes a workspace
//! change; the spine is an inert document like any other document artifact.

use crate::{ContractId, UnixMillis};

use super::capability::{BmadDocumentArtifact, BmadDocumentLogEntry, BmadDocumentSection};
use super::memlog::{Memlog, MemlogEntryKind, MemlogError};

pub const BMAD_ARCHITECTURE_SESSION_SCHEMA: &str = "sapphirus.bmad-architecture-session.v1";

/// Cumulative decision-log entries across the whole session. Each accepted
/// step section becomes one `AD-n`, so this also bounds total step sections.
const DECISION_LIMIT: usize = 96;
/// Mirrors [`BmadDocumentArtifact`]'s own open-question bound so the distilled
/// spine can always carry the merged set.
const OPEN_QUESTION_LIMIT: usize = 64;
/// Mirrors the artifact prose bound: a step whose rendered spine body would
/// exceed it fails closed at acceptance, not after seven more model calls.
const RENDERED_STEP_LIMIT: usize = 32_768;
const SPINE_TITLE: &str = "Architecture spine";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadArchitectureError {
    /// The submitted step is not the one the host selected.
    StepOutOfOrder,
    /// The submitted step was already accepted; steps are append-only.
    StepAlreadyAccepted,
    /// Every step is accepted; the session can only be finalized now.
    ProgressionComplete,
    /// Finalization was requested before every step was addressed.
    IncompleteProgression,
    /// The session is already finalized and is immutable.
    SessionFinalized,
    /// A reviewed bound would be exceeded.
    BoundsViolation,
    /// Acceptance timestamps must not move backwards.
    NonMonotonicClock,
}

/// The ordered breadth this progression sweeps, closing with `Deferred`.
///
/// Upstream prescribes no fixed step list — it requires that the spine sweep
/// "the breadth the altitude owns", with every structural dimension decided,
/// deferred, or recorded as an open question, and names the operational
/// envelope as the one most often left silent. These steps are Forge's
/// host-owned reading of that breadth, not sealed BMAD source; the order is
/// fixed so a decision identifier minted in one run means the same thing in
/// every other run.
///
/// Right-sizing still applies within a step: upstream wants "a few decisions
/// for a small thing, comprehensive for a platform", so a step may legitimately
/// resolve to a single decision or to a deferral.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadArchitectureStep {
    SystemContext,
    Components,
    TrustBoundaries,
    Data,
    FailureHandling,
    Observability,
    Deployment,
    Verification,
    /// What the spine deliberately does *not* decide.
    ///
    /// Upstream treats this as part of the spine proper — "`Deferred` naming
    /// what it won't decide" — and it is the counterpart to the discriminator
    /// test: a call that fails the test is named here rather than fixed. Without
    /// it a progression can only record decisions, so everything deliberately
    /// left open is indistinguishable from something forgotten.
    Deferred,
}

impl BmadArchitectureStep {
    /// The closed, ordered progression. Index in this array is the step order.
    pub const ORDERED: [Self; 9] = [
        Self::SystemContext,
        Self::Components,
        Self::TrustBoundaries,
        Self::Data,
        Self::FailureHandling,
        Self::Observability,
        Self::Deployment,
        Self::Verification,
        Self::Deferred,
    ];

    /// Stable wire code for this step.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SystemContext => "system_context",
            Self::Components => "components",
            Self::TrustBoundaries => "trust_boundaries",
            Self::Data => "data",
            Self::FailureHandling => "failure_handling",
            Self::Observability => "observability",
            Self::Deployment => "deployment",
            Self::Verification => "verification",
            Self::Deferred => "deferred",
        }
    }

    /// Human-readable step title shown in review and in the spine.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::SystemContext => "System context",
            Self::Components => "Components",
            Self::TrustBoundaries => "Trust boundaries",
            Self::Data => "Data",
            Self::FailureHandling => "Failure handling",
            Self::Observability => "Observability",
            Self::Deployment => "Deployment",
            Self::Verification => "Verification",
            Self::Deferred => "Deferred",
        }
    }

    /// What this step must settle. Host-owned instruction text placed in the
    /// reviewed context so the model answers this step and no other.
    #[must_use]
    pub const fn objective(self) -> &'static str {
        match self {
            Self::SystemContext => {
                "Fix the problem this system solves, who and what it interacts with, and the \
                 boundary of what is in scope."
            }
            Self::Components => {
                "Fix the units the system is built from and the dependency rules between them."
            }
            Self::TrustBoundaries => {
                "Fix where authority changes hands, what each side may assume, and what must be \
                 re-validated at the boundary."
            }
            Self::Data => {
                "Fix who owns each piece of shared state, how it is mutated, and what must stay \
                 consistent."
            }
            Self::FailureHandling => {
                "Fix what is allowed to fail, what must fail closed, and how partial failure is \
                 contained."
            }
            Self::Observability => {
                "Fix what must be observable to operate and to diagnose the failures named above."
            }
            Self::Deployment => {
                "Fix the deployment and environment envelope, including how a change reaches \
                 production and how it is withdrawn."
            }
            Self::Verification => {
                "Fix how each decision above is verified, and what evidence a reviewer needs."
            }
            Self::Deferred => {
                "Name what this spine deliberately does not decide, and why each one is safe to \
                 leave to the level below — a call that is obvious, or not a real trade-off, or \
                 that two units one level down could not choose incompatibly. Keep this distinct \
                 from an open question you still need answered."
            }
        }
    }

    /// Zero-based position in [`Self::ORDERED`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ORDERED
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0)
    }

    /// One-based step number for presentation.
    #[must_use]
    pub fn number(self) -> usize {
        self.index().saturating_add(1)
    }

    /// Resolves a wire code back to its step.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ORDERED
            .into_iter()
            .find(|candidate| candidate.code() == code)
    }

    /// Total number of steps in the progression.
    #[must_use]
    pub const fn count() -> usize {
        Self::ORDERED.len()
    }
}

/// One accepted step: the step it answers and the artifact that answered it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadArchitectureStepRecord {
    pub step: BmadArchitectureStep,
    pub artifact: BmadDocumentArtifact,
    pub accepted_at: UnixMillis,
}

/// One decision-log entry with the stable identifier the sealed guidance
/// requires. Identifiers are minted by the host in acceptance order and are
/// never renumbered or reused.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadArchitectureDecision {
    pub decision_id: String,
    /// What the decision constrains.
    ///
    /// Upstream requires every `AD` to carry Binds, Prevents and Rule — that
    /// triple is what makes a spine entry checkable by a later builder instead
    /// of prose. The model writes them into the section body; the runtime
    /// extracts them here so an entry that omits one is *visible* rather than
    /// silently accepted as a decision.
    pub binds: Option<String>,
    /// The divergence the decision exists to stop.
    pub prevents: Option<String>,
    /// The call itself, stated so a builder can comply without the reasoning.
    pub rule: Option<String>,
    pub step: BmadArchitectureStep,
    pub heading: String,
    pub body: String,
}

/// One bounded architecture-creation conversation whose progression the host
/// owns. Append-only until finalized, then immutable.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadArchitectureSession {
    schema_version: &'static str,
    session_id: ContractId,
    workspace_id: ContractId,
    steps: Vec<BmadArchitectureStepRecord>,
    open_questions: Vec<String>,
    opened_at: UnixMillis,
    #[serde(skip_serializing_if = "Option::is_none")]
    spine: Option<BmadDocumentArtifact>,
}

impl BmadArchitectureSession {
    /// Opens an empty session bound to one workspace.
    #[must_use]
    pub fn open(session_id: ContractId, workspace_id: ContractId, opened_at: UnixMillis) -> Self {
        Self {
            schema_version: BMAD_ARCHITECTURE_SESSION_SCHEMA,
            session_id,
            workspace_id,
            steps: Vec::new(),
            open_questions: Vec::new(),
            opened_at,
            spine: None,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> &ContractId {
        &self.session_id
    }

    #[must_use]
    pub const fn workspace_id(&self) -> &ContractId {
        &self.workspace_id
    }

    #[must_use]
    pub const fn opened_at(&self) -> UnixMillis {
        self.opened_at
    }

    #[must_use]
    pub fn accepted_step_count(&self) -> usize {
        self.steps.len()
    }

    #[must_use]
    pub fn steps(&self) -> &[BmadArchitectureStepRecord] {
        &self.steps
    }

    #[must_use]
    pub const fn is_finalized(&self) -> bool {
        self.spine.is_some()
    }

    #[must_use]
    pub const fn spine(&self) -> Option<&BmadDocumentArtifact> {
        self.spine.as_ref()
    }

    /// The step the host will select next, or `None` once every step is
    /// accepted or the session is finalized.
    #[must_use]
    pub fn current_step(&self) -> Option<BmadArchitectureStep> {
        if self.is_finalized() {
            return None;
        }
        BmadArchitectureStep::ORDERED.get(self.steps.len()).copied()
    }

    /// The merged, de-duplicated open questions carried so far.
    #[must_use]
    pub fn open_questions(&self) -> &[String] {
        &self.open_questions
    }

    /// Decision identifiers whose entry is missing part of the `AD` triple.
    ///
    /// A spine entry without Binds, Prevents and Rule is prose, not a decision
    /// a later builder can comply with. Reporting them lets the host say which
    /// entries fall short rather than presenting the spine as complete.
    #[must_use]
    pub fn decisions_missing_structure(&self) -> Vec<String> {
        self.decision_log()
            .into_iter()
            .filter(|decision| {
                decision.binds.is_none() || decision.prevents.is_none() || decision.rule.is_none()
            })
            .map(|decision| decision.decision_id)
            .collect()
    }

    /// The decision log: one stable `AD-n` per accepted step section, in
    /// acceptance order.
    #[must_use]
    pub fn decision_log(&self) -> Vec<BmadArchitectureDecision> {
        let mut decisions = Vec::new();
        for record in &self.steps {
            for section in &record.artifact.sections {
                let ordinal = decisions.len().saturating_add(1);
                decisions.push(BmadArchitectureDecision {
                    decision_id: format!("AD-{ordinal}"),
                    binds: Self::labelled_part(&section.body, "binds"),
                    prevents: Self::labelled_part(&section.body, "prevents"),
                    rule: Self::labelled_part(&section.body, "rule"),
                    step: record.step,
                    heading: section.heading.clone(),
                    body: section.body.clone(),
                });
            }
        }
        decisions
    }

    /// Extracts one labelled part of an `AD` body.
    fn labelled_part(body: &str, label: &str) -> Option<String> {
        super::memlog::labelled_body_part(body, label)
    }

    /// Projects the progression as an upstream-shaped memlog.
    ///
    /// This is the bridge to the method's own working-memory mechanic: the
    /// spine is *derived* from an append-only chronological record, not edited
    /// in place. Each accepted step contributes an `event` marking the step and
    /// one `decision` per section, in the order they happened; unresolved items
    /// land as `question` entries. Rendering it produces the same `.memlog.md`
    /// a `bmad-architecture` run would leave behind.
    ///
    /// # Errors
    ///
    /// Returns [`MemlogError`] if a step's content exceeds a reviewed memlog
    /// bound — the same ceilings any other run is held to.
    pub fn to_memlog(&self) -> Result<Memlog, MemlogError> {
        let mut log = Memlog::open();
        log.set_field("topic", "Architecture spine")?;
        log.set_field("altitude", "host-validated ordered progression")?;
        let mut minted = 0_usize;
        for record in &self.steps {
            log.append(
                MemlogEntryKind::Event,
                None,
                &format!(
                    "started step {}: {}",
                    record.step.number(),
                    record.step.title()
                ),
                record.accepted_at,
            )?;
            for section in &record.artifact.sections {
                minted = minted.saturating_add(1);
                log.append(
                    MemlogEntryKind::Decision,
                    None,
                    &format!("AD-{minted} — {}: {}", section.heading, section.body),
                    record.accepted_at,
                )?;
            }
            for question in &record.artifact.open_questions {
                log.append(
                    MemlogEntryKind::Question,
                    None,
                    question,
                    record.accepted_at,
                )?;
            }
        }
        if self.is_finalized() {
            let finalized_at = self
                .steps
                .last()
                .map_or(self.opened_at, |record| record.accepted_at);
            // Terminal moments are events, never a status field.
            log.append(
                MemlogEntryKind::Event,
                None,
                "spine finalized",
                finalized_at,
            )?;
        }
        Ok(log)
    }

    /// Accepts the answer to the host-selected step.
    ///
    /// # Errors
    ///
    /// Fails closed on any progression violation: a finalized session
    /// ([`BmadArchitectureError::SessionFinalized`]), a completed progression
    /// ([`BmadArchitectureError::ProgressionComplete`]), a step already
    /// accepted ([`BmadArchitectureError::StepAlreadyAccepted`]), any step
    /// other than the selected one ([`BmadArchitectureError::StepOutOfOrder`]),
    /// a backwards clock ([`BmadArchitectureError::NonMonotonicClock`]), or a
    /// cumulative bound ([`BmadArchitectureError::BoundsViolation`]).
    pub fn accept_step(
        &mut self,
        step: BmadArchitectureStep,
        artifact: BmadDocumentArtifact,
        accepted_at: UnixMillis,
    ) -> Result<BmadArchitectureStep, BmadArchitectureError> {
        if self.is_finalized() {
            return Err(BmadArchitectureError::SessionFinalized);
        }
        let Some(expected) = self.current_step() else {
            return Err(BmadArchitectureError::ProgressionComplete);
        };
        if step != expected {
            return Err(if step.index() < expected.index() {
                BmadArchitectureError::StepAlreadyAccepted
            } else {
                BmadArchitectureError::StepOutOfOrder
            });
        }
        let previous_at = self
            .steps
            .last()
            .map_or(self.opened_at, |record| record.accepted_at);
        if accepted_at < previous_at {
            return Err(BmadArchitectureError::NonMonotonicClock);
        }

        // Bound the cumulative log and the rendered spine body now, so a
        // violation surfaces on the step that caused it.
        let decisions_so_far: usize = self
            .steps
            .iter()
            .map(|record| record.artifact.sections.len())
            .sum();
        if decisions_so_far.saturating_add(artifact.sections.len()) > DECISION_LIMIT {
            return Err(BmadArchitectureError::BoundsViolation);
        }
        if rendered_step_body(&artifact.sections, decisions_so_far).len() > RENDERED_STEP_LIMIT {
            return Err(BmadArchitectureError::BoundsViolation);
        }
        let merged_questions = merge_questions(&self.open_questions, &artifact.open_questions);
        if merged_questions.len() > OPEN_QUESTION_LIMIT {
            return Err(BmadArchitectureError::BoundsViolation);
        }

        self.open_questions = merged_questions;
        self.steps.push(BmadArchitectureStepRecord {
            step,
            artifact,
            accepted_at,
        });
        Ok(step)
    }

    /// Distills the accepted steps into one inert spine artifact.
    ///
    /// # Errors
    ///
    /// Returns [`BmadArchitectureError::IncompleteProgression`] unless every
    /// step is accepted, [`BmadArchitectureError::SessionFinalized`] on a
    /// second finalization, and [`BmadArchitectureError::BoundsViolation`] if
    /// the distilled artifact would exceed a reviewed bound.
    pub fn finalize(
        &mut self,
        finalized_at: UnixMillis,
    ) -> Result<BmadDocumentArtifact, BmadArchitectureError> {
        if self.is_finalized() {
            return Err(BmadArchitectureError::SessionFinalized);
        }
        if self.steps.len() != BmadArchitectureStep::count() {
            return Err(BmadArchitectureError::IncompleteProgression);
        }
        let previous_at = self
            .steps
            .last()
            .map_or(self.opened_at, |record| record.accepted_at);
        if finalized_at < previous_at {
            return Err(BmadArchitectureError::NonMonotonicClock);
        }

        let mut sections = Vec::with_capacity(self.steps.len());
        let mut minted = 0_usize;
        for record in &self.steps {
            sections.push(BmadDocumentSection {
                heading: format!("{}. {}", record.step.number(), record.step.title()),
                body: rendered_step_body(&record.artifact.sections, minted),
            });
            minted = minted.saturating_add(record.artifact.sections.len());
        }

        // Diagrams do not concatenate. The spine carries the first supplied
        // diagram — system context is step one, so that is the structural view
        // — and the remaining diagrams stay in their step records.
        let mermaid_text = self
            .steps
            .iter()
            .find_map(|record| record.artifact.mermaid_text.clone());
        let evidence_refs = self
            .steps
            .iter()
            .flat_map(|record| record.artifact.evidence_refs.iter().cloned())
            .collect::<Vec<_>>();

        // The spine carries the log it was distilled from. For this capability
        // that is not a projection of the finished document: every entry was
        // recorded as its step landed, which is exactly upstream's ordering.
        let decision_log = self
            .to_memlog()
            .map_err(|_| BmadArchitectureError::BoundsViolation)?
            .entries()
            .iter()
            .map(|entry| BmadDocumentLogEntry {
                kind: entry.kind,
                subject: None,
                text: entry.text.clone(),
            })
            .collect::<Vec<_>>();

        let spine = BmadDocumentArtifact::new(
            SPINE_TITLE.to_owned(),
            sections,
            decision_log,
            evidence_refs,
            self.open_questions.clone(),
            mermaid_text,
        )
        .map_err(|_| BmadArchitectureError::BoundsViolation)?;
        self.spine = Some(spine.clone());
        Ok(spine)
    }
}

/// Renders one step's sections as decision-log entries, continuing the stable
/// `AD-n` numbering from `already_minted`.
fn rendered_step_body(sections: &[BmadDocumentSection], already_minted: usize) -> String {
    let mut body = String::new();
    for (offset, section) in sections.iter().enumerate() {
        let ordinal = already_minted.saturating_add(offset).saturating_add(1);
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str("AD-");
        body.push_str(&ordinal.to_string());
        body.push_str(" — ");
        body.push_str(&section.heading);
        body.push('\n');
        body.push_str(&section.body);
    }
    body
}

/// Merges open questions preserving first-seen order and dropping duplicates.
fn merge_questions(carried: &[String], incoming: &[String]) -> Vec<String> {
    let mut merged = carried.to_vec();
    for question in incoming {
        if !merged.iter().any(|existing| existing == question) {
            merged.push(question.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        BmadArchitectureError, BmadArchitectureSession, BmadArchitectureStep, MemlogEntryKind,
        BMAD_ARCHITECTURE_SESSION_SCHEMA,
    };
    use crate::{BmadDocumentArtifact, BmadDocumentSection, ContractId, UnixMillis};

    fn session() -> BmadArchitectureSession {
        BmadArchitectureSession::open(
            ContractId::new("archsess_test").expect("session id"),
            ContractId::new("ws_test").expect("workspace id"),
            UnixMillis(1_000),
        )
    }

    fn artifact(heading: &str) -> BmadDocumentArtifact {
        BmadDocumentArtifact::new(
            format!("Step: {heading}"),
            vec![BmadDocumentSection {
                heading: heading.to_owned(),
                body: format!("Decided: {heading}."),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .expect("step artifact")
    }

    fn accept_all(session: &mut BmadArchitectureSession) {
        for (offset, step) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            let at = UnixMillis(2_000 + u64::try_from(offset).unwrap_or(0));
            session
                .accept_step(step, artifact(step.title()), at)
                .expect("ordered acceptance");
        }
    }

    #[test]
    fn progression_is_ordered_and_complete() {
        let mut session = session();
        assert_eq!(session.schema_version, BMAD_ARCHITECTURE_SESSION_SCHEMA);
        assert_eq!(
            session.current_step(),
            Some(BmadArchitectureStep::SystemContext)
        );

        accept_all(&mut session);

        assert_eq!(session.accepted_step_count(), BmadArchitectureStep::count());
        assert_eq!(session.current_step(), None);
    }

    #[test]
    fn skipping_a_step_fails_closed() {
        let mut session = session();

        assert_eq!(
            session.accept_step(
                BmadArchitectureStep::Deployment,
                artifact("Deployment"),
                UnixMillis(2_000),
            ),
            Err(BmadArchitectureError::StepOutOfOrder)
        );
        assert_eq!(session.accepted_step_count(), 0);
    }

    #[test]
    fn repeating_an_accepted_step_fails_closed() {
        let mut session = session();
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(2_000),
            )
            .expect("first step");

        assert_eq!(
            session.accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context again"),
                UnixMillis(2_001),
            ),
            Err(BmadArchitectureError::StepAlreadyAccepted)
        );
        assert_eq!(session.accepted_step_count(), 1);
    }

    #[test]
    fn finalizing_early_fails_closed() {
        let mut session = session();
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(2_000),
            )
            .expect("first step");

        assert_eq!(
            session.finalize(UnixMillis(3_000)).map(|_| ()),
            Err(BmadArchitectureError::IncompleteProgression)
        );
        assert!(!session.is_finalized());
    }

    #[test]
    fn a_finalized_session_is_immutable() {
        let mut session = session();
        accept_all(&mut session);
        let spine = session.finalize(UnixMillis(9_000)).expect("spine");

        assert_eq!(spine.title, "Architecture spine");
        assert_eq!(spine.sections.len(), BmadArchitectureStep::count());
        assert_eq!(
            session.finalize(UnixMillis(9_001)).map(|_| ()),
            Err(BmadArchitectureError::SessionFinalized)
        );
        assert_eq!(
            session.accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(9_002),
            ),
            Err(BmadArchitectureError::SessionFinalized)
        );
    }

    #[test]
    fn accepting_after_the_last_step_fails_closed() {
        let mut session = session();
        accept_all(&mut session);

        // The progression is complete but not yet distilled: no step — the
        // last one included — may be answered again. Only `finalize` may run.
        for step in [
            BmadArchitectureStep::Verification,
            BmadArchitectureStep::SystemContext,
        ] {
            assert_eq!(
                session.accept_step(step, artifact(step.title()), UnixMillis(8_000)),
                Err(BmadArchitectureError::ProgressionComplete)
            );
        }
        assert_eq!(session.accepted_step_count(), BmadArchitectureStep::count());
    }

    /// Upstream requires every `AD` to carry Binds, Prevents and Rule; without
    /// them a spine entry is prose a later builder cannot comply with. The
    /// runtime extracts the triple so a short entry is visible rather than
    /// silently counted as a decision.
    #[test]
    fn decisions_expose_the_binds_prevents_rule_triple() {
        let structured = BmadDocumentArtifact::new(
            "Step: Components".to_owned(),
            vec![BmadDocumentSection {
                heading: "Module boundaries".to_owned(),
                body: "- **Binds** — every crate depending on the kernel\n\
                       Prevents: two modules choosing incompatible error shapes\n\
                       **Rule:** errors cross a boundary as the shared enum only"
                    .to_owned(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .expect("structured artifact");

        let mut session = session();
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(2_000),
            )
            .expect("first step");
        session
            .accept_step(
                BmadArchitectureStep::Components,
                structured,
                UnixMillis(2_001),
            )
            .expect("second step");

        let log = session.decision_log();
        let structured_decision = &log[1];
        assert_eq!(
            structured_decision.binds.as_deref(),
            Some("every crate depending on the kernel"),
        );
        assert_eq!(
            structured_decision.prevents.as_deref(),
            Some("two modules choosing incompatible error shapes"),
        );
        assert_eq!(
            structured_decision.rule.as_deref(),
            Some("errors cross a boundary as the shared enum only"),
        );

        // The first step's body is prose, so it is reported as falling short
        // rather than passing as a decision.
        assert_eq!(log[0].binds, None);
        assert_eq!(
            session.decisions_missing_structure(),
            vec!["AD-1".to_owned()],
        );
    }

    #[test]
    fn decision_identifiers_are_stable_and_continuous() {
        let mut session = session();
        accept_all(&mut session);

        let log = session.decision_log();
        assert_eq!(log.len(), BmadArchitectureStep::count());
        for (offset, decision) in log.iter().enumerate() {
            assert_eq!(decision.decision_id, format!("AD-{}", offset + 1));
            assert_eq!(decision.step, BmadArchitectureStep::ORDERED[offset]);
        }

        // Identifiers minted earlier keep their meaning in the distilled spine.
        let spine = session.finalize(UnixMillis(9_000)).expect("spine");
        assert!(spine.sections[0].body.starts_with("AD-1 — System context"));
        assert!(spine.sections[7].body.starts_with("AD-8 — Verification"));
    }

    #[test]
    fn open_questions_merge_without_duplicates() {
        let mut session = session();
        let mut with_questions = |step: BmadArchitectureStep, questions: Vec<String>, at: u64| {
            let artifact = BmadDocumentArtifact::new(
                format!("Step: {}", step.title()),
                vec![BmadDocumentSection {
                    heading: step.title().to_owned(),
                    body: "Decided.".to_owned(),
                }],
                Vec::new(),
                Vec::new(),
                questions,
                None,
            )
            .expect("artifact");
            session
                .accept_step(step, artifact, UnixMillis(at))
                .expect("acceptance");
        };
        with_questions(
            BmadArchitectureStep::SystemContext,
            vec!["Which tenant model?".to_owned()],
            2_000,
        );
        with_questions(
            BmadArchitectureStep::Components,
            vec!["Which tenant model?".to_owned(), "Which queue?".to_owned()],
            2_001,
        );

        assert_eq!(
            session.open_questions(),
            ["Which tenant model?", "Which queue?"]
        );
    }

    #[test]
    fn a_backwards_clock_fails_closed() {
        let mut session = session();
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(5_000),
            )
            .expect("first step");

        assert_eq!(
            session.accept_step(
                BmadArchitectureStep::Components,
                artifact("Components"),
                UnixMillis(4_999),
            ),
            Err(BmadArchitectureError::NonMonotonicClock)
        );
    }

    #[test]
    fn the_cumulative_decision_bound_is_enforced_at_the_offending_step() {
        let mut session = session();
        let wide = BmadDocumentArtifact::new(
            "Wide step".to_owned(),
            (0..64)
                .map(|index| BmadDocumentSection {
                    heading: format!("Decision {index}"),
                    body: "Body.".to_owned(),
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .expect("wide artifact");
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                wide.clone(),
                UnixMillis(2_000),
            )
            .expect("first wide step");

        // 64 + 64 exceeds the 96-entry log bound.
        assert_eq!(
            session.accept_step(BmadArchitectureStep::Components, wide, UnixMillis(2_001)),
            Err(BmadArchitectureError::BoundsViolation)
        );
        assert_eq!(session.accepted_step_count(), 1);
    }

    #[test]
    fn the_progression_projects_an_upstream_shaped_memlog() {
        let mut session = session();
        accept_all(&mut session);
        session.finalize(UnixMillis(9_000)).expect("spine");

        let log = session.to_memlog().expect("memlog");
        let rendered = log.render();

        // Frontmatter is descriptive only; the log carries no status field.
        assert!(rendered.starts_with(
            "---
"
        ));
        assert!(!rendered.contains("status:"));

        // One event per step, plus the terminal event — recorded as an entry.
        assert_eq!(
            log.entries_of(MemlogEntryKind::Event).len(),
            BmadArchitectureStep::count() + 1
        );
        assert!(rendered.contains(
            "- (event) started step 1: System context
"
        ));
        assert!(rendered.contains(
            "- (event) spine finalized
"
        ));

        // Decisions keep the stable identifiers the spine cites.
        let decisions = log.entries_of(MemlogEntryKind::Decision);
        assert_eq!(decisions.len(), BmadArchitectureStep::count());
        assert!(decisions[0].text.starts_with("AD-1 — System context:"));

        // Deriving is read-only: the same log renders identically twice.
        assert_eq!(session.to_memlog().expect("memlog").render(), rendered);
    }

    #[test]
    fn an_unfinished_progression_records_no_terminal_event() {
        let mut session = session();
        session
            .accept_step(
                BmadArchitectureStep::SystemContext,
                artifact("System context"),
                UnixMillis(2_000),
            )
            .expect("first step");

        let log = session.to_memlog().expect("memlog");

        assert_eq!(log.entries_of(MemlogEntryKind::Event).len(), 1);
        assert!(!log.render().contains("spine finalized"));
    }

    #[test]
    fn step_codes_round_trip_and_cover_the_progression() {
        for (offset, step) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            assert_eq!(BmadArchitectureStep::from_code(step.code()), Some(step));
            assert_eq!(step.index(), offset);
            assert_eq!(step.number(), offset + 1);
            assert!(!step.objective().is_empty());
        }
        assert_eq!(BmadArchitectureStep::from_code("finalize"), None);
        // `Deferred` closes the progression: upstream requires every spine to
        // name what it will not decide, distinct from an open question.
        assert_eq!(BmadArchitectureStep::count(), 9);
        assert_eq!(
            BmadArchitectureStep::ORDERED.last(),
            Some(&BmadArchitectureStep::Deferred),
        );
    }

    #[test]
    fn the_spine_carries_the_first_supplied_diagram() {
        let mut session = session();
        for (offset, step) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            let mermaid = match step {
                BmadArchitectureStep::Components => Some("graph TD; A-->B;".to_owned()),
                BmadArchitectureStep::Deployment => Some("graph TD; C-->D;".to_owned()),
                _ => None,
            };
            let artifact = BmadDocumentArtifact::new(
                format!("Step: {}", step.title()),
                vec![BmadDocumentSection {
                    heading: step.title().to_owned(),
                    body: "Decided.".to_owned(),
                }],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                mermaid,
            )
            .expect("artifact");
            session
                .accept_step(
                    step,
                    artifact,
                    UnixMillis(2_000 + u64::try_from(offset).unwrap_or(0)),
                )
                .expect("acceptance");
        }

        let spine = session.finalize(UnixMillis(9_000)).expect("spine");
        assert_eq!(spine.mermaid_text.as_deref(), Some("graph TD; A-->B;"));
    }
}
