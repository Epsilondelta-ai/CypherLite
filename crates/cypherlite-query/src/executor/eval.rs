// Expression evaluator: eval(), eval_cmp() with typed comparison

use super::{ExecutionError, Params, Record, Value};
use crate::parser::ast::*;
use cypherlite_core::LabelRegistry;
use cypherlite_storage::StorageEngine;

/// Evaluate an expression against a record (current row bindings).
pub fn eval(
    expr: &Expression,
    record: &Record,
    engine: &StorageEngine,
    params: &Params,
) -> Result<Value, ExecutionError> {
    match expr {
        Expression::Literal(lit) => Ok(eval_literal(lit)),
        Expression::Variable(name) => Ok(record.get(name).cloned().unwrap_or(Value::Null)),
        Expression::Property(inner_expr, prop_name) => {
            let inner = eval(inner_expr, record, engine, params)?;
            eval_property_access(&inner, prop_name, engine)
        }
        Expression::Parameter(name) => Ok(params.get(name).cloned().unwrap_or(Value::Null)),
        Expression::BinaryOp(op, lhs, rhs) => {
            let left = eval(lhs, record, engine, params)?;
            let right = eval(rhs, record, engine, params)?;
            eval_binary_op(*op, &left, &right)
        }
        Expression::UnaryOp(op, inner) => {
            let val = eval(inner, record, engine, params)?;
            eval_unary_op(*op, &val)
        }
        Expression::IsNull(inner, negated) => {
            let val = eval(inner, record, engine, params)?;
            let is_null = val == Value::Null;
            if *negated {
                Ok(Value::Bool(!is_null))
            } else {
                Ok(Value::Bool(is_null))
            }
        }
        // CountStar and FunctionCall with aggregates are handled at the aggregate level.
        // When encountered in non-aggregate context, return Null.
        Expression::CountStar => Ok(Value::Null),
        Expression::FunctionCall { name, args, .. } => {
            // Non-aggregate function calls: evaluate if known
            let func_name = name.to_lowercase();
            match func_name.as_str() {
                "count" | "sum" | "avg" | "min" | "max" | "collect" => {
                    // Aggregates are handled by AggregateOp, return Null here.
                    Ok(Value::Null)
                }
                "id" => {
                    if args.len() != 1 {
                        return Err(ExecutionError {
                            message: "id() requires exactly one argument".to_string(),
                        });
                    }
                    let val = eval(&args[0], record, engine, params)?;
                    match val {
                        Value::Node(nid) => Ok(Value::Int64(nid.0 as i64)),
                        Value::Edge(eid) => Ok(Value::Int64(eid.0 as i64)),
                        _ => Err(ExecutionError {
                            message: "id() requires a node or edge argument".to_string(),
                        }),
                    }
                }
                "type" => {
                    if args.len() != 1 {
                        return Err(ExecutionError {
                            message: "type() requires exactly one argument".to_string(),
                        });
                    }
                    let val = eval(&args[0], record, engine, params)?;
                    match val {
                        Value::Edge(eid) => {
                            if let Some(edge) = engine.get_edge(eid) {
                                let type_name = engine
                                    .catalog()
                                    .rel_type_name(edge.rel_type_id)
                                    .unwrap_or("")
                                    .to_string();
                                Ok(Value::String(type_name))
                            } else {
                                Ok(Value::Null)
                            }
                        }
                        _ => Err(ExecutionError {
                            message: "type() requires an edge argument".to_string(),
                        }),
                    }
                }
                "labels" => {
                    if args.len() != 1 {
                        return Err(ExecutionError {
                            message: "labels() requires exactly one argument".to_string(),
                        });
                    }
                    let val = eval(&args[0], record, engine, params)?;
                    match val {
                        Value::Node(nid) => {
                            if let Some(node) = engine.get_node(nid) {
                                let label_names: Vec<Value> = node
                                    .labels
                                    .iter()
                                    .filter_map(|lid| {
                                        engine
                                            .catalog()
                                            .label_name(*lid)
                                            .map(|n| Value::String(n.to_string()))
                                    })
                                    .collect();
                                Ok(Value::List(label_names))
                            } else {
                                Ok(Value::Null)
                            }
                        }
                        _ => Err(ExecutionError {
                            message: "labels() requires a node argument".to_string(),
                        }),
                    }
                }
                _ => Err(ExecutionError {
                    message: format!("unknown function: {}", name),
                }),
            }
        }
    }
}

