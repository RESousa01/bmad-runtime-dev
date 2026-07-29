//! The shared Method-skill adapter surface (MTH scaffolding).
//!
//! Kept thin on purpose: everything a per-skill adapter needs lives in
//! [`harness`], and this file exists only so `bmad::skills::BmadMethodSkill`
//! is one path a downstream MTH task can `use` without reaching into a
//! private submodule.

mod adapters;
mod harness;

pub use adapters::{
    BmadAdvancedElicitationSkill, BmadAgentAnalystSkill, BmadAgentArchitectSkill, BmadAgentPmSkill,
    BmadAgentTechWriterSkill, BmadAgentUxDesignerSkill, BmadArchitectureSkill,
    BmadBrainstormingSkill, BmadCheckImplementationReadinessSkill, BmadCreateEpicsAndStoriesSkill,
    BmadCustomizeSkill, BmadDeepReconSkill, BmadDocumentProjectSkill, BmadForgeIdeaSkill,
    BmadGenerateProjectContextSkill, BmadHelpSkill, BmadPartyModeSkill, BmadPrdSkill,
    BmadPrfaqSkill, BmadProductBriefSkill, BmadReviewSkill, BmadSpecSkill, BmadUxSkill,
};
pub use harness::{
    BmadMethodSkill, BmadSkillDescriptor, BmadSkillDescriptorInput, BmadSkillHarnessError,
    BmadSkillSupportState, BMAD_SKILL_HARNESS_SCHEMA,
};
