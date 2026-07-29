//! Per-skill [`BmadMethodSkill`](super::BmadMethodSkill) adapters for the
//! `1-analysis` Method skills covered by MTH-01..MTH-05.
//!
//! Each submodule holds exactly one zero-sized adapter type and its
//! `descriptor()` implementation, built from that skill's pinned `SKILL.md`
//! per `bmad_method_analysis_skills.rs`. Kept as five separate files instead
//! of one so concurrent MTH tasks touching other skills never conflict on
//! this file's diff.

mod bmad_agent_analyst;
mod bmad_agent_tech_writer;
mod bmad_document_project;
mod bmad_prfaq;
mod bmad_product_brief;

pub use bmad_agent_analyst::BmadAgentAnalystSkill;
pub use bmad_agent_tech_writer::BmadAgentTechWriterSkill;
pub use bmad_document_project::BmadDocumentProjectSkill;
pub use bmad_prfaq::BmadPrfaqSkill;
pub use bmad_product_brief::BmadProductBriefSkill;
