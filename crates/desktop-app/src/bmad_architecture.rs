//! Host composition for the ordered architecture progression.
//!
//! The sealed `architecture-create` guidance is the only reviewed capability
//! whose foundation record carries `entrypointKind: "step_jit"`: it instructs
//! the model to answer "the current host-selected step" and states that "the
//! host validates progression". This module is that host.
//!
//! It rides the *existing* generic capability lane rather than adding a second
//! egress path. One architecture run is a sequence of ordinary reviewed
//! capability runs — same manifest, same single-use consent decision, same
//! untrusted-output verification — where the host:
//!
//! 1. selects the step and places it in the reviewed context, so the step the
//!    user consents to sending is covered by the manifest hash;
//! 2. carries the accepted decision log forward as reviewed context;
//! 3. accepts each step's durably stored, re-verified artifact in order,
//!    refusing to skip, repeat, or finalize early;
//! 4. distills the spine locally once every step is answered — no model call,
//!    no egress, no workspace effect.
//!
//! The live session is in-memory, like the generic coordinator it rides on, but
//! its *boundary* is durable: which session a workspace has open, and the
//! ordered runs that answered its steps. A restart therefore resumes at the step
//! it interrupted rather than reopening at step one — upstream's claim for a
//! memlog is exactly that "a fresh session can load it and continue".
//!
//! Resuming re-reads each recorded step from its own capability run and
//! re-parses it through the same strict constructor that accepted it, so a
//! resumed progression stands on evidence the store still holds rather than on
//! remembered state, and fails closed if any of it no longer verifies.

use desktop_egress::{ContextCandidate, ContextClassification};
use desktop_runtime::{
    canonical_hash, BmadArchitectureError, BmadArchitectureSession, BmadArchitectureStep,
    BmadDocumentArtifact, ContractId, RelativeWorkspacePath, UnixMillis,
};
use desktop_store::LocalStore;
use serde::Serialize;

use crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError;
use crate::bmad_model::context::derived_contract_id;

/// The one reviewed capability that runs as an ordered progression. Name it
/// only where the architecture run folder or its spine is specifically what is
/// meant; for "does this capability run stepwise at all", ask the sealed
/// foundation via [`is_stepwise_capability`] instead. Production dispatch reads
/// the sealed `entrypointKind`, so this literal now only names the fixture
/// capability the tests exercise.
#[cfg(test)]
pub(crate) const ARCHITECTURE_CAPABILITY_ID: &str = "bmm:bmad-architecture";

/// Whether the sealed foundation records this capability as running stepwise.
///
/// The foundation already carries `entrypointKind` per installed skill, and
/// `step_jit` is exactly the upstream marker for a skill the host drives one
/// step at a time. Routing on that record rather than on a hard-coded
/// capability id keeps the decision where the reviewed data lives — and means
/// a second stepwise capability needs no new branch here.
///
/// Today only `bmm:bmad-architecture` carries it, so this is behaviour-neutral.
#[must_use]
pub(crate) fn is_stepwise_capability(
    foundation: &crate::bmad_foundation::BmadLoadedFoundation,
    capability_id: &str,
) -> bool {
    // Capability ids are `<product namespace>:<upstream skill name>`; the
    // catalog is keyed by the skill name.
    let Some((_, skill_name)) = capability_id.split_once(':') else {
        return false;
    };
    foundation.catalog().installed_skills.iter().any(|skill| {
        skill.skill_name == skill_name
            && skill.entrypoint_kind == desktop_runtime::BmadEntrypointKind::StepJit
    })
}

/// What the renderer is told about the progression. Present only on the
/// stepwise capability; every other capability omits it entirely.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchitectureProgressProjection {
    pub step_code: &'static str,
    pub step_title: &'static str,
    pub step_number: usize,
    pub step_count: usize,
    pub finalized: bool,
    /// The distilled spine, present exactly when `finalized` is true.
    pub spine: Option<BmadDocumentArtifact>,
}

/// The run folder a finalized progression writes into, matching upstream's
/// "the spine and its `.memlog.md` live in one run folder" shape.
const ARCHITECTURE_RUN_FOLDER: &str = "docs/architecture";
const SPINE_FILE_NAME: &str = "ARCHITECTURE-SPINE.md";