/// Convert a literal AST node to a Value.
fn eval_literal(lit: &Literal) -> Value {
    match lit {
        Literal::Integer(i) => Value::Int64(*i),
        Literal::Float(f) => Value::Float64(*f),
        Literal::String(s) => Value::String(s.clone()),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Null => Value::Null,
    }
}

/// Access a property on a Value. For Node/Edge, look up from engine.
fn eval_property_access(
    val: &Value,
    prop_name: &str,
    engine: &StorageEngine,
) -> Result<Value, ExecutionError> {
    match val {
        Value::Node(nid) => {
            let node = engine.get_node(*nid).ok_or_else(|| ExecutionError {
                message: format!("node {} not found", nid.0),
            })?;
            let prop_key_id = engine.catalog().prop_key_id(prop_name);
            match prop_key_id {
                Some(kid) => {
                    for (k, v) in &node.properties {
                        if *k == kid {
                            return Ok(Value::from(v.clone()));
                        }
                    }
                    Ok(Value::Null)
                }
                None => Ok(Value::Null),
            }
        }
        Value::Edge(eid) => {
            let edge = engine.get_edge(*eid).ok_or_else(|| ExecutionError {
                message: format!("edge {} not found", eid.0),
            })?;
            let prop_key_id = engine.catalog().prop_key_id(prop_name);
            match prop_key_id {
                Some(kid) => {
                    for (k, v) in &edge.properties {
                        if *k == kid {
                            return Ok(Value::from(v.clone()));
                        }
                    }
                    Ok(Value::Null)
                }
                None => Ok(Value::Null),
            }
        }
        Value::Null => Ok(Value::Null),
        _ => Err(ExecutionError {
            message: format!("cannot access property '{}' on non-entity value", prop_name),
        }),
    }
}

/// Evaluate a binary operation.
fn eval_binary_op(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, ExecutionError> {
    match op {
        BinaryOp::And => eval_boolean_op(op, left, right),
        BinaryOp::Or => eval_boolean_op(op, left, right),
        BinaryOp::Eq
        | BinaryOp::Neq
        | BinaryOp::Lt
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Gte => eval_cmp(left, right, op),
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            eval_arithmetic(left, right, op)
        }
    }
}

/// Compare two values with the given comparison operator.
pub fn eval_cmp(left: &Value, right: &Value, op: BinaryOp) -> Result<Value, ExecutionError> {
    // Null semantics: any comparison with Null yields false (Cypher three-valued logic)
    if *left == Value::Null || *right == Value::Null {
        return Ok(Value::Bool(false));
    }

    match (left, right) {
        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Bool(cmp_ord(a, b, op))),
        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Bool(cmp_f64(*a, *b, op))),
        // Int64 vs Float64 promotion
        (Value::Int64(a), Value::Float64(b)) => Ok(Value::Bool(cmp_f64(*a as f64, *b, op))),
        (Value::Float64(a), Value::Int64(b)) => Ok(Value::Bool(cmp_f64(*a, *b as f64, op))),
        (Value::String(a), Value::String(b)) => Ok(Value::Bool(cmp_ord(a, b, op))),
        (Value::Bool(a), Value::Bool(b)) => {
            // Only Eq and Neq make sense for booleans
            match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Neq => Ok(Value::Bool(a != b)),
                _ => Err(ExecutionError {
                    message: "cannot order boolean values".to_string(),
                }),
            }
        }
        // Node/Edge ID equality
        (Value::Node(a), Value::Node(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::Neq => Ok(Value::Bool(a != b)),
            _ => Err(ExecutionError {
                message: "cannot order node values".to_string(),
            }),
        },
        (Value::Edge(a), Value::Edge(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::Neq => Ok(Value::Bool(a != b)),
            _ => Err(ExecutionError {
                message: "cannot order edge values".to_string(),
            }),
        },
        _ => Err(ExecutionError {
            message: "type mismatch in comparison".to_string(),
        }),
    }
}

/// Compare two Ord values with the given operation.
fn cmp_ord<T: Ord>(a: &T, b: &T, op: BinaryOp) -> bool {
    match op {
        BinaryOp::Eq => a == b,
        BinaryOp::Neq => a != b,
        BinaryOp::Lt => a < b,
        BinaryOp::Lte => a <= b,
        BinaryOp::Gt => a > b,
        BinaryOp::Gte => a >= b,
        _ => false,
    }
}

