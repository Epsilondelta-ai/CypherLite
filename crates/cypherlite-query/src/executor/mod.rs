// Executor: Volcano/Iterator model physical operators
pub mod eval;
pub mod operators;

use crate::parser::ast::*;
use crate::planner::LogicalPlan;
use cypherlite_core::{EdgeId, LabelRegistry, NodeId, PropertyValue};
use cypherlite_storage::StorageEngine;
use std::collections::HashMap;

/// Runtime value in query execution. Extends PropertyValue with graph entity references.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Node(NodeId),
    Edge(EdgeId),
}

/// Convert from storage PropertyValue to executor Value.
impl From<PropertyValue> for Value {
    fn from(pv: PropertyValue) -> Self {
        match pv {
            PropertyValue::Null => Value::Null,
            PropertyValue::Bool(b) => Value::Bool(b),
            PropertyValue::Int64(i) => Value::Int64(i),
            PropertyValue::Float64(f) => Value::Float64(f),
            PropertyValue::String(s) => Value::String(s),
            PropertyValue::Bytes(b) => Value::Bytes(b),
            PropertyValue::Array(a) => Value::List(a.into_iter().map(Value::from).collect()),
        }
    }
}

/// Convert from executor Value to storage PropertyValue (for SET operations).
impl TryFrom<Value> for PropertyValue {
    type Error = String;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Null => Ok(PropertyValue::Null),
            Value::Bool(b) => Ok(PropertyValue::Bool(b)),
            Value::Int64(i) => Ok(PropertyValue::Int64(i)),
            Value::Float64(f) => Ok(PropertyValue::Float64(f)),
            Value::String(s) => Ok(PropertyValue::String(s)),
            Value::Bytes(b) => Ok(PropertyValue::Bytes(b)),
            Value::List(l) => {
                let items: Result<Vec<_>, _> = l.into_iter().map(PropertyValue::try_from).collect();
                Ok(PropertyValue::Array(items?))
            }
            Value::Node(_) | Value::Edge(_) => {
                Err("cannot convert graph entity to property".into())
            }
        }
    }
}

/// A row of named values produced during query execution.
pub type Record = HashMap<String, Value>;

/// Query parameters passed by the user.
pub type Params = HashMap<String, Value>;

/// Execution error.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionError {
    pub message: String,
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Execution error: {}", self.message)
    }
}

impl std::error::Error for ExecutionError {}

