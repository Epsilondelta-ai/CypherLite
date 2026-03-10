// Rule-based optimization: predicate pushdown, label-filter merge
//
// For Phase 2, this is a pass-through. Optimization rules will be added
// incrementally in future phases.

use super::LogicalPlan;

/// Apply optimization rules to a logical plan.
///
/// Currently a no-op pass-through that returns the plan unchanged.
/// Future optimizations:
/// - Predicate pushdown: move Filter below Project
/// - Label filter merge: merge Filter(label = X) into NodeScan
pub fn optimize(plan: LogicalPlan) -> LogicalPlan {
    plan
}