impl BmadArchitectureCoordinator {
    /// Renders a finalized progression as the two files a real
    /// `bmad-architecture` run leaves behind: the derived spine and the
    /// append-only memlog it was derived *from*.
    ///
    /// These are returned as ordinary candidate changes, so they enter the
    /// same D3 review as any other proposed edit. Nothing is written here.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityCoordinatorError::Conflict`] when no finalized
    /// progression exists for the workspace, and
    /// [`BmadCapabilityCoordinatorError::OutputRejected`] when the run cannot
    /// render within its reviewed bounds.
    pub(crate) fn finalized_run_files(
        &self,
        workspace_id: &ContractId,
    ) -> Result<Vec<(RelativeWorkspacePath, String)>, BmadCapabilityCoordinatorError> {
        let session = self
            .active
            .as_ref()
            .filter(|session| session.workspace_id() == workspace_id)
            .filter(|session| session.is_finalized())
            .ok_or(BmadCapabilityCoordinatorError::Conflict)?;
        let spine = session
            .spine()
            .ok_or(BmadCapabilityCoordinatorError::Conflict)?;
        let memlog = session
            .to_memlog()
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)?;

        let spine_path =
            RelativeWorkspacePath::new(format!("{ARCHITECTURE_RUN_FOLDER}/{SPINE_FILE_NAME}"))
                .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
        let memlog_path = RelativeWorkspacePath::new(format!(
            "{ARCHITECTURE_RUN_FOLDER}/{}",
            desktop_runtime::MEMLOG_FILE_NAME
        ))
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;

        Ok(vec![
            (spine_path, render_spine(spine)),
            (memlog_path, memlog.render()),
        ])
    }
}

/// Renders the distilled spine as the Markdown document downstream skills read.
fn render_spine(spine: &BmadDocumentArtifact) -> String {
    use std::fmt::Write as _;
    let mut document = String::new();
    let _ = writeln!(document, "# {}", spine.title);
    for section in &spine.sections {
        let _ = write!(
            document,
            "
## {}

{}
",
            section.heading, section.body
        );
    }
    if let Some(diagram) = spine.mermaid_text.as_ref() {
        let _ = write!(
            document,
            "
## Diagram

```mermaid
{diagram}
```
"
        );
    }
    if !spine.open_questions.is_empty() {
        document.push_str(
            "
## Open questions

",
        );
        for question in &spine.open_questions {
            let _ = writeln!(document, "- {question}");
        }
    }
    document
}

/// The progression a prepared-but-unanswered run will advance. A review is
/// never finalized and never carries a spine.
#[must_use]
pub(crate) fn selected_step_progress(step: BmadArchitectureStep) -> ArchitectureProgressProjection {
    ArchitectureProgressProjection {
        step_code: step.code(),
        step_title: step.title(),
        step_number: step.number(),
        step_count: BmadArchitectureStep::count(),
        finalized: false,
        spine: None,
    }
}

#[derive(Serialize)]
struct ArchitectureSessionIdentity<'a> {
    schema_version: &'static str,
    workspace_id: &'a ContractId,
    opened_at: UnixMillis,
}

/// Owns the at-most-one active architecture progression.
pub(crate) struct BmadArchitectureCoordinator {
    active: Option<BmadArchitectureSession>,
}

impl BmadArchitectureCoordinator {
    pub(crate) const fn new() -> Self {
        Self { active: None }
    }

    /// Withdraws any in-flight progression, for sign-out and
    /// workspace-revocation paths (ADR-0002). The stored per-step evidence is
    /// untouched; only the host's ability to continue the progression ends.
    pub(crate) fn invalidate(&mut self) {
        self.active = None;
    }

    /// Selects the step the next prepared run must answer, opening a session
    /// when none is active for this workspace.
    ///
    /// A finalized session, or one bound to a different workspace, is replaced
    /// by a fresh progression: a spine belongs to the workspace it was
    /// reasoned about, and is never continued into another.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityCoordinatorError::Integrity`] when the session
    /// identity cannot be derived.
    /// A restart clears `active`, so before opening a fresh progression the
    /// host asks the store whether this workspace has one to continue. That is
    /// the whole of upstream's claim for a memlog — "a fresh session can load
    /// it and continue" — and without it a restart silently restarted the
    /// architecture run at step one.
    pub(crate) fn select_step(
        &mut self,
        store: &LocalStore,
        workspace_id: &ContractId,
        now: UnixMillis,
    ) -> Result<BmadArchitectureStep, BmadCapabilityCoordinatorError> {
        let reusable = self.active.as_ref().is_some_and(|session| {
            session.workspace_id() == workspace_id && !session.is_finalized()
        });
        if !reusable {
            let restored = restore_session(store, workspace_id)?;
            self.active = Some(if let Some(restored) = restored {
                restored
            } else {
                let opened = open_session(workspace_id, now)?;
                store
                    .open_architecture_session(
                        opened.session_id().as_str(),
                        workspace_id.as_str(),
                        now.0,
                    )
                    .map_err(|_| BmadCapabilityCoordinatorError::Store)?;
                opened
            });
        }
        let session = self
            .active
            .as_ref()
            .ok_or(BmadCapabilityCoordinatorError::Integrity)?;
        session
            .current_step()
            .ok_or(BmadCapabilityCoordinatorError::Integrity)
    }