/// Execute a logical plan against a storage engine.
// @MX:ANCHOR: Main executor dispatch - called by api layer and all recursive plan nodes
// @MX:REASON: Central entry point for query execution; fan_in >= 3 (recursive + api + tests)
pub fn execute(
    plan: &LogicalPlan,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    match plan {
        LogicalPlan::EmptySource => Ok(vec![Record::new()]),
        LogicalPlan::NodeScan {
            variable, label_id, ..
        } => Ok(operators::node_scan::execute_node_scan(
            variable, *label_id, engine,
        )),
        LogicalPlan::Expand {
            source,
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        } => {
            let source_records = execute(source, engine, params)?;
            Ok(operators::expand::execute_expand(
                source_records,
                src_var,
                rel_var.as_deref(),
                target_var,
                *rel_type_id,
                direction,
                engine,
            ))
        }
        LogicalPlan::Filter { source, predicate } => {
            let source_records = execute(source, engine, params)?;
            operators::filter::execute_filter(source_records, predicate, engine, params)
        }
        LogicalPlan::Project {
            source,
            items,
            distinct,
        } => {
            let source_records = execute(source, engine, params)?;
            let mut result =
                operators::project::execute_project(source_records, items, engine, params)?;
            if *distinct {
                deduplicate_records(&mut result);
            }
            Ok(result)
        }
        LogicalPlan::Sort { source, items } => {
            let source_records = execute(source, engine, params)?;
            Ok(operators::sort::execute_sort(
                source_records,
                items,
                engine,
                params,
            ))
        }
        LogicalPlan::Skip { source, count } => {
            let source_records = execute(source, engine, params)?;
            let n = eval_count_expr(count)?;
            Ok(operators::limit::execute_skip(source_records, n))
        }
        LogicalPlan::Limit { source, count } => {
            let source_records = execute(source, engine, params)?;
            let n = eval_count_expr(count)?;
            Ok(operators::limit::execute_limit(source_records, n))
        }
        LogicalPlan::Aggregate {
            source,
            group_keys,
            aggregates,
        } => {
            let source_records = execute(source, engine, params)?;
            operators::aggregate::execute_aggregate(
                source_records,
                group_keys,
                aggregates,
                engine,
                params,
            )
        }
        LogicalPlan::CreateOp { source, pattern } => {
            let source_records = match source {
                Some(s) => execute(s, engine, params)?,
                None => vec![Record::new()],
            };
            operators::create::execute_create(source_records, pattern, engine, params)
        }
        LogicalPlan::DeleteOp {
            source,
            exprs,
            detach,
        } => {
            let source_records = execute(source, engine, params)?;
            operators::delete::execute_delete(source_records, exprs, *detach, engine, params)
        }
        LogicalPlan::SetOp { source, items } => {
            let source_records = execute(source, engine, params)?;
            operators::set_props::execute_set(source_records, items, engine, params)
        }
        LogicalPlan::RemoveOp { source, items } => {
            let source_records = execute(source, engine, params)?;
            operators::set_props::execute_remove(source_records, items, engine, params)
        }
        LogicalPlan::Unwind {
            source,
            expr,
            variable,
        } => {
            let source_records = execute(source, engine, params)?;
            operators::unwind::execute_unwind(source_records, expr, variable, engine, params)
        }
        LogicalPlan::With {
            source,
            items,
            where_clause,
            distinct,
        } => {
            let source_records = execute(source, engine, params)?;
            let mut result =
                operators::with::execute_with(source_records, items, engine, params)?;
            if *distinct {
                deduplicate_records(&mut result);
            }
            if let Some(ref predicate) = where_clause {
                result = operators::filter::execute_filter(result, predicate, engine, params)?;
            }
            Ok(result)
        }
        LogicalPlan::MergeOp {
            source,
            pattern,
            on_match,
            on_create,
        } => {
            let source_records = match source {
                Some(s) => execute(s, engine, params)?,
                None => vec![Record::new()],
            };
            operators::merge::execute_merge(source_records, pattern, on_match, on_create, engine, params)
        }
        LogicalPlan::CreateIndex {
            name,
            label,
            property,
        } => {
            // Resolve label and property names via catalog
            let label_id = engine.get_or_create_label(label);
            let prop_key_id = engine.get_or_create_prop_key(property);

            // Generate index name if not provided
            let index_name = match name {
                Some(n) => n.clone(),
                None => format!("idx_{}_{}", label, property),
            };

            // Create the index
            engine
                .index_manager_mut()
                .create_index(index_name.clone(), label_id, prop_key_id)
                .map_err(|e| ExecutionError {
                    message: e.to_string(),
                })?;

            // Register in catalog
            engine.catalog_mut().add_index_definition(
                cypherlite_storage::index::IndexDefinition {
                    name: index_name,
                    label_id,
                    prop_key_id,
                },
            );

            // Backfill: index existing nodes that match the label + property
            let nodes: Vec<(cypherlite_core::NodeId, Vec<(u32, cypherlite_core::PropertyValue)>)> = engine
                .scan_nodes_by_label(label_id)
                .iter()
                .map(|n| (n.node_id, n.properties.clone()))
                .collect();
            for (nid, props) in &nodes {
                for (pk, v) in props {
                    if *pk == prop_key_id {
                        if let Some(idx) = engine.index_manager_mut().find_index_mut(label_id, prop_key_id) {
                            idx.insert(v, *nid);
                        }
                    }
                }
            }

            Ok(vec![])
        }
        LogicalPlan::DropIndex { name } => {
            // Remove from index manager
            engine
                .index_manager_mut()
                .drop_index(name)
                .map_err(|e| ExecutionError {
                    message: e.to_string(),
                })?;

            // Remove from catalog
            engine.catalog_mut().remove_index_definition(name);

            Ok(vec![])
        }
        LogicalPlan::OptionalExpand {
            source,
            src_var,
            rel_var,
            target_var,
            rel_type_id,
            direction,
        } => {
            let source_records = execute(source, engine, params)?;
            Ok(operators::optional_expand::execute_optional_expand(
                source_records,
                src_var,
                rel_var.as_deref(),
                target_var,
                *rel_type_id,
                direction,
                engine,
            ))
        }
    }
}

/// Evaluate a SKIP/LIMIT count expression (must be a literal integer).
fn eval_count_expr(expr: &Expression) -> Result<usize, ExecutionError> {
    match expr {
        Expression::Literal(Literal::Integer(n)) => {
            if *n < 0 {
                return Err(ExecutionError {
                    message: "SKIP/LIMIT count must be non-negative".to_string(),
                });
            }
            Ok(*n as usize)
        }
        _ => Err(ExecutionError {
            message: "SKIP/LIMIT count must be a literal integer".to_string(),
        }),
    }
}

