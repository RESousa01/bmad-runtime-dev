//! Typed action plans (ENG-08).

/// Placeholder wiring; the plan builder lands with ENG-08.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadActionPlanError {
    /// A plan exceeded its declared budget.
    BudgetExceeded,
}
