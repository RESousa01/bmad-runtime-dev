//! Script execution isolation: per-script profiles, approval binding, and
//! the sandbox that enforces or explicitly declines to enforce them.
//!
//! Consumes an Airlock-approved `BmadScriptPlan`
//! (`packages/contracts/schemas/bmad-script-plan.schema.json`); produces a
//! bounded [`sandbox::ScriptResult`] plus cleanup/evidence records. Authority
//! here comes only from generated contracts, the declared [`script_profiles::ScriptProfile`],
//! and runtime ports — never from script or SKILL.md prose.

pub mod sandbox;
pub mod script_profiles;
