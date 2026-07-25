//! The BMAD method as a machine-readable graph.
//!
//! Upstream ships `module-help.csv`: one row per skill (or skill action)
//! carrying its phase, its ordering edges, whether it is a required gate, and
//! where its output artifact lands. Upstream's `bmad-help` skill hands that
//! file to a model and asks it to infer where the user is.
//!
//! Forge does not need to ask. It has the workspace and it has code, so it
//! computes the answer: which steps are complete, which phase that puts you
//! in, what the next required gate is, and what is available right now. No
//! model call, no receipt, nothing to hallucinate. This is the one place where
//! being a native runtime beats the original rather than merely matching it.
//!
//! The graph is sealed foundation data (ADR-0008 adopted source). Nothing here
//! executes anything: a step is a description of work the user may choose to
//! start, never an instruction the host acts on.

use std::collections::{BTreeMap, BTreeSet};

use crate::bmad::error::{MethodError, MethodErrorCode};

/// Upstream phase ordering. `Anytime` steps are always offerable and never
/// gate progression.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BmadWorkflowPhase {
    Anytime,
    Analysis,
    Planning,
    Solutioning,
    Implementation,
}

impl BmadWorkflowPhase {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Anytime => "anytime",
            Self::Analysis => "1-analysis",
            Self::Planning => "2-planning",
            Self::Solutioning => "3-solutioning",
            Self::Implementation => "4-implementation",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Anytime => "Anytime",
            Self::Analysis => "Analysis",
            Self::Planning => "Planning",
            Self::Solutioning => "Solutioning",
            Self::Implementation => "Implementation",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "anytime" | "" => Some(Self::Anytime),
            "1-analysis" => Some(Self::Analysis),
            "2-planning" => Some(Self::Planning),
            "3-solutioning" => Some(Self::Solutioning),
            "4-implementation" => Some(Self::Implementation),
            _ => None,
        }
    }

    /// The ordered phases a run progresses through. `Anytime` is excluded: it
    /// is not a position in the method.
    pub const ORDERED: [Self; 4] = [
        Self::Analysis,
        Self::Planning,
        Self::Solutioning,
        Self::Implementation,
    ];
}

/// One row of the method graph.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadWorkflowStep {
    /// `skill` for a single-action skill, `skill:action` otherwise. Unique.
    pub key: String,
    pub module: String,
    pub skill: String,
    pub display_name: String,
    pub menu_code: String,
    pub description: String,
    pub action: Option<String>,
    pub args: Option<String>,
    pub phase: BmadWorkflowPhase,
    pub preceded_by: Vec<String>,
    pub followed_by: Vec<String>,
    /// A hard gate: later phases are not meaningfully reachable without it.
    pub required: bool,
    /// Config-variable names naming where this step's artifact lands.
    pub output_location: Vec<String>,
    /// Human description of the artifact, used for completion matching.
    pub outputs: Option<String>,
}

/// The computed position of one project in the method.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BmadWorkflowState {
    /// The earliest phase that still has an unmet required gate, or
    /// `Implementation` once every gate is met.
    pub phase: BmadWorkflowPhase,
    pub completed: Vec<String>,
    /// The next required gate, in phase then row order. `None` when every
    /// required step is complete.
    pub next_required: Option<String>,
    /// Steps whose ordering predecessors are all satisfied and which are not
    /// themselves complete.
    pub available: Vec<String>,
    /// Required steps still outstanding, in order.
    pub outstanding_required: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmadWorkflowGraph {
    steps: Vec<BmadWorkflowStep>,
}

