//! Per-skill [`BmadMethodSkill`](super::BmadMethodSkill) adapters for the
//! `1-analysis` Method skills covered by MTH-01..MTH-05.
//!
//! Each submodule holds exactly one zero-sized adapter type and its
//! `descriptor()` implementation, built from that skill's pinned `SKILL.md`
//! per `bmad_method_analysis_skills.rs`. Kept as five separate files instead
//! of one so concurrent MTH tasks touching other skills never conflict on
//! this file's diff.

mod bmad_advanced_elicitation;
mod bmad_agent_analyst;
mod bmad_agent_architect;
mod bmad_agent_pm;
mod bmad_agent_tech_writer;
mod bmad_agent_ux_designer;
mod bmad_architecture;
mod bmad_brainstorming;
mod bmad_check_implementation_readiness;
mod bmad_create_epics_and_stories;
mod bmad_customize;
mod bmad_deep_recon;
mod bmad_document_project;
mod bmad_forge_idea;
mod bmad_generate_project_context;
mod bmad_help;
mod bmad_party_mode;
mod bmad_prd;
mod bmad_prfaq;
mod bmad_product_brief;
mod bmad_review;
mod bmad_spec;
mod bmad_ux;

pub use bmad_advanced_elicitation::BmadAdvancedElicitationSkill;
pub use bmad_agent_analyst::BmadAgentAnalystSkill;
pub use bmad_agent_architect::BmadAgentArchitectSkill;
pub use bmad_agent_pm::BmadAgentPmSkill;
pub use bmad_agent_tech_writer::BmadAgentTechWriterSkill;
pub use bmad_agent_ux_designer::BmadAgentUxDesignerSkill;
pub use bmad_architecture::BmadArchitectureSkill;
pub use bmad_brainstorming::BmadBrainstormingSkill;
pub use bmad_check_implementation_readiness::BmadCheckImplementationReadinessSkill;
pub use bmad_create_epics_and_stories::BmadCreateEpicsAndStoriesSkill;
pub use bmad_customize::BmadCustomizeSkill;
pub use bmad_deep_recon::BmadDeepReconSkill;
pub use bmad_document_project::BmadDocumentProjectSkill;
pub use bmad_forge_idea::BmadForgeIdeaSkill;
pub use bmad_generate_project_context::BmadGenerateProjectContextSkill;
pub use bmad_help::BmadHelpSkill;
pub use bmad_party_mode::BmadPartyModeSkill;
pub use bmad_prd::BmadPrdSkill;
pub use bmad_prfaq::BmadPrfaqSkill;
pub use bmad_product_brief::BmadProductBriefSkill;
pub use bmad_review::BmadReviewSkill;
pub use bmad_spec::BmadSpecSkill;
pub use bmad_ux::BmadUxSkill;

static BMAD_PRD_RUNTIME_SKILL: BmadPrdSkill = BmadPrdSkill;

/// Resolves only independently accepted Method capabilities to executable
/// runtime adapters. Preserved or implementation-pending adapters remain
/// protocol-known but cannot enter the host execution path through this map.
#[must_use]
pub fn canonical_method_skill_adapter(
    canonical_id: &str,
) -> Option<&'static dyn super::BmadMethodSkill> {
    match canonical_id {
        "bmm:bmad-prd" => Some(&BMAD_PRD_RUNTIME_SKILL),
        _ => None,
    }
}