    /// The active session's durable identity, for recording an accepted step.
    #[must_use]
    pub(crate) fn active_session_id(&self, workspace_id: &ContractId) -> Option<ContractId> {
        self.active
            .as_ref()
            .filter(|session| session.workspace_id() == workspace_id)
            .map(|session| session.session_id().clone())
    }

    /// The step an already-prepared run answers, without opening or advancing
    /// anything. `None` when no live progression is bound to this workspace.
    #[must_use]
    pub(crate) fn current_step(&self, workspace_id: &ContractId) -> Option<BmadArchitectureStep> {
        self.active
            .as_ref()
            .filter(|session| session.workspace_id() == workspace_id)
            .and_then(BmadArchitectureSession::current_step)
    }

    /// Accepts one completed step's verified artifact and reports the
    /// resulting progression, distilling the spine on the final step.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityCoordinatorError::Conflict`] for any
    /// progression violation — no active session, a skipped or repeated step,
    /// a completed or finalized progression — and
    /// [`BmadCapabilityCoordinatorError::OutputRejected`] when the accepted
    /// artifacts cannot distill within the reviewed bounds.
    pub(crate) fn accept_step(
        &mut self,
        workspace_id: &ContractId,
        step: BmadArchitectureStep,
        artifact: BmadDocumentArtifact,
        accepted_at: UnixMillis,
    ) -> Result<ArchitectureProgressProjection, BmadCapabilityCoordinatorError> {
        let session = self
            .active
            .as_mut()
            .filter(|session| session.workspace_id() == workspace_id)
            .ok_or(BmadCapabilityCoordinatorError::Conflict)?;
        session
            .accept_step(step, artifact, accepted_at)
            .map_err(map_architecture_error)?;
        let complete = session.current_step().is_none();
        let spine = if complete {
            Some(
                session
                    .finalize(accepted_at)
                    .map_err(map_architecture_error)?,
            )
        } else {
            None
        };
        Ok(ArchitectureProgressProjection {
            step_code: step.code(),
            step_title: step.title(),
            step_number: step.number(),
            step_count: BmadArchitectureStep::count(),
            finalized: complete,
            spine,
        })
    }

    /// The reviewed-context candidates that bind one prepared run to the
    /// host-selected step and the decisions already accepted.
    ///
    /// # Errors
    ///
    /// Returns [`BmadCapabilityCoordinatorError::Integrity`] when a candidate
    /// identity cannot be constructed.
    pub(crate) fn step_context_candidates(
        &self,
        step: BmadArchitectureStep,
    ) -> Result<Vec<ContextCandidate>, BmadCapabilityCoordinatorError> {
        let decision_log = self
            .active
            .as_ref()
            .map_or_else(String::new, render_decision_log);
        Ok(vec![
            progression_candidate(
                "capctx_architecture_step",
                "review/bmad-capability/architecture-step.md",
                render_selected_step(step),
            )?,
            progression_candidate(
                "capctx_architecture_log",
                "review/bmad-capability/architecture-decision-log.md",
                decision_log,
            )?,
        ])
    }
}