impl BmadWorkflowGraph {
    /// Parses the sealed `module-help.csv` graph.
    ///
    /// # Errors
    ///
    /// Returns [`MethodErrorCode::MethodResultInvalid`] when the header does
    /// not match the reviewed column set, a row has the wrong arity, a phase
    /// is unknown, or two rows claim the same key.
    pub fn parse(csv: &str) -> Result<Self, MethodError> {
        let mut rows = parse_csv(csv).into_iter();
        let header = rows
            .next()
            .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
        if header
            != [
                "module",
                "skill",
                "display-name",
                "menu-code",
                "description",
                "action",
                "args",
                "phase",
                "preceded-by",
                "followed-by",
                "required",
                "output-location",
                "outputs",
            ]
        {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }

        let mut steps = Vec::new();
        let mut keys = BTreeSet::new();
        for row in rows {
            if row.iter().all(String::is_empty) {
                continue;
            }
            if row.len() != header.len() {
                return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
            }
            let skill = row[1].clone();
            // `_meta` rows carry module documentation links, not steps.
            if skill == "_meta" {
                continue;
            }
            let action = optional(&row[5]);
            let key = match &action {
                Some(action) => format!("{skill}:{action}"),
                None => skill.clone(),
            };
            if !keys.insert(key.clone()) {
                return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
            }
            let phase = BmadWorkflowPhase::parse(&row[7])
                .ok_or_else(|| MethodError::new(MethodErrorCode::MethodResultInvalid))?;
            steps.push(BmadWorkflowStep {
                key,
                module: row[0].clone(),
                skill,
                display_name: row[2].clone(),
                menu_code: row[3].clone(),
                description: row[4].clone(),
                action,
                args: optional(&row[6]),
                phase,
                preceded_by: split_list(&row[8]),
                followed_by: split_list(&row[9]),
                required: row[10] == "true",
                output_location: split_pipe(&row[11]),
                outputs: optional(&row[12]),
            });
        }
        if steps.is_empty() {
            return Err(MethodError::new(MethodErrorCode::MethodResultInvalid));
        }
        Ok(Self { steps })
    }

    #[must_use]
    pub fn steps(&self) -> &[BmadWorkflowStep] {
        &self.steps
    }

    #[must_use]
    pub fn step(&self, key: &str) -> Option<&BmadWorkflowStep> {
        self.steps.iter().find(|step| step.key == key)
    }

    /// Computes where a project stands from the set of completed step keys.
    ///
    /// Ordering edges are *soft* upstream ("these are soft suggestions, not
    /// hard gates — see `required` for gating"), so an unmet predecessor makes
    /// a step un-*recommended*, never forbidden. Only `required` steps gate a
    /// phase.
    #[must_use]
    pub fn state(&self, completed: &BTreeSet<String>) -> BmadWorkflowState {
        let known = completed
            .iter()
            .filter(|key| self.step(key).is_some())
            .cloned()
            .collect::<Vec<_>>();
        let done = known.iter().cloned().collect::<BTreeSet<_>>();

        let outstanding_required = self
            .steps
            .iter()
            .filter(|step| step.required && !done.contains(&step.key))
            .map(|step| step.key.clone())
            .collect::<Vec<_>>();

        // The current phase is the earliest ordered phase not yet satisfied.
        //
        // Gates alone are not enough to derive this. `1-analysis` carries no
        // `required` row at all, so keying purely off unmet gates made Analysis
        // unreachable as a reported phase for any project, ever — a brand-new
        // empty workspace reported `Planning`. Upstream is explicit that "a
        // phase with no required items is entirely optional — recommend it but
        // be clear about what's actually required next", which means such a
        // phase is *current* until the project engages with it or moves past
        // it, not skipped outright.
        let phase = BmadWorkflowPhase::ORDERED
            .into_iter()
            .find(|phase| !self.phase_is_satisfied(*phase, &done))
            .unwrap_or(BmadWorkflowPhase::Implementation);

        let next_required = self
            .steps
            .iter()
            .filter(|step| step.required && !done.contains(&step.key))
            .min_by_key(|step| (step.phase, self.order_of(&step.key)))
            .map(|step| step.key.clone());

        let available = self
            .steps
            .iter()
            .filter(|step| {
                !done.contains(&step.key)
                    && step
                        .preceded_by
                        .iter()
                        .all(|predecessor| done.contains(predecessor))
            })
            .map(|step| step.key.clone())
            .collect::<Vec<_>>();

        BmadWorkflowState {
            phase,
            completed: known,
            next_required,
            available,
            outstanding_required,
        }
    }

