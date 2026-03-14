// Expression evaluator: eval(), eval_cmp() with typed comparison

use super::{ExecutionError, Params, Record, ScalarFnLookup, Value};
use crate::parser::ast::*;
use cypherlite_core::LabelRegistry;
use cypherlite_storage::StorageEngine;

// @MX:ANCHOR: [AUTO] Expression evaluator — called by Filter, Project, Create, Set, and Sort operators
// @MX:REASON: fan_in >= 5; core evaluation logic for all WHERE/RETURN/SET expressions
/// Evaluate an expression against a record (current row bindings).
pub fn eval(
    expr: &Expression,
    record: &Record,
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Value, ExecutionError> {
    match expr {
        Expression::Literal(lit) => Ok(eval_literal(lit)),
        Expression::Variable(name) => Ok(record.get(name).cloned().unwrap_or(Value::Null)),
        Expression::Property(inner_expr, prop_name) => {
            // Check for temporal property override (from AT TIME / BETWEEN TIME queries)
            if let Expression::Variable(var_name) = inner_expr.as_ref() {
                let temporal_key = format!("__temporal_props__{}", var_name);
                if let Some(Value::List(props_list)) = record.get(&temporal_key) {
                    return eval_temporal_property_access(props_list, prop_name, engine);
                }
            }
            let inner = eval(inner_expr, record, engine, params, scalar_fns)?;
            eval_property_access(&inner, prop_name, engine)
        }
        Expression::Parameter(name) => Ok(params.get(name).cloned().unwrap_or(Value::Null)),
        Expression::BinaryOp(op, lhs, rhs) => {
            let left = eval(lhs, record, engine, params, scalar_fns)?;
            let right = eval(rhs, record, engine, params, scalar_fns)?;
            eval_binary_op(*op, &left, &right)
        }
        Expression::UnaryOp(op, inner) => {
            let val = eval(inner, record, engine, params, scalar_fns)?;
            eval_unary_op(*op, &val)
        }
        Expression::IsNull(inner, negated) => {
            let val = eval(inner, record, engine, params, scalar_fns)?;
            let is_null = val == Value::Null;
            if *negated {
                Ok(Value::Bool(!is_null))
            } else {
                Ok(Value::Bool(is_null))
            }
        }
        Expression::ListLiteral(elements) => {
            let mut values = Vec::with_capacity(elements.len());
            for elem in elements {
                values.push(eval(elem, record, engine, params, scalar_fns)?);
            }
            Ok(Value::List(values))
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
                    let val = eval(&args[0], record, engine, params, scalar_fns)?;
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
                    let val = eval(&args[0], record, engine, params, scalar_fns)?;
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
                    let val = eval(&args[0], record, engine, params, scalar_fns)?;
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
                "datetime" => {
                    if args.len() != 1 {
                        return Err(ExecutionError {
                            message: "datetime() requires exactly one string argument".to_string(),
                        });
                    }
                    let val = eval(&args[0], record, engine, params, scalar_fns)?;
                    match val {
                        Value::String(s) => {
                            let millis = parse_iso8601_to_millis(&s).map_err(|e| ExecutionError {
                                message: e,
                            })?;
                            Ok(Value::DateTime(millis))
                        }
                        _ => Err(ExecutionError {
                            message: "datetime() requires a string argument".to_string(),
                        }),
                    }
                }
                "now" => {
                    if !args.is_empty() {
                        return Err(ExecutionError {
                            message: "now() takes no arguments".to_string(),
                        });
                    }
                    // Read query start time from params
                    match params.get("__query_start_ms__") {
                        Some(Value::Int64(ms)) => Ok(Value::DateTime(*ms)),
                        _ => {
                            // Fallback: use current system time
                            let ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as i64)
                                .unwrap_or(0);
                            Ok(Value::DateTime(ms))
                        }
                    }
                }
                _ => {
                    // Evaluate arguments, then try plugin scalar function lookup.
                    let evaluated_args: Result<Vec<_>, _> = args
                        .iter()
                        .map(|a| eval(a, record, engine, params, scalar_fns))
                        .collect();
                    let evaluated_args = evaluated_args?;
                    match scalar_fns.call_scalar(&func_name, &evaluated_args) {
                        Some(result) => result,
                        None => Err(ExecutionError {
                            message: format!("unknown function: {}", name),
                        }),
                    }
                }
            }
        }
        #[cfg(feature = "hypergraph")]
        Expression::TemporalRef { node, timestamp } => {
            // Evaluate both sub-expressions and return a placeholder.
            // The actual interpretation is done in the executor during hyperedge creation.
            let _node_val = eval(node, record, engine, params, scalar_fns)?;
            let _ts_val = eval(timestamp, record, engine, params, scalar_fns)?;
            // For expression evaluation context, return the node value
            // (temporal resolution happens at the hyperedge executor level).
            eval(node, record, engine, params, scalar_fns)
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
/// Access a property from temporal version properties.
/// The props_list is a List of [prop_key_id, value] pairs.
fn eval_temporal_property_access(
    props_list: &[Value],
    prop_name: &str,
    engine: &StorageEngine,
) -> Result<Value, ExecutionError> {
    let prop_key_id = engine.catalog().prop_key_id(prop_name);
    match prop_key_id {
        Some(kid) => {
            for item in props_list {
                if let Value::List(pair) = item {
                    if pair.len() == 2 {
                        if let Value::Int64(k) = &pair[0] {
                            if *k as u32 == kid {
                                return Ok(pair[1].clone());
                            }
                        }
                    }
                }
            }
            Ok(Value::Null)
        }
        None => Ok(Value::Null),
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
        #[cfg(feature = "subgraph")]
        Value::Subgraph(sg_id) => {
            // Special property: _temporal_anchor maps to SubgraphRecord.temporal_anchor
            if prop_name == "_temporal_anchor" {
                let sg = engine.get_subgraph(*sg_id).ok_or_else(|| ExecutionError {
                    message: format!("subgraph {} not found", sg_id.0),
                })?;
                return match sg.temporal_anchor {
                    Some(ms) => Ok(Value::Int64(ms)),
                    None => Ok(Value::Null),
                };
            }
            // Regular property access on SubgraphRecord.properties
            let sg = engine.get_subgraph(*sg_id).ok_or_else(|| ExecutionError {
                message: format!("subgraph {} not found", sg_id.0),
            })?;
            let prop_key_id = engine.catalog().prop_key_id(prop_name);
            match prop_key_id {
                Some(kid) => {
                    for (k, v) in &sg.properties {
                        if *k == kid {
                            return Ok(Value::from(v.clone()));
                        }
                    }
                    Ok(Value::Null)
                }
                None => Ok(Value::Null),
            }
        }
        #[cfg(feature = "hypergraph")]
        Value::Hyperedge(he_id) => {
            let he = engine.get_hyperedge(*he_id).ok_or_else(|| ExecutionError {
                message: format!("hyperedge {} not found", he_id.0),
            })?;
            let prop_key_id = engine.catalog().prop_key_id(prop_name);
            match prop_key_id {
                Some(kid) => {
                    for (k, v) in &he.properties {
                        if *k == kid {
                            return Ok(Value::from(v.clone()));
                        }
                    }
                    Ok(Value::Null)
                }
                None => Ok(Value::Null),
            }
        }
        // NN-001, NN-002: Lazy TemporalRef resolution via VersionStore.
        // When a TemporalNode's properties are accessed, resolve the node state
        // at the referenced timestamp by walking the version chain.
        #[cfg(feature = "hypergraph")]
        Value::TemporalNode(nid, timestamp) => {
            resolve_temporal_node_property(*nid, *timestamp, prop_name, engine)
        }
        Value::Null => Ok(Value::Null),
        _ => Err(ExecutionError {
            message: format!("cannot access property '{}' on non-entity value", prop_name),
        }),
    }
}

/// NN-001: Resolve a property on a node at a specific point in time.
///
/// Walk the VersionStore chain for the given node. Each version is a pre-update
/// snapshot. Find the version whose `_updated_at` is closest to but not after
/// `timestamp`. If no suitable version is found, fall back to the current node.
#[cfg(feature = "hypergraph")]
fn resolve_temporal_node_property(
    nid: cypherlite_core::NodeId,
    timestamp: i64,
    prop_name: &str,
    engine: &StorageEngine,
) -> Result<Value, ExecutionError> {
    use cypherlite_storage::version::VersionRecord;

    let updated_at_key = engine.catalog().prop_key_id("_updated_at");

    // Get version chain (oldest to newest).
    let chain = engine.version_store().get_version_chain(nid.0);

    // Find the best version: the latest version whose _updated_at <= timestamp.
    let mut best_version: Option<&cypherlite_core::NodeRecord> = None;
    for (_seq, record) in &chain {
        if let VersionRecord::Node(node_rec) = record {
            if let Some(ua_key) = updated_at_key {
                for (k, v) in &node_rec.properties {
                    if *k == ua_key {
                        let ua_ms = match v {
                            cypherlite_core::PropertyValue::DateTime(ms) => *ms,
                            cypherlite_core::PropertyValue::Int64(ms) => *ms,
                            _ => continue,
                        };
                        if ua_ms <= timestamp {
                            best_version = Some(node_rec);
                        }
                        break;
                    }
                }
            } else {
                // No _updated_at key registered; use latest version as best guess.
                best_version = Some(node_rec);
            }
        }
    }

    // Look up the property from the resolved version (or current node as fallback).
    let prop_key_id = engine.catalog().prop_key_id(prop_name);
    match prop_key_id {
        Some(kid) => {
            if let Some(node_rec) = best_version {
                // Read property from versioned node record.
                for (k, v) in &node_rec.properties {
                    if *k == kid {
                        return Ok(Value::from(v.clone()));
                    }
                }
                Ok(Value::Null)
            } else {
                // Fallback: no matching version, use current node state.
                let node = engine.get_node(nid).ok_or_else(|| ExecutionError {
                    message: format!("node {} not found", nid.0),
                })?;
                for (k, v) in &node.properties {
                    if *k == kid {
                        return Ok(Value::from(v.clone()));
                    }
                }
                Ok(Value::Null)
            }
        }
        None => Ok(Value::Null),
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
        // DateTime comparison: follows numeric ordering of underlying i64
        (Value::DateTime(a), Value::DateTime(b)) => Ok(Value::Bool(cmp_ord(a, b, op))),
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

/// Parse an ISO 8601 string to milliseconds since Unix epoch.
/// Supports: YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, YYYY-MM-DDTHH:MM:SSZ, YYYY-MM-DDTHH:MM:SS+HH:MM
fn parse_iso8601_to_millis(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.len() < 10 {
        return Err(format!("invalid datetime: '{}'", s));
    }

    // Parse date part: YYYY-MM-DD
    let year: i64 = s[0..4]
        .parse()
        .map_err(|_| format!("invalid year in '{}'", s))?;
    if s.as_bytes()[4] != b'-' {
        return Err(format!("invalid datetime: '{}'", s));
    }
    let month: u32 = s[5..7]
        .parse()
        .map_err(|_| format!("invalid month in '{}'", s))?;
    if s.as_bytes()[7] != b'-' {
        return Err(format!("invalid datetime: '{}'", s));
    }
    let day: u32 = s[8..10]
        .parse()
        .map_err(|_| format!("invalid day in '{}'", s))?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("invalid date values in '{}'", s));
    }

    let mut hour: u32 = 0;
    let mut minute: u32 = 0;
    let mut second: u32 = 0;
    let mut tz_offset_minutes: i64 = 0;

    let rest = &s[10..];
    if !rest.is_empty() {
        // Expect 'T' separator
        if rest.as_bytes()[0] != b'T' {
            return Err(format!("expected 'T' separator in '{}'", s));
        }
        let time_str = &rest[1..];
        if time_str.len() < 8 {
            return Err(format!("incomplete time in '{}'", s));
        }
        hour = time_str[0..2]
            .parse()
            .map_err(|_| format!("invalid hour in '{}'", s))?;
        if time_str.as_bytes()[2] != b':' {
            return Err(format!("invalid time format in '{}'", s));
        }
        minute = time_str[3..5]
            .parse()
            .map_err(|_| format!("invalid minute in '{}'", s))?;
        if time_str.as_bytes()[5] != b':' {
            return Err(format!("invalid time format in '{}'", s));
        }
        second = time_str[6..8]
            .parse()
            .map_err(|_| format!("invalid second in '{}'", s))?;

        // Parse timezone suffix
        let tz_part = &time_str[8..];
        if !tz_part.is_empty() {
            if tz_part == "Z" {
                // UTC
            } else if tz_part.len() == 6
                && (tz_part.as_bytes()[0] == b'+' || tz_part.as_bytes()[0] == b'-')
            {
                let sign: i64 = if tz_part.as_bytes()[0] == b'+' {
                    1
                } else {
                    -1
                };
                let tz_hour: i64 = tz_part[1..3]
                    .parse()
                    .map_err(|_| format!("invalid timezone hour in '{}'", s))?;
                let tz_min: i64 = tz_part[4..6]
                    .parse()
                    .map_err(|_| format!("invalid timezone minute in '{}'", s))?;
                tz_offset_minutes = sign * (tz_hour * 60 + tz_min);
            } else {
                return Err(format!("invalid timezone in '{}'", s));
            }
        }
    }

    // Convert to days since epoch using Howard Hinnant's algorithm
    let days = days_from_civil(year, month, day);
    let total_seconds =
        days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
        - tz_offset_minutes * 60;

    Ok(total_seconds * 1000)
}

/// Convert (year, month, day) to days since 1970-01-01.
/// Based on Howard Hinnant's `days_from_civil` algorithm.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 9 } else { month - 3 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // year of era [0, 399]
    let doy = (153 * m + 2) / 5 + day - 1; // day of year [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era [0, 146096]
    era * 146097 + doe as i64 - 719468
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
        (Value::DateTime(x), Value::DateTime(y)) => x.cmp(y),
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
            .expect_err("should error")
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
            &(),
        );
        assert_eq!(result, Ok(Value::Int64(42)));

        let result = eval(
            &Expression::Literal(Literal::Float(3.15)),
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::Float64(3.15)));

        let result = eval(
            &Expression::Literal(Literal::String("hello".into())),
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::String("hello".into())));

        let result = eval(
            &Expression::Literal(Literal::Bool(true)),
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::Bool(true)));

        let result = eval(
            &Expression::Literal(Literal::Null),
            &record,
            &engine,
            &params,
            &(),
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
            &(),
        );
        assert_eq!(result, Ok(Value::Int64(99)));

        // Missing variable returns Null
        let result = eval(
            &Expression::Variable("missing".to_string()),
            &record,
            &engine,
            &params,
            &(),
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
            &(),
        );
        assert_eq!(result, Ok(Value::String("Alice".into())));

        // Missing parameter returns Null
        let result = eval(
            &Expression::Parameter("missing".to_string()),
            &record,
            &engine,
            &params,
            &(),
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
            &(),
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
            &(),
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
            &(),
        );
        assert_eq!(result, Ok(Value::Bool(true)));

        // IS NOT NULL
        let result = eval(
            &Expression::IsNull(Box::new(Expression::Variable("y".to_string())), true),
            &record,
            &engine,
            &params,
            &(),
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
            &(),
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
            &(),
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

    // ======================================================================
    // U-002: datetime() built-in function
    // ======================================================================

    #[test]
    fn test_eval_datetime_date_only() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        // datetime('2024-01-15') -> 2024-01-15T00:00:00.000Z
        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "2024-01-15".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(1_705_276_800_000)));
    }

    #[test]
    fn test_eval_datetime_with_time() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        // datetime('2024-01-15T10:30:00')
        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "2024-01-15T10:30:00".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(1_705_276_800_000 + 10 * 3_600_000 + 30 * 60_000)));
    }

    #[test]
    fn test_eval_datetime_with_z_suffix() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "2024-01-15T10:30:00Z".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(1_705_276_800_000 + 10 * 3_600_000 + 30 * 60_000)));
    }

    #[test]
    fn test_eval_datetime_with_timezone_offset() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        // datetime('2024-01-15T10:30:00+09:00') -> UTC 01:30:00
        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "2024-01-15T10:30:00+09:00".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(1_705_276_800_000 + 3_600_000 + 30 * 60_000)));
    }

    #[test]
    fn test_eval_datetime_invalid_format() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "not-a-date".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_datetime_wrong_arg_count() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_datetime_non_string_arg() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::Integer(42))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert!(result.is_err());
    }

    // ======================================================================
    // U-003: now() function
    // ======================================================================

    #[test]
    fn test_eval_now_returns_datetime() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let mut params = Params::new();
        params.insert(
            "__query_start_ms__".to_string(),
            Value::Int64(1_700_000_000_000),
        );

        let result = eval(
            &Expression::FunctionCall {
                name: "now".to_string(),
                distinct: false,
                args: vec![],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(1_700_000_000_000)));
    }

    #[test]
    fn test_eval_now_no_args() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let mut params = Params::new();
        params.insert(
            "__query_start_ms__".to_string(),
            Value::Int64(1_700_000_000_000),
        );

        // now() with args should fail
        let result = eval(
            &Expression::FunctionCall {
                name: "now".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::Integer(1))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert!(result.is_err());
    }

    // ======================================================================
    // U-004: DateTime comparison operators
    // ======================================================================

    #[test]
    fn test_eval_cmp_datetime_eq() {
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_700_000_000_000),
                &Value::DateTime(1_700_000_000_000),
                BinaryOp::Eq
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_700_000_000_000),
                &Value::DateTime(1_700_000_000_001),
                BinaryOp::Eq
            ),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn test_eval_cmp_datetime_lt_gt() {
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_000),
                &Value::DateTime(2_000),
                BinaryOp::Lt
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(
                &Value::DateTime(2_000),
                &Value::DateTime(1_000),
                BinaryOp::Gt
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_000),
                &Value::DateTime(1_000),
                BinaryOp::Lte
            ),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_000),
                &Value::DateTime(1_000),
                BinaryOp::Gte
            ),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn test_eval_cmp_datetime_neq() {
        assert_eq!(
            eval_cmp(
                &Value::DateTime(1_000),
                &Value::DateTime(2_000),
                BinaryOp::Neq
            ),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn test_eval_cmp_datetime_vs_non_datetime_error() {
        let result = eval_cmp(
            &Value::DateTime(1_000),
            &Value::Int64(1_000),
            BinaryOp::Eq,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_cmp_datetime_vs_null() {
        assert_eq!(
            eval_cmp(&Value::DateTime(1_000), &Value::Null, BinaryOp::Eq),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn test_compare_values_datetime_ordering() {
        use std::cmp::Ordering;
        assert_eq!(
            compare_values(&Value::DateTime(1_000), &Value::DateTime(2_000)),
            Ordering::Less
        );
        assert_eq!(
            compare_values(&Value::DateTime(2_000), &Value::DateTime(1_000)),
            Ordering::Greater
        );
        assert_eq!(
            compare_values(&Value::DateTime(1_000), &Value::DateTime(1_000)),
            Ordering::Equal
        );
    }

    // U-005: DateTime epoch
    #[test]
    fn test_eval_datetime_epoch() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());
        let record = Record::new();
        let params = Params::new();

        let result = eval(
            &Expression::FunctionCall {
                name: "datetime".to_string(),
                distinct: false,
                args: vec![Expression::Literal(Literal::String(
                    "1970-01-01".to_string(),
                ))],
            },
            &record,
            &engine,
            &params,
            &(),
        );
        assert_eq!(result, Ok(Value::DateTime(0)));
    }

    // ── Hypergraph property access tests ───────────────────────────────
    #[cfg(feature = "hypergraph")]
    mod hyperedge_property_tests {
        use super::*;

        #[test]
        fn test_property_access_on_hyperedge() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let rel_type = engine.get_or_create_rel_type("INVOLVES");
            let prop_key = engine.get_or_create_prop_key("weight");

            use cypherlite_core::{GraphEntity, PropertyValue};
            let n1 = engine.create_node(vec![], vec![]);
            let he_id = engine.create_hyperedge(
                rel_type,
                vec![GraphEntity::Node(n1)],
                vec![],
                vec![(prop_key, PropertyValue::Int64(42))],
            );

            let mut record = Record::new();
            record.insert("he".to_string(), Value::Hyperedge(he_id));

            let expr = Expression::Property(
                Box::new(Expression::Variable("he".to_string())),
                "weight".to_string(),
            );
            let result = eval(&expr, &record, &engine, &Params::new(), &());
            assert_eq!(result, Ok(Value::Int64(42)));
        }

        #[test]
        fn test_property_access_on_hyperedge_missing_prop() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let rel_type = engine.get_or_create_rel_type("INVOLVES");

            let he_id = engine.create_hyperedge(rel_type, vec![], vec![], vec![]);

            let mut record = Record::new();
            record.insert("he".to_string(), Value::Hyperedge(he_id));

            let expr = Expression::Property(
                Box::new(Expression::Variable("he".to_string())),
                "nonexistent".to_string(),
            );
            let result = eval(&expr, &record, &engine, &Params::new(), &());
            assert_eq!(result, Ok(Value::Null));
        }

        #[test]
        fn test_property_access_on_hyperedge_not_found() {
            let dir = tempdir().expect("tempdir");
            let engine = test_engine(dir.path());

            let fake_id = cypherlite_core::HyperEdgeId(999);
            let mut record = Record::new();
            record.insert("he".to_string(), Value::Hyperedge(fake_id));

            let expr = Expression::Property(
                Box::new(Expression::Variable("he".to_string())),
                "weight".to_string(),
            );
            let result = eval(&expr, &record, &engine, &Params::new(), &());
            assert!(result.is_err());
        }

        // NN-001: TemporalNode property access falls back to current node
        // when no versions exist.
        #[test]
        fn test_temporal_node_no_versions_falls_back_to_current() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let name_key = engine.get_or_create_prop_key("name");
            let nid = engine.create_node(
                vec![],
                vec![(name_key, cypherlite_core::PropertyValue::String("Alice".into()))],
            );

            let mut record = Record::new();
            record.insert("n".to_string(), Value::TemporalNode(nid, 999_999));
            let params = Params::new();

            let result = eval(
                &Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                ),
                &record,
                &engine,
                &params,
            &(),
            );
            assert_eq!(result, Ok(Value::String("Alice".into())));
        }

        // NN-001: TemporalNode property access resolves from VersionStore
        // when versions exist.
        #[test]
        fn test_temporal_node_resolves_versioned_properties() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let name_key = engine.get_or_create_prop_key("name");
            let updated_at_key = engine.get_or_create_prop_key("_updated_at");

            // Create node with name='Alice' and _updated_at=100
            let nid = engine.create_node(
                vec![],
                vec![
                    (name_key, cypherlite_core::PropertyValue::String("Alice".into())),
                    (updated_at_key, cypherlite_core::PropertyValue::DateTime(100)),
                ],
            );

            // Update node to name='Bob' and _updated_at=200
            // This should snapshot the old state (Alice, _updated_at=100)
            engine
                .update_node(
                    nid,
                    vec![
                        (name_key, cypherlite_core::PropertyValue::String("Bob".into())),
                        (updated_at_key, cypherlite_core::PropertyValue::DateTime(200)),
                    ],
                )
                .expect("update");

            // TemporalNode with timestamp=150 should resolve to the version
            // where _updated_at=100 (Alice), because 100 <= 150.
            let mut record = Record::new();
            record.insert("n".to_string(), Value::TemporalNode(nid, 150));
            let params = Params::new();

            let result = eval(
                &Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                ),
                &record,
                &engine,
                &params,
            &(),
            );
            assert_eq!(result, Ok(Value::String("Alice".into())));
        }

        // NN-002: TemporalNode resolves to latest matching version
        // when multiple versions exist.
        #[test]
        fn test_temporal_node_multiple_versions_picks_latest_match() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let name_key = engine.get_or_create_prop_key("name");
            let updated_at_key = engine.get_or_create_prop_key("_updated_at");

            // Create: name='v1', _updated_at=100
            let nid = engine.create_node(
                vec![],
                vec![
                    (name_key, cypherlite_core::PropertyValue::String("v1".into())),
                    (updated_at_key, cypherlite_core::PropertyValue::DateTime(100)),
                ],
            );

            // Update to v2 at time 200 (snapshots v1)
            engine
                .update_node(
                    nid,
                    vec![
                        (name_key, cypherlite_core::PropertyValue::String("v2".into())),
                        (updated_at_key, cypherlite_core::PropertyValue::DateTime(200)),
                    ],
                )
                .expect("update 1");

            // Update to v3 at time 300 (snapshots v2)
            engine
                .update_node(
                    nid,
                    vec![
                        (name_key, cypherlite_core::PropertyValue::String("v3".into())),
                        (updated_at_key, cypherlite_core::PropertyValue::DateTime(300)),
                    ],
                )
                .expect("update 2");

            // At timestamp 250: should resolve to v2 (version with _updated_at=200)
            let mut record = Record::new();
            record.insert("n".to_string(), Value::TemporalNode(nid, 250));
            let params = Params::new();

            let result = eval(
                &Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                ),
                &record,
                &engine,
                &params,
            &(),
            );
            assert_eq!(result, Ok(Value::String("v2".into())));
        }

        // NN-002: TemporalNode with timestamp before all versions
        // falls back to current node.
        #[test]
        fn test_temporal_node_timestamp_before_all_versions() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let name_key = engine.get_or_create_prop_key("name");
            let updated_at_key = engine.get_or_create_prop_key("_updated_at");

            // Create: name='Alice', _updated_at=200
            let nid = engine.create_node(
                vec![],
                vec![
                    (name_key, cypherlite_core::PropertyValue::String("Alice".into())),
                    (updated_at_key, cypherlite_core::PropertyValue::DateTime(200)),
                ],
            );

            // Update to name='Bob', _updated_at=300 (snapshots Alice at 200)
            engine
                .update_node(
                    nid,
                    vec![
                        (name_key, cypherlite_core::PropertyValue::String("Bob".into())),
                        (updated_at_key, cypherlite_core::PropertyValue::DateTime(300)),
                    ],
                )
                .expect("update");

            // Timestamp=50 is before the earliest version (_updated_at=200).
            // No version has _updated_at <= 50, so falls back to current node (Bob).
            let mut record = Record::new();
            record.insert("n".to_string(), Value::TemporalNode(nid, 50));
            let params = Params::new();

            let result = eval(
                &Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "name".to_string(),
                ),
                &record,
                &engine,
                &params,
            &(),
            );
            assert_eq!(result, Ok(Value::String("Bob".into())));
        }

        // NN-003: TemporalNode non-existent property returns Null.
        #[test]
        fn test_temporal_node_nonexistent_property() {
            let dir = tempdir().expect("tempdir");
            let mut engine = test_engine(dir.path());

            let name_key = engine.get_or_create_prop_key("name");
            let nid = engine.create_node(
                vec![],
                vec![(name_key, cypherlite_core::PropertyValue::String("Alice".into()))],
            );

            let mut record = Record::new();
            record.insert("n".to_string(), Value::TemporalNode(nid, 999));
            let params = Params::new();

            let result = eval(
                &Expression::Property(
                    Box::new(Expression::Variable("n".to_string())),
                    "nonexistent".to_string(),
                ),
                &record,
                &engine,
                &params,
            &(),
            );
            assert_eq!(result, Ok(Value::Null));
        }
    }
}
