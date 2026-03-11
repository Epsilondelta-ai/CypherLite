// Rule-based optimization: index scan selection, limit pushdown, constant folding,
// projection pruning.

use super::LogicalPlan;
use crate::parser::ast::*;

/// Apply optimization rules to a logical plan.
///
/// Rules are applied bottom-up (children optimized first):
/// 1. Index scan selection: Filter(NodeScan) -> IndexScan when applicable
/// 2. Limit pushdown: Limit(NodeScan) -> NodeScan with limit annotation
/// 3. Constant folding: simplify constant expressions
/// 4. Projection pruning: merge consecutive Projects
pub fn optimize(plan: LogicalPlan) -> LogicalPlan {
    apply_rules(plan)
}

/// Recursively apply optimization rules bottom-up.
fn apply_rules(plan: LogicalPlan) -> LogicalPlan {
    // First, recursively optimize children
    let plan = optimize_children(plan);
    // Then apply rules at the current node
    let plan = push_index_scan(plan);
    let plan = push_limit_down(plan);
    let plan = fold_constants_in_plan(plan);
    prune_projections(plan)
}

/// Recursively optimize child plans.
fn optimize_children(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter { source, predicate } => LogicalPlan::Filter {
            source: Box::new(apply_rules(*source)),
            predicate,
        },
        LogicalPlan::Project {
            source,
            items,
            distinct,
        } => LogicalPlan::Project {
            source: Box::new(apply_rules(*source)),
            items,
            distinct,
        },
        LogicalPlan::Sort { source, items } => LogicalPlan::Sort {
            source: Box::new(apply_rules(*source)),
            items,
        },
        LogicalPlan::Skip { source, count } => LogicalPlan::Skip {
            source: Box::new(apply_rules(*source)),
            count,
        },
        LogicalPlan::Limit { source, count } => LogicalPlan::Limit {
            source: Box::new(apply_rules(*source)),
            count,
        },
        LogicalPlan::Aggregate {
            source,
            group_keys,
            aggregates,
        } => LogicalPlan::Aggregate {
            source: Box::new(apply_rules(*source)),
            group_keys,
            aggregates,
        },
        LogicalPlan::Expand {
            source,
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        } => LogicalPlan::Expand {
            source: Box::new(apply_rules(*source)),
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        },
        LogicalPlan::CreateOp { source, pattern } => LogicalPlan::CreateOp {
            source: source.map(|s| Box::new(apply_rules(*s))),
            pattern,
        },
        LogicalPlan::DeleteOp {
            source,
            exprs,
            detach,
        } => LogicalPlan::DeleteOp {
            source: Box::new(apply_rules(*source)),
            exprs,
            detach,
        },
        LogicalPlan::SetOp { source, items } => LogicalPlan::SetOp {
            source: Box::new(apply_rules(*source)),
            items,
        },
        LogicalPlan::RemoveOp { source, items } => LogicalPlan::RemoveOp {
            source: Box::new(apply_rules(*source)),
            items,
        },
        LogicalPlan::With {
            source,
            items,
            where_clause,
            distinct,
        } => LogicalPlan::With {
            source: Box::new(apply_rules(*source)),
            items,
            where_clause,
            distinct,
        },
        LogicalPlan::Unwind {
            source,
            expr,
            variable,
        } => LogicalPlan::Unwind {
            source: Box::new(apply_rules(*source)),
            expr,
            variable,
        },
        LogicalPlan::MergeOp {
            source,
            pattern,
            on_match,
            on_create,
        } => LogicalPlan::MergeOp {
            source: source.map(|s| Box::new(apply_rules(*s))),
            pattern,
            on_match,
            on_create,
        },
        LogicalPlan::VarLengthExpand {
            source,
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
            min_hops,
            max_hops,
        } => LogicalPlan::VarLengthExpand {
            source: Box::new(apply_rules(*source)),
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
            min_hops,
            max_hops,
        },
        LogicalPlan::OptionalExpand {
            source,
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        } => LogicalPlan::OptionalExpand {
            source: Box::new(apply_rules(*source)),
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        },
        LogicalPlan::AsOfScan {
            source,
            timestamp_expr,
        } => LogicalPlan::AsOfScan {
            source: Box::new(apply_rules(*source)),
            timestamp_expr,
        },
        LogicalPlan::TemporalRangeScan {
            source,
            start_expr,
            end_expr,
        } => LogicalPlan::TemporalRangeScan {
            source: Box::new(apply_rules(*source)),
            start_expr,
            end_expr,
        },
        // Leaf nodes: no children to optimize
        plan @ LogicalPlan::NodeScan { .. }
        | plan @ LogicalPlan::IndexScan { .. }
        | plan @ LogicalPlan::EmptySource
        | plan @ LogicalPlan::CreateIndex { .. }
        | plan @ LogicalPlan::DropIndex { .. } => plan,
    }
}