    /// Whether a project has moved past `phase`.
    ///
    /// A phase holding `required` steps is satisfied only when every one of
    /// them is complete. A wholly optional phase is satisfied once the project
    /// has engaged with it — completed any of its steps — or has demonstrably
    /// moved on by completing work in a later phase.
    fn phase_is_satisfied(&self, phase: BmadWorkflowPhase, done: &BTreeSet<String>) -> bool {
        let mut has_required = false;
        let mut any_done_here = false;
        for step in &self.steps {
            if step.phase != phase {
                continue;
            }
            if step.required {
                has_required = true;
                if !done.contains(&step.key) {
                    return false;
                }
            }
            if done.contains(&step.key) {
                any_done_here = true;
            }
        }
        if has_required {
            return true;
        }
        any_done_here
            || self
                .steps
                .iter()
                .any(|step| step.phase > phase && done.contains(&step.key))
    }

    fn order_of(&self, key: &str) -> usize {
        self.steps
            .iter()
            .position(|step| step.key == key)
            .unwrap_or(usize::MAX)
    }

    /// Groups steps by the config variable naming their output location, so a
    /// host can resolve each location once and scan it for many steps.
    #[must_use]
    pub fn steps_by_output_location(&self) -> BTreeMap<String, Vec<&BmadWorkflowStep>> {
        let mut grouped: BTreeMap<String, Vec<&BmadWorkflowStep>> = BTreeMap::new();
        for step in &self.steps {
            for location in &step.output_location {
                grouped.entry(location.clone()).or_default().push(step);
            }
        }
        grouped
    }
}