/// Rebuilds a workspace's unfinalized progression from the store, or `None`
/// when there is nothing to continue.
///
/// Each step is re-read from its own capability run and re-parsed through the
/// same strict archetype constructor that accepted it, then replayed in order
/// through `accept_step`. A resumed progression therefore stands on evidence
/// the store still holds and the parser still accepts — never on remembered
/// state — and any row the current build can no longer verify fails the resume
/// rather than silently continuing from a partial spine.
fn restore_session(
    store: &LocalStore,
    workspace_id: &ContractId,
) -> Result<Option<BmadArchitectureSession>, BmadCapabilityCoordinatorError> {
    let Some(resumable) = store
        .resumable_architecture_session(workspace_id.as_str())
        .map_err(|_| BmadCapabilityCoordinatorError::Store)?
    else {
        return Ok(None);
    };
    let session_id = ContractId::new(resumable.session_id)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let mut session = BmadArchitectureSession::open(
        session_id,
        workspace_id.clone(),
        UnixMillis(resumable.opened_at_ms),
    );
    for record in resumable.steps {
        let step = session
            .current_step()
            .ok_or(BmadCapabilityCoordinatorError::Integrity)?;
        let run_id =
            ContractId::new(record.run_id).map_err(|_| BmadCapabilityCoordinatorError::Store)?;
        let stored = store
            .bmad_capability_run(&run_id)
            .map_err(|_| BmadCapabilityCoordinatorError::Store)?
            .and_then(|run| run.result_json)
            .ok_or(BmadCapabilityCoordinatorError::Store)?;
        let value: serde_json::Value = desktop_runtime::deserialize_strict_json(stored.as_bytes())
            .map_err(|_| BmadCapabilityCoordinatorError::OutputRejected)?;
        let desktop_runtime::BmadCapabilityOutput::DocumentArtifact(artifact) =
            crate::bmad_capability_host::parse_capability_result(&value)?
        else {
            return Err(BmadCapabilityCoordinatorError::ResultArchetypeMismatch);
        };
        session
            .accept_step(step, artifact, UnixMillis(record.accepted_at_ms))
            .map_err(map_architecture_error)?;
    }
    // A complete-but-unfinalized session cannot be continued or finalized here:
    // finalizing is the accepting run's job, and it records that durably.
    if session.current_step().is_none() {
        return Ok(None);
    }
    Ok(Some(session))
}

fn open_session(
    workspace_id: &ContractId,
    now: UnixMillis,
) -> Result<BmadArchitectureSession, BmadCapabilityCoordinatorError> {
    let digest = canonical_hash(
        "bmad-architecture-session-id",
        1,
        &ArchitectureSessionIdentity {
            schema_version: desktop_runtime::BMAD_ARCHITECTURE_SESSION_SCHEMA,
            workspace_id,
            opened_at: now,
        },
    )
    .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    let session_id = derived_contract_id("archsess", digest)
        .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?;
    Ok(BmadArchitectureSession::open(
        session_id,
        workspace_id.clone(),
        now,
    ))
}

const fn map_architecture_error(error: BmadArchitectureError) -> BmadCapabilityCoordinatorError {
    match error {
        BmadArchitectureError::BoundsViolation => BmadCapabilityCoordinatorError::OutputRejected,
        BmadArchitectureError::StepOutOfOrder
        | BmadArchitectureError::StepAlreadyAccepted
        | BmadArchitectureError::ProgressionComplete
        | BmadArchitectureError::IncompleteProgression
        | BmadArchitectureError::SessionFinalized
        | BmadArchitectureError::NonMonotonicClock => BmadCapabilityCoordinatorError::Conflict,
    }
}

fn progression_candidate(
    client_item_id: &str,
    relative_label: &str,
    content: String,
) -> Result<ContextCandidate, BmadCapabilityCoordinatorError> {
    Ok(ContextCandidate {
        client_item_id: ContractId::new(client_item_id)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        relative_label: RelativeWorkspacePath::new(relative_label)
            .map_err(|_| BmadCapabilityCoordinatorError::Integrity)?,
        semantic_role: "architecture_progression".to_owned(),
        language: Some("markdown".to_owned()),
        classification: ContextClassification::Internal,
        content,
    })
}

fn render_selected_step(step: BmadArchitectureStep) -> String {
    let mut text = String::from("# Host-selected step\n\nStep ");
    text.push_str(&step.number().to_string());
    text.push_str(" of ");
    text.push_str(&BmadArchitectureStep::count().to_string());
    text.push_str(" — ");
    text.push_str(step.title());
    text.push_str("\n\n## Objective\n\n");
    text.push_str(step.objective());
    text.push_str(
        "\n\n## Boundary\n\nAnswer only this step. Do not answer, restate, or finalize any other \
         step: the host validates progression and rejects out-of-order output. Return this step's \
         content as document sections, each stating what it binds and the divergence it prevents, \
         and list anything you could not settle as an open question rather than inventing an \
         answer. If this dimension does not apply to the supplied problem, say so explicitly in a \
         section instead of leaving it silent.\n",
    );
    text
}