/// Deduplicate records by comparing all key-value pairs.
fn deduplicate_records(records: &mut Vec<Record>) {
    let mut seen: Vec<Record> = Vec::new();
    records.retain(|r| {
        if seen.contains(r) {
            false
        } else {
            seen.push(r.clone());
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // TASK-044: Value, Record, Params, ExecutionError tests
    // ======================================================================

    #[test]
    fn test_value_from_property_value_null() {
        assert_eq!(Value::from(PropertyValue::Null), Value::Null);
    }

    #[test]
    fn test_value_from_property_value_bool() {
        assert_eq!(Value::from(PropertyValue::Bool(true)), Value::Bool(true));
    }

    #[test]
    fn test_value_from_property_value_int64() {
        assert_eq!(Value::from(PropertyValue::Int64(42)), Value::Int64(42));
    }

    #[test]
    fn test_value_from_property_value_float64() {
        assert_eq!(
            Value::from(PropertyValue::Float64(3.15)),
            Value::Float64(3.15)
        );
    }

    #[test]
    fn test_value_from_property_value_string() {
        assert_eq!(
            Value::from(PropertyValue::String("hello".into())),
            Value::String("hello".into())
        );
    }

    #[test]
    fn test_value_from_property_value_bytes() {
        assert_eq!(
            Value::from(PropertyValue::Bytes(vec![1, 2, 3])),
            Value::Bytes(vec![1, 2, 3])
        );
    }

    #[test]
    fn test_value_from_property_value_array() {
        let pv = PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null]);
        assert_eq!(
            Value::from(pv),
            Value::List(vec![Value::Int64(1), Value::Null])
        );
    }

    #[test]
    fn test_value_try_into_property_value_success() {
        assert_eq!(
            PropertyValue::try_from(Value::Null),
            Ok(PropertyValue::Null)
        );
        assert_eq!(
            PropertyValue::try_from(Value::Bool(false)),
            Ok(PropertyValue::Bool(false))
        );
        assert_eq!(
            PropertyValue::try_from(Value::Int64(10)),
            Ok(PropertyValue::Int64(10))
        );
        assert_eq!(
            PropertyValue::try_from(Value::Float64(1.5)),
            Ok(PropertyValue::Float64(1.5))
        );
        assert_eq!(
            PropertyValue::try_from(Value::String("x".into())),
            Ok(PropertyValue::String("x".into()))
        );
        assert_eq!(
            PropertyValue::try_from(Value::Bytes(vec![0xAB])),
            Ok(PropertyValue::Bytes(vec![0xAB]))
        );
    }

    #[test]
    fn test_value_try_into_property_value_list() {
        let v = Value::List(vec![Value::Int64(1), Value::Bool(true)]);
        let pv = PropertyValue::try_from(v);
        assert_eq!(
            pv,
            Ok(PropertyValue::Array(vec![
                PropertyValue::Int64(1),
                PropertyValue::Bool(true)
            ]))
        );
    }

    #[test]
    fn test_value_try_into_property_value_node_fails() {
        let result = PropertyValue::try_from(Value::Node(NodeId(1)));
        assert!(result.is_err());
        assert!(result
            .expect_err("should error")
            .contains("cannot convert graph entity"));
    }

    #[test]
    fn test_value_try_into_property_value_edge_fails() {
        let result = PropertyValue::try_from(Value::Edge(EdgeId(1)));
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_error_display() {
        let err = ExecutionError {
            message: "test error".to_string(),
        };
        assert_eq!(err.to_string(), "Execution error: test error");
    }

    #[test]
    fn test_execution_error_is_error_trait() {
        let err = ExecutionError {
            message: "test".to_string(),
        };
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_record_type_is_hashmap() {
        let mut record: Record = Record::new();
        record.insert("n".to_string(), Value::Node(NodeId(1)));
        assert_eq!(record.get("n"), Some(&Value::Node(NodeId(1))));
    }

    #[test]
    fn test_params_type_is_hashmap() {
        let mut params: Params = Params::new();
        params.insert("name".to_string(), Value::String("Alice".into()));
        assert_eq!(
            params.get("name"),
            Some(&Value::String("Alice".to_string()))
        );
    }

    #[test]
    fn test_eval_count_expr_positive() {
        let expr = Expression::Literal(Literal::Integer(10));
        assert_eq!(eval_count_expr(&expr), Ok(10));
    }

    #[test]
    fn test_eval_count_expr_zero() {
        let expr = Expression::Literal(Literal::Integer(0));
        assert_eq!(eval_count_expr(&expr), Ok(0));
    }

    #[test]
    fn test_eval_count_expr_negative_fails() {
        let expr = Expression::Literal(Literal::Integer(-5));
        assert!(eval_count_expr(&expr).is_err());
    }

    #[test]
    fn test_eval_count_expr_non_integer_fails() {
        let expr = Expression::Variable("n".to_string());
        assert!(eval_count_expr(&expr).is_err());
    }

    #[test]
    fn test_deduplicate_records() {
        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::Int64(1));
        let mut r2 = Record::new();
        r2.insert("x".to_string(), Value::Int64(2));
        let r3 = r1.clone();

        let mut records = vec![r1, r2, r3];
        deduplicate_records(&mut records);
        assert_eq!(records.len(), 2);
    }
}