// ============================================================================
// TASK-112: Index scan selection
// ============================================================================

/// Transform Filter(NodeScan { variable, label_id: Some(lid) }, predicate)
/// into IndexScan when the predicate matches `variable.prop == literal`.
fn push_index_scan(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter {
            source,
            predicate,
        } => {
            if let LogicalPlan::NodeScan {
                ref variable,
                label_id: Some(lid),
                ..
            } = *source
            {
                match try_extract_index_predicate(&predicate, variable) {
                    Some((prop_key, lookup_value, remaining)) => {
                        let index_scan = LogicalPlan::IndexScan {
                            variable: variable.clone(),
                            label_id: lid,
                            prop_key,
                            lookup_value,
                        };
                        match remaining {
                            Some(rest) => LogicalPlan::Filter {
                                source: Box::new(index_scan),
                                predicate: rest,
                            },
                            None => index_scan,
                        }
                    }
                    None => LogicalPlan::Filter {
                        source,
                        predicate,
                    },
                }
            } else {
                LogicalPlan::Filter {
                    source,
                    predicate,
                }
            }
        }
        other => other,
    }
}

/// Try to extract an index-suitable predicate from an expression.
///
/// Returns (prop_key, lookup_value, remaining_predicate) if found.
/// The remaining_predicate is Some if there were additional AND conditions.
fn try_extract_index_predicate(
    expr: &Expression,
    variable: &str,
) -> Option<(String, Expression, Option<Expression>)> {
    // Check for direct equality: variable.prop == literal or literal == variable.prop
    if let Some((prop, val)) = try_extract_eq_predicate(expr, variable) {
        return Some((prop, val, None));
    }

    // Check for AND expressions: try to extract one index predicate from conjuncts
    if let Expression::BinaryOp(BinaryOp::And, left, right) = expr {
        // Try left side
        if let Some((prop, val)) = try_extract_eq_predicate(left, variable) {
            return Some((prop, val, Some(*right.clone())));
        }
        // Try right side
        if let Some((prop, val)) = try_extract_eq_predicate(right, variable) {
            return Some((prop, val, Some(*left.clone())));
        }
    }

    None
}

/// Try to match `variable.property == literal_value` or `literal_value == variable.property`.
fn try_extract_eq_predicate(
    expr: &Expression,
    variable: &str,
) -> Option<(String, Expression)> {
    if let Expression::BinaryOp(BinaryOp::Eq, left, right) = expr {
        // Case 1: variable.property == literal
        if let Some((prop, val)) = match_property_eq_literal(left, right, variable) {
            return Some((prop, val));
        }
        // Case 2: literal == variable.property (reversed)
        if let Some((prop, val)) = match_property_eq_literal(right, left, variable) {
            return Some((prop, val));
        }
    }
    None
}

/// Check if prop_side is `variable.property` and val_side is a literal.
fn match_property_eq_literal(
    prop_side: &Expression,
    val_side: &Expression,
    variable: &str,
) -> Option<(String, Expression)> {
    if let Expression::Property(var_expr, prop_name) = prop_side {
        if let Expression::Variable(var_name) = var_expr.as_ref() {
            if var_name == variable && is_literal(val_side) {
                return Some((prop_name.clone(), val_side.clone()));
            }
        }
    }
    None
}

/// Check if an expression is a literal value (suitable for index lookup).
fn is_literal(expr: &Expression) -> bool {
    matches!(expr, Expression::Literal(_))
}

// ============================================================================
// TASK-114: LIMIT pushdown
// ============================================================================