fn render_decision_log(session: &BmadArchitectureSession) -> String {
    let decisions = session.decision_log();
    let mut text = String::from("# Accepted decision log\n\n");
    if decisions.is_empty() {
        text.push_str("No steps have been accepted yet; this is the first step.\n");
    } else {
        for decision in &decisions {
            text.push_str("## ");
            text.push_str(&decision.decision_id);
            text.push_str(" — ");
            text.push_str(&decision.heading);
            text.push_str("\n\nStep: ");
            text.push_str(decision.step.title());
            text.push_str("\n\n");
            text.push_str(&decision.body);
            text.push_str("\n\n");
        }
        text.push_str(
            "These identifiers are stable. Do not renumber, reuse, or contradict them; a decision \
             that would weaken one is a conflict to surface, not a local override.\n\n",
        );
    }
    let questions = session.open_questions();
    text.push_str("# Open questions carried forward\n\n");
    if questions.is_empty() {
        text.push_str("None recorded.\n");
    } else {
        for question in questions {
            text.push_str("- ");
            text.push_str(question);
            text.push('\n');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{is_stepwise_capability, BmadArchitectureCoordinator, ARCHITECTURE_CAPABILITY_ID};
    use crate::bmad_model::capability_coordinator::BmadCapabilityCoordinatorError;
    use desktop_runtime::{
        BmadArchitectureStep, BmadDocumentArtifact, BmadDocumentSection, ContractId, UnixMillis,
    };
    use desktop_store::{KeyProtector, LocalStore, StoreError};

    #[derive(Debug)]
    struct TestProtector;

    impl KeyProtector for TestProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(plaintext.to_vec())
        }

        fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
            Ok(protected.to_vec())
        }
    }

    /// A store for tests that only need `select_step` to have somewhere to
    /// record a session; nothing is resumed from it.
    fn scratch_store() -> (tempfile::TempDir, LocalStore) {
        let directory = tempfile::tempdir().expect("temp dir");
        let store = LocalStore::open(directory.path(), &TestProtector).expect("store");
        (directory, store)
    }

    fn workspace(id: &str) -> ContractId {
        ContractId::new(id).expect("workspace id")
    }

    fn artifact(heading: &str) -> BmadDocumentArtifact {
        BmadDocumentArtifact::new(
            format!("Step: {heading}"),
            vec![BmadDocumentSection {
                heading: heading.to_owned(),
                body: format!("Binds {heading}; prevents divergence."),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .expect("artifact")
    }

    /// Stepwise dispatch reads the sealed `entrypointKind`, not a hard-coded
    /// id. The foundation is the authority on which capabilities run one step
    /// at a time, so a second stepwise capability needs no new branch — and a
    /// capability the foundation does *not* mark `step_jit` can never reach the
    /// progression coordinator.
    #[test]
    fn stepwise_dispatch_follows_the_sealed_entrypoint_kind() {
        let foundation = crate::bmad_foundation::load_bmad_foundation(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation"),
        )
        .expect("sealed foundation");

        assert!(is_stepwise_capability(
            &foundation,
            ARCHITECTURE_CAPABILITY_ID
        ));
        for other in [
            "bmm:bmad-product-brief",
            "bmm:bmad-prd",
            "bmm:bmad-dev-story",
            "bmm:bmad-spec",
            "builder:agent.edit",
            "bmm:not-a-real-skill",
            "malformed-without-a-namespace",
        ] {
            assert!(
                !is_stepwise_capability(&foundation, other),
                "{other} is not sealed as step_jit and must not run stepwise",
            );
        }
    }

    #[test]
    fn the_progression_runs_to_a_distilled_spine() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let workspace = workspace("ws_arch");
        let mut last = None;

        for (offset, expected) in BmadArchitectureStep::ORDERED.into_iter().enumerate() {
            let at = UnixMillis(1_000 + u64::try_from(offset).unwrap_or(0));
            let step = coordinator
                .select_step(&store, &workspace, at)
                .expect("host-selected step");
            assert_eq!(step, expected);
            last = Some(
                coordinator
                    .accept_step(&workspace, step, artifact(step.title()), at)
                    .expect("acceptance"),
            );
        }

        let completion = last.expect("final progression");
        assert!(completion.finalized);
        assert_eq!(completion.step_number, BmadArchitectureStep::count());
        let spine = completion.spine.expect("distilled spine");
        assert_eq!(spine.sections.len(), BmadArchitectureStep::count());
    }

    #[test]
    fn a_step_cannot_be_answered_out_of_order() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let workspace = workspace("ws_arch");
        coordinator
            .select_step(&store, &workspace, UnixMillis(1_000))
            .expect("first step");

        assert_eq!(
            coordinator
                .accept_step(
                    &workspace,
                    BmadArchitectureStep::Deployment,
                    artifact("Deployment"),
                    UnixMillis(1_000),
                )
                .err(),
            Some(BmadCapabilityCoordinatorError::Conflict)
        );
    }

    #[test]
    fn a_result_from_another_workspace_cannot_advance_the_progression() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let bound = workspace("ws_arch");
        let other = workspace("ws_other");
        let step = coordinator
            .select_step(&store, &bound, UnixMillis(1_000))
            .expect("first step");

        assert_eq!(
            coordinator
                .accept_step(&other, step, artifact("System context"), UnixMillis(1_000))
                .err(),
            Some(BmadCapabilityCoordinatorError::Conflict)
        );
    }

    #[test]
    fn switching_workspaces_starts_a_fresh_progression() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let first = workspace("ws_one");
        let step = coordinator
            .select_step(&store, &first, UnixMillis(1_000))
            .expect("first step");
        coordinator
            .accept_step(&first, step, artifact("System context"), UnixMillis(1_000))
            .expect("acceptance");

        let second = workspace("ws_two");
        assert_eq!(
            coordinator
                .select_step(&store, &second, UnixMillis(2_000))
                .expect("fresh progression"),
            BmadArchitectureStep::SystemContext
        );
    }

    #[test]
    fn invalidation_ends_the_progression() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let workspace = workspace("ws_arch");
        let step = coordinator
            .select_step(&store, &workspace, UnixMillis(1_000))
            .expect("first step");
        coordinator.invalidate();

        assert_eq!(
            coordinator
                .accept_step(
                    &workspace,
                    step,
                    artifact("System context"),
                    UnixMillis(1_001)
                )
                .err(),
            Some(BmadCapabilityCoordinatorError::Conflict)
        );
    }

    #[test]
    fn a_finalized_progression_renders_its_run_folder_for_d3() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let bound = workspace("ws_arch");
        for offset in 0..BmadArchitectureStep::count() {
            let at = UnixMillis(1_000 + u64::try_from(offset).unwrap_or(0));
            let selected = coordinator.select_step(&store, &bound, at).expect("step");
            coordinator
                .accept_step(&bound, selected, artifact(selected.title()), at)
                .expect("acceptance");
        }

        let files = coordinator.finalized_run_files(&bound).expect("run folder");

        assert_eq!(files.len(), 2);
        let spine = files
            .iter()
            .find(|(path, _)| path.as_str().ends_with("ARCHITECTURE-SPINE.md"))
            .expect("spine file");
        let memlog = files
            .iter()
            .find(|(path, _)| path.as_str().ends_with(".memlog.md"))
            .expect("memlog file");

        // The spine is a real Markdown document, derived from the log.
        assert!(spine.1.starts_with(
            "# Architecture spine
"
        ));
        assert!(spine.1.contains("## 1. System context"));
        // The memlog carries the chronology the spine was derived from.
        assert!(memlog
            .1
            .contains("- (event) started step 1: System context"));
        assert!(memlog.1.contains("- (event) spine finalized"));
        // Both live in one run folder, as upstream expects.
        assert!(spine.0.as_str().starts_with("docs/architecture/"));
        assert!(memlog.0.as_str().starts_with("docs/architecture/"));
    }

    #[test]
    fn an_unfinished_or_foreign_progression_has_no_run_folder() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let bound = workspace("ws_arch");
        let other = workspace("ws_other");
        let step = coordinator
            .select_step(&store, &bound, UnixMillis(1_000))
            .expect("first step");
        coordinator
            .accept_step(&bound, step, artifact("System context"), UnixMillis(1_000))
            .expect("acceptance");

        // Not finalized yet: nothing to write.
        assert_eq!(
            coordinator.finalized_run_files(&bound).err(),
            Some(BmadCapabilityCoordinatorError::Conflict)
        );
        // And never another workspace's run.
        assert_eq!(
            coordinator.finalized_run_files(&other).err(),
            Some(BmadCapabilityCoordinatorError::Conflict)
        );
    }

    #[test]
    fn the_selected_step_and_accepted_log_are_reviewed_context() {
        let (_scratch, store) = scratch_store();
        let mut coordinator = BmadArchitectureCoordinator::new();
        let workspace = workspace("ws_arch");
        let step = coordinator
            .select_step(&store, &workspace, UnixMillis(1_000))
            .expect("first step");

        let first = coordinator
            .step_context_candidates(step)
            .expect("first candidates");
        assert_eq!(first.len(), 2);
        assert!(first[0].content.contains("Step 1 of 9 — System context"));
        assert!(first[0].content.contains("Answer only this step."));
        assert!(first[1].content.contains("No steps have been accepted yet"));
        assert!(first
            .iter()
            .all(|candidate| candidate.semantic_role == "architecture_progression"));

        coordinator
            .accept_step(
                &workspace,
                step,
                artifact("System context"),
                UnixMillis(1_000),
            )
            .expect("acceptance");
        let next = coordinator
            .select_step(&store, &workspace, UnixMillis(1_001))
            .expect("second step");
        let second = coordinator
            .step_context_candidates(next)
            .expect("second candidates");

        assert!(second[0].content.contains("Step 2 of 9 — Components"));
        assert!(second[1].content.contains("AD-1 — System context"));
        assert!(second[1].content.contains("Do not renumber"));
    }

    /// P14 finding 1: the progression must survive a process restart.
    ///
    /// Upstream's whole claim for a memlog is that "a fresh session can load it
    /// and continue". Before this, `select_step` opened a fresh session whenever
    /// `active` was `None`, so a restart silently reopened at step one and
    /// discarded every step the user had already reviewed and consented to.
    #[test]
    fn a_progression_resumes_at_the_step_a_restart_interrupted() {
        let (_scratch, store) = scratch_store();
        let workspace = workspace("ws_resume");

        // Three steps accepted, each recorded exactly as the command handler
        // records them: the run, then its result, then the session step.
        let mut coordinator = BmadArchitectureCoordinator::new();
        let mut accepted = Vec::new();
        for index in 0..3 {
            let step = coordinator
                .select_step(&store, &workspace, UnixMillis(1_000))
                .expect("step");
            let session_id = coordinator
                .active_session_id(&workspace)
                .expect("active session");
            let artifact = artifact(step.title());
            let run_id = persist_step_run(&store, &workspace, index, &artifact);
            coordinator
                .accept_step(&workspace, step, artifact, UnixMillis(2_000 + index))
                .expect("accepted");
            store
                .record_architecture_step(session_id.as_str(), run_id.as_str(), 2_000 + index)
                .expect("recorded");
            accepted.push(step);
        }
        let session_before = coordinator
            .active_session_id(&workspace)
            .expect("session id");

        // The process restarts: every host coordinator starts empty.
        let mut restarted = BmadArchitectureCoordinator::new();
        let resumed = restarted
            .select_step(&store, &workspace, UnixMillis(9_000))
            .expect("resumed step");

        assert_eq!(
            resumed,
            BmadArchitectureStep::ORDERED[3],
            "the progression must continue at step four, not restart at step one",
        );
        assert_eq!(
            restarted.active_session_id(&workspace).as_ref(),
            Some(&session_before),
            "resuming continues the same session rather than minting a new one",
        );
        // The decisions the earlier steps made are carried forward, which is
        // what the next run reads as its reviewed context.
        let candidates = restarted
            .step_context_candidates(resumed)
            .expect("candidates");
        let log = &candidates[1].content;
        for step in accepted {
            assert!(
                log.contains(step.title()),
                "the resumed decision log must still carry {}",
                step.title(),
            );
        }
    }

    /// A finalized progression is closed: its spine belongs to the workspace it
    /// was reasoned about and is never continued into a later run.
    #[test]
    fn a_finalized_progression_is_never_resumed() {
        let (_scratch, store) = scratch_store();
        let workspace = workspace("ws_finalized");

        let mut coordinator = BmadArchitectureCoordinator::new();
        let mut session_id = None;
        for index in 0..BmadArchitectureStep::count() {
            let index = index as u64;
            let step = coordinator
                .select_step(&store, &workspace, UnixMillis(1_000))
                .expect("step");
            session_id = coordinator.active_session_id(&workspace);
            let artifact = artifact(step.title());
            let run_id = persist_step_run(&store, &workspace, index, &artifact);
            let progress = coordinator
                .accept_step(&workspace, step, artifact, UnixMillis(2_000 + index))
                .expect("accepted");
            let session = session_id.clone().expect("session id");
            store
                .record_architecture_step(session.as_str(), run_id.as_str(), 2_000 + index)
                .expect("recorded");
            if progress.finalized {
                store
                    .finalize_architecture_session(session.as_str())
                    .expect("finalized");
            }
        }
        assert!(session_id.is_some());

        let mut restarted = BmadArchitectureCoordinator::new();
        let step = restarted
            .select_step(&store, &workspace, UnixMillis(9_000))
            .expect("fresh step");
        assert_eq!(
            step,
            BmadArchitectureStep::ORDERED[0],
            "a finalized spine is not continued; the next run starts a new one",
        );
        assert_ne!(
            restarted.active_session_id(&workspace),
            session_id,
            "a new progression gets its own session identity",
        );
    }

    /// Resuming re-reads each step from its own capability run. A run whose
    /// stored result the current build can no longer verify must fail the
    /// resume rather than quietly continue from a partial spine.
    #[test]
    fn a_resume_fails_closed_when_a_recorded_step_cannot_be_re_verified() {
        let (_scratch, store) = scratch_store();
        let workspace = workspace("ws_tampered");

        let mut coordinator = BmadArchitectureCoordinator::new();
        let step = coordinator
            .select_step(&store, &workspace, UnixMillis(1_000))
            .expect("step");
        let session_id = coordinator
            .active_session_id(&workspace)
            .expect("session id");
        let artifact = artifact(step.title());
        // A run row with no stored result: the step was recorded, but the
        // evidence a resume must re-read is not there.
        let run_id = open_step_run(&store, &workspace, 0);
        coordinator
            .accept_step(&workspace, step, artifact, UnixMillis(2_000))
            .expect("accepted");
        store
            .record_architecture_step(session_id.as_str(), run_id.as_str(), 2_000)
            .expect("recorded");

        let mut restarted = BmadArchitectureCoordinator::new();
        assert!(
            restarted
                .select_step(&store, &workspace, UnixMillis(9_000))
                .is_err(),
            "a step that cannot be re-verified must fail the resume",
        );
    }

    /// Persists one accepted step the way the capability lane does: a run row
    /// plus its verified result, which is what a resume re-reads.
    fn persist_step_run(
        store: &LocalStore,
        workspace: &ContractId,
        index: u64,
        artifact: &BmadDocumentArtifact,
    ) -> ContractId {
        use desktop_runtime::BmadCapabilityOutput;
        let run_id = open_step_run(store, workspace, index);
        store
            .record_bmad_capability_result(
                &run_id,
                &BmadCapabilityOutput::DocumentArtifact(artifact.clone()),
            )
            .expect("result");
        run_id
    }

    /// The run row alone, without a result.
    fn open_step_run(store: &LocalStore, workspace: &ContractId, index: u64) -> ContractId {
        use desktop_runtime::{BmadCapabilityRun, BmadCapabilityRunParams};
        let run_id =
            ContractId::new(format!("caprun_01J0000000000000000000{index:04}")).expect("run id");
        let run = BmadCapabilityRun::open(BmadCapabilityRunParams {
            run_id: run_id.clone(),
            capability_id: desktop_runtime::BmadClosureCapabilityId::new(
                ARCHITECTURE_CAPABILITY_ID,
            )
            .expect("capability id"),
            workspace_id: workspace.clone(),
            instruction_hash: desktop_runtime::sha256_bytes(b"instruction"),
            context_manifest_hash: desktop_runtime::sha256_bytes(b"manifest"),
            output_schema_id: desktop_runtime::BMAD_DOCUMENT_ARTIFACT_SCHEMA.to_owned(),
            // Single-use per run: each step consumed its own reviewed decision.
            consent_evidence_id: ContractId::new(format!(
                "decision_01J000000000000000000{index:05}"
            ))
            .expect("decision id"),
            created_at: UnixMillis(2_000 + index),
        })
        .expect("run");
        store.create_bmad_capability_run(&run).expect("created");
        run_id
    }
}