/// Compare two f64 values with the given operation.
fn cmp_f64(a: f64, b: f64, op: BinaryOp) -> bool {
    match op {
        BinaryOp::Eq => (a - b).abs() < f64::EPSILON,
        BinaryOp::Neq => (a - b).abs() >= f64::EPSILON,
        BinaryOp::Lt => a < b,
        BinaryOp::Lte => a <= b,
        BinaryOp::Gt => a > b,
        BinaryOp::Gte => a >= b,
        _ => false,
    }
}

/// Evaluate arithmetic operations.
fn eval_arithmetic(left: &Value, right: &Value, op: BinaryOp) -> Result<Value, ExecutionError> {
    match (left, right) {
        (Value::Int64(a), Value::Int64(b)) => match op {
            BinaryOp::Add => Ok(Value::Int64(a.wrapping_add(*b))),
            BinaryOp::Sub => Ok(Value::Int64(a.wrapping_sub(*b))),
            BinaryOp::Mul => Ok(Value::Int64(a.wrapping_mul(*b))),
            BinaryOp::Div => {
                if *b == 0 {
                    return Err(ExecutionError {
                        message: "division by zero".to_string(),
                    });
                }
                Ok(Value::Int64(a / b))
            }
            BinaryOp::Mod => {
                if *b == 0 {
                    return Err(ExecutionError {
                        message: "division by zero".to_string(),
                    });
                }
                Ok(Value::Int64(a % b))
            }
            _ => Err(ExecutionError {
                message: "unexpected arithmetic op".to_string(),
            }),
        },
        (Value::Float64(a), Value::Float64(b)) => eval_float_arithmetic(*a, *b, op),
        (Value::Int64(a), Value::Float64(b)) => eval_float_arithmetic(*a as f64, *b, op),
        (Value::Float64(a), Value::Int64(b)) => eval_float_arithmetic(*a, *b as f64, op),
        (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
        _ => Err(ExecutionError {
            message: "type mismatch in arithmetic operation".to_string(),
        }),
    }
}

/// Evaluate float arithmetic.
fn eval_float_arithmetic(a: f64, b: f64, op: BinaryOp) -> Result<Value, ExecutionError> {
    match op {
        BinaryOp::Add => Ok(Value::Float64(a + b)),
        BinaryOp::Sub => Ok(Value::Float64(a - b)),
        BinaryOp::Mul => Ok(Value::Float64(a * b)),
        BinaryOp::Div => {
            if b == 0.0 {
                return Err(ExecutionError {
                    message: "division by zero".to_string(),
                });
            }
            Ok(Value::Float64(a / b))
        }
        BinaryOp::Mod => {
            if b == 0.0 {
                return Err(ExecutionError {
                    message: "division by zero".to_string(),
                });
            }
            Ok(Value::Float64(a % b))
        }
        _ => Err(ExecutionError {
            message: "unexpected arithmetic op".to_string(),
        }),
    }
}

/// Evaluate boolean operations (AND, OR).
fn eval_boolean_op(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, ExecutionError> {
    // Null propagation for boolean ops
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => match op {
            BinaryOp::And => Ok(Value::Bool(*a && *b)),
            BinaryOp::Or => Ok(Value::Bool(*a || *b)),
            _ => Err(ExecutionError {
                message: "unexpected boolean op".to_string(),
            }),
        },
        (Value::Null, Value::Bool(b)) => match op {
            BinaryOp::And => {
                if !b {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Null)
                }
            }
            BinaryOp::Or => {
                if *b {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => Err(ExecutionError {
                message: "unexpected boolean op".to_string(),
            }),
        },
        (Value::Bool(a), Value::Null) => match op {
            BinaryOp::And => {
                if !a {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Null)
                }
            }
            BinaryOp::Or => {
                if *a {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => Err(ExecutionError {
                message: "unexpected boolean op".to_string(),
            }),
        },
        (Value::Null, Value::Null) => Ok(Value::Null),
        _ => Err(ExecutionError {
            message: "non-boolean operand in boolean operation".to_string(),
        }),
    }
}

/// Evaluate a unary operation.
fn eval_unary_op(op: UnaryOp, val: &Value) -> Result<Value, ExecutionError> {
    match op {
        UnaryOp::Not => match val {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            Value::Null => Ok(Value::Null),
            _ => Err(ExecutionError {
                message: "NOT requires a boolean operand".to_string(),
            }),
        },
        UnaryOp::Neg => match val {
            Value::Int64(i) => Ok(Value::Int64(-i)),
            Value::Float64(f) => Ok(Value::Float64(-f)),
            Value::Null => Ok(Value::Null),
            _ => Err(ExecutionError {
                message: "unary minus requires a numeric operand".to_string(),
            }),
        },
    }
}

/// Utility: compare two Values for sorting. Returns Ordering.
/// Used by SortOp.
pub fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Int64(x), Value::Int64(y)) => x.cmp(y),
        (Value::Float64(x), Value::Float64(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Int64(x), Value::Float64(y)) => {
            (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (Value::Float64(x), Value::Int64(y)) => {
            x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::SyncMode;
    use cypherlite_storage::StorageEngine;
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = cypherlite_core::DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // ======================================================================
    // TASK-045: eval() and eval_cmp() tests
    // ======================================================================

    // EXEC-T007: eval_cmp Int64 vs Float64 (promotion)
    #[test]
    fn test_eval_cmp_int_vs_float_promotion() {
        let result = eval_cmp(&Value::Int64(5), &Value::Float64(5.0), BinaryOp::Eq);
        assert_eq!(result, Ok(Value::Bool(true)));

        let result = eval_cmp(&Value::Int64(5), &Value::Float64(6.0), BinaryOp::Lt);
        assert_eq!(result, Ok(Value::Bool(true)));

        let result = eval_cmp(&Value::Float64(3.0), &Value::Int64(3), BinaryOp::Eq);
        assert_eq!(result, Ok(Value::Bool(true)));
    }

    // EXEC-T008: eval_cmp Null vs anything -> false
    #[test]
    fn test_eval_cmp_null_always_false() {
        assert_eq!(
            eval_cmp(&Value::Null, &Value::Int64(1), BinaryOp::Eq),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            eval_cmp(&Value::Int64(1), &Value::Null, BinaryOp::Eq),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            eval_cmp(&Value::Null, &Value::Null, BinaryOp::Eq),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            eval_cmp(&Value::Null, &Value::String("x".into()), BinaryOp::Lt),
            Ok(Value::Bool(false))
        );
    }

    // EXEC-T009: eval_cmp type mismatch -> ExecutionError
    #[test]
    fn test_eval_cmp_type_mismatch() {
        let result = eval_cmp(&Value::Int64(1), &Value::String("x".into()), BinaryOp::Eq);
        assert!(result.is_err());
        assert!(result
            .err()
            .expect("should error")
            .message
            .contains("type mismatch"));
    }

    #[test]
    fn test_eval_cmp_int_int() {
        assert_eq!(
            eval_cmp(&Value::Int64(3), &Value::Int64(5), BinaryOp::Lt),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(&Value::Int64(5), &Value::Int64(5), BinaryOp::Lte),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(&Value::Int64(5), &Value::Int64(3), BinaryOp::Gt),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(&Value::Int64(5), &Value::Int64(5), BinaryOp::Gte),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(&Value::Int64(3), &Value::Int64(5), BinaryOp::Neq),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn test_eval_cmp_string_string() {
        assert_eq!(
            eval_cmp(
                &Value::String("abc".into()),
                &Value::String("def".into()),
                BinaryOp::Lt
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(
                &Value::String("abc".into()),
                &Value::String("abc".into()),
                BinaryOp::Eq
            ),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn test_eval_literal() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::Literal(Literal::Integer(42)),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Int64(42)));

        let result = eval(
            &Expression::Literal(Literal::Float(3.14)),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Float64(3.14)));

        let result = eval(
            &Expression::Literal(Literal::String("hello".into())),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::String("hello".into())));

        let result = eval(
            &Expression::Literal(Literal::Bool(true)),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Bool(true)));

        let result = eval(
            &Expression::Literal(Literal::Null),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Null));
    }

    #[test]
    fn test_eval_variable_lookup() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let mut record = Record::new();
        record.insert("x".to_string(), Value::Int64(99));
        let params = Params::new();

        let result = eval(
            &Expression::Variable("x".to_string()),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Int64(99)));

        // Missing variable returns Null
        let result = eval(
            &Expression::Variable("missing".to_string()),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Null));
    }

    #[test]
    fn test_eval_parameter_lookup() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let mut params = Params::new();
        params.insert("name".to_string(), Value::String("Alice".into()));

        let result = eval(
            &Expression::Parameter("name".to_string()),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::String("Alice".into())));

        // Missing parameter returns Null
        let result = eval(
            &Expression::Parameter("missing".to_string()),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Null));
    }

    #[test]
    fn test_eval_property_access_on_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        // Register property key and create node
        let name_key = engine.get_or_create_prop_key("name");
        let nid = engine.create_node(
            vec![],
            vec![(
                name_key,
                cypherlite_core::PropertyValue::String("Alice".into()),
            )],
        );

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(nid));
        let params = Params::new();

        let result = eval(
            &Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "name".to_string(),
            ),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::String("Alice".into())));

        // Non-existent property returns Null
        let result = eval(
            &Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "age".to_string(),
            ),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Null));
    }

    #[test]
    fn test_eval_arithmetic_int() {
        assert_eq!(
            eval_arithmetic(&Value::Int64(10), &Value::Int64(3), BinaryOp::Add),
            Ok(Value::Int64(13))
        );
        assert_eq!(
            eval_arithmetic(&Value::Int64(10), &Value::Int64(3), BinaryOp::Sub),
            Ok(Value::Int64(7))
        );
        assert_eq!(
            eval_arithmetic(&Value::Int64(10), &Value::Int64(3), BinaryOp::Mul),
            Ok(Value::Int64(30))
        );
        assert_eq!(
            eval_arithmetic(&Value::Int64(10), &Value::Int64(3), BinaryOp::Div),
            Ok(Value::Int64(3))
        );
        assert_eq!(
            eval_arithmetic(&Value::Int64(10), &Value::Int64(3), BinaryOp::Mod),
            Ok(Value::Int64(1))
        );
    }

    #[test]
    fn test_eval_arithmetic_division_by_zero() {
        assert!(eval_arithmetic(&Value::Int64(10), &Value::Int64(0), BinaryOp::Div).is_err());
        assert!(
            eval_arithmetic(&Value::Float64(10.0), &Value::Float64(0.0), BinaryOp::Div).is_err()
        );
    }

    #[test]
    fn test_eval_arithmetic_mixed_types() {
        let result = eval_arithmetic(&Value::Int64(10), &Value::Float64(2.5), BinaryOp::Add);
        assert_eq!(result, Ok(Value::Float64(12.5)));
    }

    #[test]
    fn test_eval_arithmetic_null_propagation() {
        assert_eq!(
            eval_arithmetic(&Value::Null, &Value::Int64(5), BinaryOp::Add),
            Ok(Value::Null)
        );
    }

    #[test]
    fn test_eval_arithmetic_type_mismatch() {
        assert!(
            eval_arithmetic(&Value::String("x".into()), &Value::Int64(1), BinaryOp::Add).is_err()
        );
    }

    #[test]
    fn test_eval_boolean_and_or() {
        assert_eq!(
            eval_boolean_op(BinaryOp::And, &Value::Bool(true), &Value::Bool(false)),
            Ok(Value::Bool(false))
        );
        assert_eq!(
            eval_boolean_op(BinaryOp::And, &Value::Bool(true), &Value::Bool(true)),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_boolean_op(BinaryOp::Or, &Value::Bool(false), &Value::Bool(true)),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_boolean_op(BinaryOp::Or, &Value::Bool(false), &Value::Bool(false)),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn test_eval_boolean_non_bool_error() {
        assert!(eval_boolean_op(BinaryOp::And, &Value::Int64(1), &Value::Bool(true)).is_err());
    }

    #[test]
    fn test_eval_is_null() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let mut record = Record::new();
        record.insert("x".to_string(), Value::Null);
        record.insert("y".to_string(), Value::Int64(1));
        let params = Params::new();

        // IS NULL
        let result = eval(
            &Expression::IsNull(Box::new(Expression::Variable("x".to_string())), false),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Bool(true)));

        // IS NOT NULL
        let result = eval(
            &Expression::IsNull(Box::new(Expression::Variable("y".to_string())), true),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Bool(true)));
    }

    #[test]
    fn test_eval_unary_not() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::UnaryOp(
                UnaryOp::Not,
                Box::new(Expression::Literal(Literal::Bool(true))),
            ),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Bool(false)));
    }

    #[test]
    fn test_eval_unary_neg() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::UnaryOp(
                UnaryOp::Neg,
                Box::new(Expression::Literal(Literal::Integer(42))),
            ),
            &record,
            &engine,
            &params,
        );
        assert_eq!(result, Ok(Value::Int64(-42)));
    }

    #[test]
    fn test_compare_values_ordering() {
        use std::cmp::Ordering;
        assert_eq!(
            compare_values(&Value::Int64(1), &Value::Int64(2)),
            Ordering::Less
        );
        assert_eq!(
            compare_values(&Value::Null, &Value::Int64(1)),
            Ordering::Less
        );
        assert_eq!(
            compare_values(&Value::Int64(1), &Value::Null),
            Ordering::Greater
        );
        assert_eq!(
            compare_values(&Value::String("a".into()), &Value::String("b".into())),
            Ordering::Less
        );
    }
}