/// Push LIMIT into NodeScan when the source is a simple NodeScan (no Sort in between).
/// Pattern: Limit(NodeScan { variable, label_id, limit: None }, count)
///       -> NodeScan { variable, label_id, limit: Some(count) }
///
/// Does NOT push limit through Sort (sort needs all data).
fn push_limit_down(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Limit { source, count } => {
            if let Some(limit_val) = try_eval_limit_count(&count) {
                match *source {
                    LogicalPlan::NodeScan {
                        variable,
                        label_id,
                        limit: existing_limit,
                    } => {
                        // Merge limits: take the smaller one
                        let new_limit = match existing_limit {
                            Some(existing) => Some(existing.min(limit_val)),
                            None => Some(limit_val),
                        };
                        return LogicalPlan::NodeScan {
                            variable,
                            label_id,
                            limit: new_limit,
                        };
                    }
                    // Don't push through Sort (needs all data for sorting)
                    other => {
                        return LogicalPlan::Limit {
                            source: Box::new(other),
                            count,
                        };
                    }
                }
            }
            LogicalPlan::Limit { source, count }
        }
        other => other,
    }
}

/// Try to evaluate a LIMIT count expression at optimization time.
/// Only works for literal integers.
fn try_eval_limit_count(expr: &Expression) -> Option<usize> {
    if let Expression::Literal(Literal::Integer(n)) = expr {
        if *n >= 0 {
            return Some(*n as usize);
        }
    }
    None
}

// ============================================================================
// TASK-115: Constant folding
// ============================================================================

/// Fold constant expressions in a plan node's expressions.
fn fold_constants_in_plan(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter { source, predicate } => {
            let folded = fold_expr(predicate);
            // If predicate folds to true, eliminate the filter entirely
            if folded == Expression::Literal(Literal::Bool(true)) {
                return *source;
            }
            LogicalPlan::Filter {
                source,
                predicate: folded,
            }
        }
        LogicalPlan::Project {
            source,
            items,
            distinct,
        } => {
            let items = items
                .into_iter()
                .map(|item| ReturnItem {
                    expr: fold_expr(item.expr),
                    alias: item.alias,
                })
                .collect();
            LogicalPlan::Project {
                source,
                items,
                distinct,
            }
        }
        LogicalPlan::Sort { source, items } => {
            let items = items
                .into_iter()
                .map(|item| OrderItem {
                    expr: fold_expr(item.expr),
                    ascending: item.ascending,
                })
                .collect();
            LogicalPlan::Sort { source, items }
        }
        other => other,
    }
}

/// Recursively fold constant sub-expressions.
fn fold_expr(expr: Expression) -> Expression {
    match expr {
        Expression::BinaryOp(op, left, right) => {
            let left = fold_expr(*left);
            let right = fold_expr(*right);
            fold_binary_op(op, left, right)
        }
        Expression::UnaryOp(op, inner) => {
            let inner = fold_expr(*inner);
            fold_unary_op(op, inner)
        }
        other => other,
    }
}