fn optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(&[',', '|'][..])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn split_pipe(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Minimal RFC 4180 reader: the graph's descriptions contain commas inside
/// quoted fields, so a naive split would corrupt the row.
fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted => {
                if characters.peek() == Some(&'"') {
                    field.push('"');
                    characters.next();
                } else {
                    quoted = false;
                }
            }
            '"' => quoted = true,
            ',' if !quoted => row.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeSet;

    use super::{BmadWorkflowGraph, BmadWorkflowPhase};

    const GRAPH: &str = include_str!(
        "../../../../packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/module-help.csv"
    );

    fn graph() -> BmadWorkflowGraph {
        BmadWorkflowGraph::parse(GRAPH).expect("sealed method graph")
    }

    fn completed(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    #[test]
    fn the_sealed_graph_parses_into_the_upstream_method() {
        let graph = graph();

        // Quoted descriptions containing commas must survive intact.
        let prd = graph.step("bmad-prd").expect("PRD step");
        assert_eq!(prd.menu_code, "PRD");
        assert_eq!(prd.phase, BmadWorkflowPhase::Planning);
        assert!(prd.required);
        assert!(prd
            .description
            .contains("create a new PRD via coached discovery"));
        assert_eq!(prd.preceded_by, ["bmad-product-brief"]);

        // Multi-action skills are distinct steps.
        assert!(graph.step("bmad-create-story:create").is_some());
        assert!(graph.step("bmad-create-story:validate").is_some());
        assert!(graph.step("bmad-create-story").is_none());

        // `_meta` documentation rows are not steps.
        assert!(graph.steps().iter().all(|step| step.skill != "_meta"));
    }

    #[test]
    fn the_required_gates_are_the_upstream_gates() {
        let graph = graph();
        let required = graph
            .steps()
            .iter()
            .filter(|step| step.required)
            .map(|step| step.key.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            required,
            [
                "bmad-prd",
                "bmad-architecture",
                "bmad-create-epics-and-stories",
                "bmad-check-implementation-readiness",
                "bmad-sprint-planning",
                "bmad-create-story:create",
                "bmad-dev-story",
            ]
        );
    }

    #[test]
    fn an_empty_project_starts_at_analysis_while_the_first_gate_is_the_prd() {
        let graph = graph();
        let state = graph.state(&BTreeSet::new());

        // Analysis carries no `required` row upstream, but it is still where an
        // untouched project stands. Reporting the earliest *gate's* phase here
        // skipped Analysis entirely and claimed a brand-new workspace was
        // already in Planning.
        assert_eq!(state.phase, BmadWorkflowPhase::Analysis);
        // The phase and the next gate are deliberately different facts: nothing
        // in Analysis is required, so the first thing that gates progress is
        // still the PRD.
        assert_eq!(state.next_required.as_deref(), Some("bmad-prd"));
        assert!(state.completed.is_empty());
        // Anytime and analysis steps have no predecessors, so they are offerable.
        assert!(state.available.contains(&"bmad-brainstorming".to_owned()));
        assert!(state.available.contains(&"bmad-quick-dev".to_owned()));
    }

    #[test]
    fn an_optional_phase_is_satisfied_by_engaging_with_it_or_moving_past_it() {
        let graph = graph();

        // Engaging with Analysis moves the project on, even though nothing in
        // Analysis is required.
        let engaged = graph.state(&completed(&["bmad-product-brief"]));
        assert_eq!(engaged.phase, BmadWorkflowPhase::Planning);

        // Skipping Analysis entirely is legitimate: completing later work also
        // satisfies a wholly optional phase rather than pinning the project to
        // it forever.
        let skipped = graph.state(&completed(&["bmad-prd"]));
        assert_eq!(skipped.phase, BmadWorkflowPhase::Solutioning);
    }

    #[test]
    fn completing_gates_advances_the_phase() {
        let graph = graph();

        let planned = graph.state(&completed(&["bmad-prd"]));
        assert_eq!(planned.phase, BmadWorkflowPhase::Solutioning);
        assert_eq!(planned.next_required.as_deref(), Some("bmad-architecture"));

        let solutioned = graph.state(&completed(&[
            "bmad-prd",
            "bmad-architecture",
            "bmad-create-epics-and-stories",
            "bmad-check-implementation-readiness",
        ]));
        assert_eq!(solutioned.phase, BmadWorkflowPhase::Implementation);
        assert_eq!(
            solutioned.next_required.as_deref(),
            Some("bmad-sprint-planning")
        );
    }

    #[test]
    fn every_gate_met_leaves_no_next_required() {
        let graph = graph();
        let all_gates = graph
            .steps()
            .iter()
            .filter(|step| step.required)
            .map(|step| step.key.clone())
            .collect::<BTreeSet<_>>();

        let state = graph.state(&all_gates);

        assert_eq!(state.next_required, None);
        assert!(state.outstanding_required.is_empty());
        assert_eq!(state.phase, BmadWorkflowPhase::Implementation);
    }

    #[test]
    fn ordering_edges_gate_recommendation_not_permission() {
        let graph = graph();
        let state = graph.state(&BTreeSet::new());

        // UX follows the PRD, so it is not yet recommended...
        assert!(!state.available.contains(&"bmad-ux".to_owned()));
        // ...but it is still a real step the user may choose; nothing forbids it.
        assert!(graph.step("bmad-ux").is_some());

        let after_prd = graph.state(&completed(&["bmad-prd"]));
        assert!(after_prd.available.contains(&"bmad-ux".to_owned()));
    }

    #[test]
    fn unknown_completion_keys_are_ignored_rather_than_trusted() {
        let graph = graph();
        let state = graph.state(&completed(&["bmad-prd", "not-a-real-skill"]));

        assert_eq!(state.completed, ["bmad-prd"]);
        assert_eq!(state.next_required.as_deref(), Some("bmad-architecture"));
    }

    #[test]
    fn output_locations_group_for_one_scan_per_location() {
        let graph = graph();
        let grouped = graph.steps_by_output_location();

        assert!(grouped.contains_key("planning_artifacts"));
        assert!(grouped["planning_artifacts"]
            .iter()
            .any(|step| step.key == "bmad-architecture"));
    }

    #[test]
    fn a_malformed_graph_fails_closed() {
        assert!(BmadWorkflowGraph::parse("").is_err());
        assert!(BmadWorkflowGraph::parse("wrong,header\n1,2\n").is_err());
        // Correct header, unknown phase.
        let header = "module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs\n";
        assert!(
            BmadWorkflowGraph::parse(&format!("{header}M,s,D,C,desc,,,9-unknown,,,false,,"))
                .is_err()
        );
        // Duplicate keys.
        assert!(BmadWorkflowGraph::parse(&format!(
            "{header}M,s,D,C,desc,,,anytime,,,false,,\nM,s,D,C,desc,,,anytime,,,false,,"
        ))
        .is_err());
        // Header only.
        assert!(BmadWorkflowGraph::parse(header).is_err());
    }
}