/// Fold a binary operation if both sides are literals.
fn fold_binary_op(op: BinaryOp, left: Expression, right: Expression) -> Expression {
    match (&left, &right) {
        // Arithmetic: Int op Int -> Int
        (Expression::Literal(Literal::Integer(a)), Expression::Literal(Literal::Integer(b))) => {
            match op {
                BinaryOp::Add => return Expression::Literal(Literal::Integer(a + b)),
                BinaryOp::Sub => return Expression::Literal(Literal::Integer(a - b)),
                BinaryOp::Mul => return Expression::Literal(Literal::Integer(a * b)),
                BinaryOp::Div => {
                    if *b != 0 {
                        return Expression::Literal(Literal::Integer(a / b));
                    }
                }
                BinaryOp::Mod => {
                    if *b != 0 {
                        return Expression::Literal(Literal::Integer(a % b));
                    }
                }
                _ => {}
            }
        }
        // Arithmetic: Float op Float -> Float
        (Expression::Literal(Literal::Float(a)), Expression::Literal(Literal::Float(b))) => {
            match op {
                BinaryOp::Add => return Expression::Literal(Literal::Float(a + b)),
                BinaryOp::Sub => return Expression::Literal(Literal::Float(a - b)),
                BinaryOp::Mul => return Expression::Literal(Literal::Float(a * b)),
                BinaryOp::Div => {
                    if *b != 0.0 {
                        return Expression::Literal(Literal::Float(a / b));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    // Boolean short-circuit folding: AND / OR with known operands
    match op {
        BinaryOp::And => {
            // false AND x -> false
            if left == Expression::Literal(Literal::Bool(false)) {
                return Expression::Literal(Literal::Bool(false));
            }
            // x AND false -> false
            if right == Expression::Literal(Literal::Bool(false)) {
                return Expression::Literal(Literal::Bool(false));
            }
            // true AND x -> x
            if left == Expression::Literal(Literal::Bool(true)) {
                return right;
            }
            // x AND true -> x
            if right == Expression::Literal(Literal::Bool(true)) {
                return left;
            }
        }
        BinaryOp::Or => {
            // true OR x -> true
            if left == Expression::Literal(Literal::Bool(true)) {
                return Expression::Literal(Literal::Bool(true));
            }
            // x OR true -> true
            if right == Expression::Literal(Literal::Bool(true)) {
                return Expression::Literal(Literal::Bool(true));
            }
            // false OR x -> x
            if left == Expression::Literal(Literal::Bool(false)) {
                return right;
            }
            // x OR false -> x
            if right == Expression::Literal(Literal::Bool(false)) {
                return left;
            }
        }
        _ => {}
    }

    Expression::BinaryOp(op, Box::new(left), Box::new(right))
}

/// Fold a unary operation if the operand is a literal.
fn fold_unary_op(op: UnaryOp, inner: Expression) -> Expression {
    match (&op, &inner) {
        (UnaryOp::Not, Expression::Literal(Literal::Bool(b))) => {
            Expression::Literal(Literal::Bool(!b))
        }
        (UnaryOp::Neg, Expression::Literal(Literal::Integer(n))) => {
            Expression::Literal(Literal::Integer(-n))
        }
        (UnaryOp::Neg, Expression::Literal(Literal::Float(f))) => {
            Expression::Literal(Literal::Float(-f))
        }
        _ => Expression::UnaryOp(op, Box::new(inner)),
    }
}

// ============================================================================
// TASK-116: Projection pruning
// ============================================================================

/// Merge consecutive Project nodes into a single Project.
fn prune_projections(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Project {
            source,
            items: outer_items,
            distinct: outer_distinct,
        } => {
            if let LogicalPlan::Project {
                source: inner_source,
                items: _inner_items,
                distinct: _inner_distinct,
            } = *source
            {
                // Merge: the outer projection determines the final columns.
                // The inner projection is redundant if the outer one re-selects.
                LogicalPlan::Project {
                    source: inner_source,
                    items: outer_items,
                    distinct: outer_distinct,
                }
            } else {
                LogicalPlan::Project {
                    source,
                    items: outer_items,
                    distinct: outer_distinct,
                }
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TASK-112: Index scan selection tests
    // ========================================================================

    #[test]
    fn test_index_scan_simple_eq() {
        // Filter(NodeScan(n, label=1), n.name == 'Alice')
        // -> IndexScan(n, label=1, prop_key='name', lookup='Alice')
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                )),
                Box::new(Expression::Literal(Literal::String("Alice".into()))),
            ),
        };

        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::IndexScan {
                variable: "n".into(),
                label_id: 1,
                prop_key: "name".into(),
                lookup_value: Expression::Literal(Literal::String("Alice".into())),
            }
        );
    }

    #[test]
    fn test_index_scan_reversed_eq() {
        // Filter(NodeScan(n, label=1), 'Alice' == n.name)
        // -> IndexScan(n, label=1, prop_key='name', lookup='Alice')
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Literal(Literal::String("Alice".into()))),
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                )),
            ),
        };

        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::IndexScan {
                variable: "n".into(),
                label_id: 1,
                prop_key: "name".into(),
                lookup_value: Expression::Literal(Literal::String("Alice".into())),
            }
        );
    }

    #[test]
    fn test_index_scan_integer_literal() {
        // Filter(NodeScan(n, label=2), n.age == 30)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(2),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "age".into(),
                )),
                Box::new(Expression::Literal(Literal::Integer(30))),
            ),
        };

        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::IndexScan {
                variable: "n".into(),
                label_id: 2,
                prop_key: "age".into(),
                lookup_value: Expression::Literal(Literal::Integer(30)),
            }
        );
    }

    #[test]
    fn test_index_scan_no_label() {
        // Filter(NodeScan(n, label=None), n.name == 'Alice')
        // -> NOT converted (no label_id)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: None,
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                )),
                Box::new(Expression::Literal(Literal::String("Alice".into()))),
            ),
        };

        let optimized = optimize(plan);
        // Should remain a Filter over NodeScan
        assert!(matches!(optimized, LogicalPlan::Filter { .. }));
    }

    #[test]
    fn test_index_scan_non_eq_predicate() {
        // Filter(NodeScan(n, label=1), n.age > 30)
        // -> NOT converted (not an equality predicate)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Gt,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "age".into(),
                )),
                Box::new(Expression::Literal(Literal::Integer(30))),
            ),
        };

        let optimized = optimize(plan);
        assert!(matches!(optimized, LogicalPlan::Filter { .. }));
    }

    #[test]
    fn test_index_scan_wrong_variable() {
        // Filter(NodeScan(n, label=1), m.name == 'Alice')
        // -> NOT converted (variable doesn't match)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("m".into())),
                    "name".into(),
                )),
                Box::new(Expression::Literal(Literal::String("Alice".into()))),
            ),
        };

        let optimized = optimize(plan);
        assert!(matches!(optimized, LogicalPlan::Filter { .. }));
    }

    #[test]
    fn test_index_scan_non_literal_value() {
        // Filter(NodeScan(n, label=1), n.name == m.name)
        // -> NOT converted (RHS is not a literal)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                )),
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("m".into())),
                    "name".into(),
                )),
            ),
        };

        let optimized = optimize(plan);
        assert!(matches!(optimized, LogicalPlan::Filter { .. }));
    }

    #[test]
    fn test_index_scan_and_predicate_extracts_one() {
        // Filter(NodeScan(n, label=1), n.name == 'Alice' AND n.age > 25)
        // -> Filter(IndexScan(n, name='Alice'), n.age > 25)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::And,
                Box::new(Expression::BinaryOp(
                    BinaryOp::Eq,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".into())),
                        "name".into(),
                    )),
                    Box::new(Expression::Literal(Literal::String("Alice".into()))),
                )),
                Box::new(Expression::BinaryOp(
                    BinaryOp::Gt,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".into())),
                        "age".into(),
                    )),
                    Box::new(Expression::Literal(Literal::Integer(25))),
                )),
            ),
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Filter {
                source, predicate, ..
            } => {
                assert!(matches!(*source, LogicalPlan::IndexScan { .. }));
                // Remaining predicate should be n.age > 25
                assert!(matches!(predicate, Expression::BinaryOp(BinaryOp::Gt, ..)));
            }
            _ => panic!("expected Filter(IndexScan, ...)"),
        }
    }

    #[test]
    fn test_index_scan_and_reversed_extracts_right() {
        // Filter(NodeScan(n, label=1), n.age > 25 AND n.name == 'Bob')
        // -> Filter(IndexScan(n, name='Bob'), n.age > 25)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::And,
                Box::new(Expression::BinaryOp(
                    BinaryOp::Gt,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".into())),
                        "age".into(),
                    )),
                    Box::new(Expression::Literal(Literal::Integer(25))),
                )),
                Box::new(Expression::BinaryOp(
                    BinaryOp::Eq,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".into())),
                        "name".into(),
                    )),
                    Box::new(Expression::Literal(Literal::String("Bob".into()))),
                )),
            ),
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Filter { source, .. } => {
                assert!(matches!(*source, LogicalPlan::IndexScan { .. }));
            }
            _ => panic!("expected Filter(IndexScan, ...)"),
        }
    }

    #[test]
    fn test_index_scan_source_not_nodescan() {
        // Filter(Expand(...), n.name == 'Alice')
        // -> NOT converted (source is not NodeScan)
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::Expand {
                source: Box::new(LogicalPlan::NodeScan {
                    variable: "m".into(),
                    label_id: Some(1),
                limit: None,
                }),
                src_var: "m".into(),
                rel_var: None,
                target_var: "n".into(),
                rel_type_id: None,
                direction: RelDirection::Outgoing,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Eq,
                Box::new(Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                )),
                Box::new(Expression::Literal(Literal::String("Alice".into()))),
            ),
        };

        let optimized = optimize(plan);
        assert!(matches!(optimized, LogicalPlan::Filter { .. }));
    }

    // ========================================================================
    // ========================================================================
    // TASK-114: LIMIT pushdown tests
    // ========================================================================

    #[test]
    fn test_limit_pushdown_into_nodescan() {
        // Limit(NodeScan(n, label=1), 5) -> NodeScan(n, label=1, limit=5)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            count: Expression::Literal(Literal::Integer(5)),
        };
        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(5),
            }
        );
    }

    #[test]
    fn test_limit_pushdown_no_label() {
        // Limit(NodeScan(n, None), 3) -> NodeScan(n, None, limit=3)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: None,
                limit: None,
            }),
            count: Expression::Literal(Literal::Integer(3)),
        };
        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: None,
                limit: Some(3),
            }
        );
    }

    #[test]
    fn test_limit_not_pushed_through_sort() {
        // Limit(Sort(NodeScan, items), 5) -> Limit(Sort(NodeScan, items), 5)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::Sort {
                source: Box::new(LogicalPlan::NodeScan {
                    variable: "n".into(),
                    label_id: Some(1),
                    limit: None,
                }),
                items: vec![OrderItem {
                    expr: Expression::Variable("n".into()),
                    ascending: true,
                }],
            }),
            count: Expression::Literal(Literal::Integer(5)),
        };
        let optimized = optimize(plan);
        // Should remain Limit(Sort(...))
        assert!(matches!(optimized, LogicalPlan::Limit { .. }));
    }

    #[test]
    fn test_limit_pushdown_non_literal() {
        // Limit(NodeScan, $param) -> no pushdown
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            count: Expression::Parameter("count".into()),
        };
        let optimized = optimize(plan);
        assert!(matches!(optimized, LogicalPlan::Limit { .. }));
    }

    #[test]
    fn test_limit_pushdown_merges_existing() {
        // Limit(NodeScan(n, label=1, limit=10), 5) -> NodeScan(n, limit=5)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(10),
            }),
            count: Expression::Literal(Literal::Integer(5)),
        };
        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(5),
            }
        );
    }

    #[test]
    fn test_limit_pushdown_keeps_smaller_existing() {
        // Limit(NodeScan(n, label=1, limit=3), 10) -> NodeScan(n, limit=3)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(3),
            }),
            count: Expression::Literal(Literal::Integer(10)),
        };
        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(3),
            }
        );
    }

    #[test]
    fn test_limit_pushdown_nested_in_project() {
        // Project(Limit(NodeScan, 2), [n]) -> after optimization, limit pushes into NodeScan
        let plan = LogicalPlan::Project {
            source: Box::new(LogicalPlan::Limit {
                source: Box::new(LogicalPlan::NodeScan {
                    variable: "n".into(),
                    label_id: Some(1),
                    limit: None,
                }),
                count: Expression::Literal(Literal::Integer(2)),
            }),
            items: vec![ReturnItem {
                expr: Expression::Variable("n".into()),
                alias: None,
            }],
            distinct: false,
        };
        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project { source, .. } => {
                assert_eq!(
                    *source,
                    LogicalPlan::NodeScan {
                        variable: "n".into(),
                        label_id: Some(1),
                        limit: Some(2),
                    }
                );
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_limit_pushdown_zero() {
        // Limit(NodeScan, 0) -> NodeScan(limit=0)
        let plan = LogicalPlan::Limit {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            count: Expression::Literal(Literal::Integer(0)),
        };
        let optimized = optimize(plan);
        assert_eq!(
            optimized,
            LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: Some(0),
            }
        );
    }

    // ========================================================================
    // TASK-115: Constant folding tests
    // ========================================================================

    #[test]
    fn test_fold_int_add() {
        let expr = Expression::BinaryOp(
            BinaryOp::Add,
            Box::new(Expression::Literal(Literal::Integer(1))),
            Box::new(Expression::Literal(Literal::Integer(2))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Integer(3)));
    }

    #[test]
    fn test_fold_int_sub() {
        let expr = Expression::BinaryOp(
            BinaryOp::Sub,
            Box::new(Expression::Literal(Literal::Integer(10))),
            Box::new(Expression::Literal(Literal::Integer(3))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Integer(7)));
    }

    #[test]
    fn test_fold_int_mul() {
        let expr = Expression::BinaryOp(
            BinaryOp::Mul,
            Box::new(Expression::Literal(Literal::Integer(4))),
            Box::new(Expression::Literal(Literal::Integer(5))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Integer(20)));
    }

    #[test]
    fn test_fold_int_div() {
        let expr = Expression::BinaryOp(
            BinaryOp::Div,
            Box::new(Expression::Literal(Literal::Integer(10))),
            Box::new(Expression::Literal(Literal::Integer(3))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Integer(3)));
    }

    #[test]
    fn test_fold_int_div_by_zero_no_fold() {
        let expr = Expression::BinaryOp(
            BinaryOp::Div,
            Box::new(Expression::Literal(Literal::Integer(10))),
            Box::new(Expression::Literal(Literal::Integer(0))),
        );
        // Should not fold (would cause runtime error)
        assert!(matches!(expr, Expression::BinaryOp(BinaryOp::Div, ..)));
    }

    #[test]
    fn test_fold_float_add() {
        let expr = Expression::BinaryOp(
            BinaryOp::Add,
            Box::new(Expression::Literal(Literal::Float(1.5))),
            Box::new(Expression::Literal(Literal::Float(2.5))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Float(4.0)));
    }

    #[test]
    fn test_fold_and_true_x() {
        let expr = Expression::BinaryOp(
            BinaryOp::And,
            Box::new(Expression::Literal(Literal::Bool(true))),
            Box::new(Expression::Variable("x".into())),
        );
        assert_eq!(fold_expr(expr), Expression::Variable("x".into()));
    }

    #[test]
    fn test_fold_and_false_x() {
        let expr = Expression::BinaryOp(
            BinaryOp::And,
            Box::new(Expression::Literal(Literal::Bool(false))),
            Box::new(Expression::Variable("x".into())),
        );
        assert_eq!(
            fold_expr(expr),
            Expression::Literal(Literal::Bool(false))
        );
    }

    #[test]
    fn test_fold_or_true_x() {
        let expr = Expression::BinaryOp(
            BinaryOp::Or,
            Box::new(Expression::Literal(Literal::Bool(true))),
            Box::new(Expression::Variable("x".into())),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Bool(true)));
    }

    #[test]
    fn test_fold_or_false_x() {
        let expr = Expression::BinaryOp(
            BinaryOp::Or,
            Box::new(Expression::Literal(Literal::Bool(false))),
            Box::new(Expression::Variable("x".into())),
        );
        assert_eq!(fold_expr(expr), Expression::Variable("x".into()));
    }

    #[test]
    fn test_fold_not_true() {
        let expr = Expression::UnaryOp(
            UnaryOp::Not,
            Box::new(Expression::Literal(Literal::Bool(true))),
        );
        assert_eq!(
            fold_expr(expr),
            Expression::Literal(Literal::Bool(false))
        );
    }

    #[test]
    fn test_fold_not_false() {
        let expr = Expression::UnaryOp(
            UnaryOp::Not,
            Box::new(Expression::Literal(Literal::Bool(false))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Bool(true)));
    }

    #[test]
    fn test_fold_nested() {
        // (1 + 2) * 3 -> 9
        let expr = Expression::BinaryOp(
            BinaryOp::Mul,
            Box::new(Expression::BinaryOp(
                BinaryOp::Add,
                Box::new(Expression::Literal(Literal::Integer(1))),
                Box::new(Expression::Literal(Literal::Integer(2))),
            )),
            Box::new(Expression::Literal(Literal::Integer(3))),
        );
        assert_eq!(fold_expr(expr), Expression::Literal(Literal::Integer(9)));
    }

    #[test]
    fn test_fold_filter_true_eliminates() {
        // Filter(source, true) -> source
        let source = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        let plan = LogicalPlan::Filter {
            source: Box::new(source.clone()),
            predicate: Expression::Literal(Literal::Bool(true)),
        };
        let optimized = optimize(plan);
        assert_eq!(optimized, source);
    }

    #[test]
    fn test_fold_in_filter_predicate() {
        // Filter(source, 1 + 2 == 3) -> after folding predicate becomes 3 == 3
        // The fold doesn't evaluate comparison, but folds the arithmetic
        let plan = LogicalPlan::Filter {
            source: Box::new(LogicalPlan::NodeScan {
                variable: "n".into(),
                label_id: Some(1),
                limit: None,
            }),
            predicate: Expression::BinaryOp(
                BinaryOp::Add,
                Box::new(Expression::Literal(Literal::Integer(1))),
                Box::new(Expression::Literal(Literal::Integer(2))),
            ),
        };
        let optimized = optimize(plan);
        // The predicate should be folded to 3
        match optimized {
            LogicalPlan::Filter { predicate, .. } => {
                assert_eq!(predicate, Expression::Literal(Literal::Integer(3)));
            }
            _ => panic!("expected Filter"),
        }
    }

    // ========================================================================
    // TASK-116: Projection pruning tests
    // ========================================================================

    #[test]
    fn test_prune_consecutive_projects() {
        // Project(Project(source, inner), outer) -> Project(source, outer)
        let source = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        let inner_items = vec![
            ReturnItem {
                expr: Expression::Variable("n".into()),
                alias: None,
            },
            ReturnItem {
                expr: Expression::Property(
                    Box::new(Expression::Variable("n".into())),
                    "name".into(),
                ),
                alias: Some("name".into()),
            },
        ];
        let outer_items = vec![ReturnItem {
            expr: Expression::Variable("name".into()),
            alias: None,
        }];

        let plan = LogicalPlan::Project {
            source: Box::new(LogicalPlan::Project {
                source: Box::new(source.clone()),
                items: inner_items,
                distinct: false,
            }),
            items: outer_items.clone(),
            distinct: false,
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project {
                source: inner_source,
                items,
                ..
            } => {
                // Should be Project(source, outer_items) - inner project eliminated
                assert_eq!(*inner_source, source);
                assert_eq!(items, outer_items);
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_no_prune_single_project() {
        let source = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        let items = vec![ReturnItem {
            expr: Expression::Variable("n".into()),
            alias: None,
        }];
        let plan = LogicalPlan::Project {
            source: Box::new(source.clone()),
            items: items.clone(),
            distinct: false,
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project {
                source: s, items: i, ..
            } => {
                assert_eq!(*s, source);
                assert_eq!(i, items);
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_prune_three_consecutive_projects() {
        // Project(Project(Project(source, a), b), c) -> Project(source, c)
        let source = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        let items_a = vec![ReturnItem {
            expr: Expression::Variable("a".into()),
            alias: None,
        }];
        let items_b = vec![ReturnItem {
            expr: Expression::Variable("b".into()),
            alias: None,
        }];
        let items_c = vec![ReturnItem {
            expr: Expression::Variable("c".into()),
            alias: None,
        }];

        let plan = LogicalPlan::Project {
            source: Box::new(LogicalPlan::Project {
                source: Box::new(LogicalPlan::Project {
                    source: Box::new(source.clone()),
                    items: items_a,
                    distinct: false,
                }),
                items: items_b,
                distinct: false,
            }),
            items: items_c.clone(),
            distinct: false,
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project {
                source: s, items, ..
            } => {
                // After bottom-up optimization, all three should collapse
                assert_eq!(*s, source);
                assert_eq!(items, items_c);
            }
            _ => panic!("expected Project"),
        }
    }

    #[test]
    fn test_prune_project_with_distinct_preserved() {
        let source = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        let items = vec![ReturnItem {
            expr: Expression::Variable("n".into()),
            alias: None,
        }];

        let plan = LogicalPlan::Project {
            source: Box::new(LogicalPlan::Project {
                source: Box::new(source.clone()),
                items: items.clone(),
                distinct: false,
            }),
            items: items.clone(),
            distinct: true, // outer has distinct
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project { distinct, .. } => {
                assert!(distinct); // outer's distinct is preserved
            }
            _ => panic!("expected Project"),
        }
    }

    // ========================================================================
    // Passthrough tests
    // ========================================================================

    #[test]
    fn test_optimize_passthrough_nodescan() {
        let plan = LogicalPlan::NodeScan {
            variable: "n".into(),
            label_id: Some(1),
                limit: None,
        };
        assert_eq!(optimize(plan.clone()), plan);
    }

    #[test]
    fn test_optimize_passthrough_empty_source() {
        let plan = LogicalPlan::EmptySource;
        assert_eq!(optimize(plan.clone()), plan);
    }

    #[test]
    fn test_optimize_nested_filter_in_project() {
        // Project(Filter(NodeScan(n, label=1), n.name == 'Alice'), [n])
        // -> Project(IndexScan(n, name='Alice'), [n])
        let plan = LogicalPlan::Project {
            source: Box::new(LogicalPlan::Filter {
                source: Box::new(LogicalPlan::NodeScan {
                    variable: "n".into(),
                    label_id: Some(1),
                limit: None,
                }),
                predicate: Expression::BinaryOp(
                    BinaryOp::Eq,
                    Box::new(Expression::Property(
                        Box::new(Expression::Variable("n".into())),
                        "name".into(),
                    )),
                    Box::new(Expression::Literal(Literal::String("Alice".into()))),
                ),
            }),
            items: vec![ReturnItem {
                expr: Expression::Variable("n".into()),
                alias: None,
            }],
            distinct: false,
        };

        let optimized = optimize(plan);
        match optimized {
            LogicalPlan::Project { source, .. } => {
                assert!(matches!(*source, LogicalPlan::IndexScan { .. }));
            }
            _ => panic!("expected Project(IndexScan)"),
        }
    }
}
