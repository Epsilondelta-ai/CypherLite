// Temporal scan operators: AsOfScan and TemporalRangeScan
//
// X-T6: AsOfScan -- find node versions at a specific point in time
// Y-T3: TemporalRangeScan -- find all versions within a time range

use crate::executor::eval::eval;
use crate::executor::operators::create::{SYSTEM_PROP_CREATED_AT, SYSTEM_PROP_UPDATED_AT};
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup, Value};
use crate::parser::ast::Expression;
use cypherlite_core::{LabelRegistry, NodeId, PropertyValue};
use cypherlite_storage::version::VersionRecord;
use cypherlite_storage::StorageEngine;

/// Execute an AsOfScan: for each node in source records, find the version
/// that was current at the specified timestamp.
///
/// Algorithm:
/// 1. Evaluate timestamp expression to get target time T
/// 2. For each record, find node variables and look up their state at time T
/// 3. A node's state at time T is determined by:
///    - Collect all states: version snapshots + current state
///    - Each state has an _updated_at timestamp (or _created_at if never updated)
///    - Find the state with the largest _updated_at <= T
///    - If no state has _updated_at <= T, the node is excluded
pub fn execute_as_of_scan(
    source_records: Vec<Record>,
    timestamp_expr: &Expression,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    // Evaluate the timestamp expression using an empty record for context
    let empty_record = Record::new();
    let target_val = eval(timestamp_expr, &empty_record, engine, params, scalar_fns)?;
    let target_ms = match target_val {
        Value::DateTime(ms) => ms,
        Value::Int64(ms) => ms,
        _ => {
            return Err(ExecutionError {
                message: "AT TIME expression must evaluate to a DateTime or integer timestamp"
                    .to_string(),
            });
        }
    };

    let updated_key = engine.get_or_create_prop_key(SYSTEM_PROP_UPDATED_AT);
    let created_key = engine.get_or_create_prop_key(SYSTEM_PROP_CREATED_AT);

    let mut results = Vec::new();

    for record in &source_records {
        // Find node variables in the record and apply temporal lookup
        let mut temporal_record = record.clone();
        let mut include = true;

        for (var_name, value) in record {
            if let Value::Node(node_id) = value {
                match find_node_state_at(*node_id, target_ms, updated_key, created_key, engine) {
                    Some(TemporalNodeState::Current) => {
                        // Current state is valid at target time; keep the record as-is
                    }
                    Some(TemporalNodeState::Version(props)) => {
                        // Replace the node's record with versioned properties
                        // We keep the node ID but mark it with versioned properties
                        // by injecting a special marker. For property access to work,
                        // we store versioned properties so eval can pick them up.
                        temporal_record.insert(
                            format!("__temporal_props__{}", var_name),
                            Value::List(
                                props
                                    .iter()
                                    .map(|(k, v)| {
                                        Value::List(vec![
                                            Value::Int64(*k as i64),
                                            Value::from(v.clone()),
                                        ])
                                    })
                                    .collect(),
                            ),
                        );
                    }
                    None => {
                        // Node did not exist at target time; exclude from results
                        include = false;
                        break;
                    }
                }
            }
        }

        if include {
            results.push(temporal_record);
        }
    }

    Ok(results)
}

/// Execute a TemporalRangeScan: for each node in source records, find all
/// versions within the specified time range.
///
/// Each version becomes a separate row in the result set.
pub fn execute_temporal_range_scan(
    source_records: Vec<Record>,
    start_expr: &Expression,
    end_expr: &Expression,
    engine: &mut StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    let empty_record = Record::new();
    let start_val = eval(start_expr, &empty_record, engine, params, scalar_fns)?;
    let end_val = eval(end_expr, &empty_record, engine, params, scalar_fns)?;

    let start_ms = match start_val {
        Value::DateTime(ms) => ms,
        Value::Int64(ms) => ms,
        _ => {
            return Err(ExecutionError {
                message: "BETWEEN TIME start must evaluate to a DateTime or integer".to_string(),
            });
        }
    };

    let end_ms = match end_val {
        Value::DateTime(ms) => ms,
        Value::Int64(ms) => ms,
        _ => {
            return Err(ExecutionError {
                message: "BETWEEN TIME end must evaluate to a DateTime or integer".to_string(),
            });
        }
    };

    let updated_key = engine.get_or_create_prop_key(SYSTEM_PROP_UPDATED_AT);
    let created_key = engine.get_or_create_prop_key(SYSTEM_PROP_CREATED_AT);

    let mut results = Vec::new();

    for record in &source_records {
        // Find node variables in the record and collect all versions in range
        for (var_name, value) in record {
            if let Value::Node(node_id) = value {
                let versions_in_range = find_node_versions_in_range(
                    *node_id,
                    start_ms,
                    end_ms,
                    updated_key,
                    created_key,
                    engine,
                );

                for props in versions_in_range {
                    let mut versioned_record = record.clone();
                    versioned_record.insert(
                        format!("__temporal_props__{}", var_name),
                        Value::List(
                            props
                                .iter()
                                .map(|(k, v)| {
                                    Value::List(vec![
                                        Value::Int64(*k as i64),
                                        Value::from(v.clone()),
                                    ])
                                })
                                .collect(),
                        ),
                    );
                    results.push(versioned_record);
                }
                // Only handle first node variable for simplicity
                break;
            }
        }
    }

    Ok(results)
}

/// Result of looking up a node's state at a specific time.
enum TemporalNodeState {
    /// The current (live) state is valid at the target time.
    Current,
    /// A historical version's properties should be used.
    Version(Vec<(u32, PropertyValue)>),
}

/// Find the node state at a specific timestamp.
///
/// Returns None if the node did not exist at that time.
fn find_node_state_at(
    node_id: NodeId,
    target_ms: i64,
    updated_key: u32,
    created_key: u32,
    engine: &StorageEngine,
) -> Option<TemporalNodeState> {
    let current = engine.get_node(node_id)?;

    // Get current node's _created_at
    let current_created = get_timestamp_prop(&current.properties, created_key)?;

    // If node was created after target time, it didn't exist then
    if current_created > target_ms {
        return None;
    }

    // Get current node's _updated_at
    let current_updated =
        get_timestamp_prop(&current.properties, updated_key).unwrap_or(current_created);

    // If current state's _updated_at <= target_ms, current state is valid
    if current_updated <= target_ms {
        return Some(TemporalNodeState::Current);
    }

    // Current state was updated after target time; check version chain
    let version_chain = engine.version_store().get_version_chain(node_id.0);

    // Iterate from newest to oldest to find the latest version valid at target time
    for (_seq, version) in version_chain.iter().rev() {
        if let VersionRecord::Node(node_record) = version {
            let version_updated = get_timestamp_prop(&node_record.properties, updated_key)
                .or_else(|| get_timestamp_prop(&node_record.properties, created_key));

            if let Some(ts) = version_updated {
                if ts <= target_ms {
                    return Some(TemporalNodeState::Version(node_record.properties.clone()));
                }
            }
        }
    }

    // No version found at target time; check if the earliest version's _created_at <= target
    // This handles the case where the node's first state is in the version chain
    if let Some((_seq, VersionRecord::Node(node_record))) = version_chain.first() {
        let version_created = get_timestamp_prop(&node_record.properties, created_key);
        if let Some(ts) = version_created {
            if ts <= target_ms {
                return Some(TemporalNodeState::Version(node_record.properties.clone()));
            }
        }
    }

    None
}

/// Find all node versions within a time range [start_ms, end_ms].
fn find_node_versions_in_range(
    node_id: NodeId,
    start_ms: i64,
    end_ms: i64,
    updated_key: u32,
    created_key: u32,
    engine: &StorageEngine,
) -> Vec<Vec<(u32, PropertyValue)>> {
    let mut results = Vec::new();

    // Check version chain
    let version_chain = engine.version_store().get_version_chain(node_id.0);

    for (_seq, version) in &version_chain {
        if let VersionRecord::Node(node_record) = version {
            let ts = get_timestamp_prop(&node_record.properties, updated_key)
                .or_else(|| get_timestamp_prop(&node_record.properties, created_key));

            if let Some(ts) = ts {
                if ts >= start_ms && ts <= end_ms {
                    results.push(node_record.properties.clone());
                }
            }
        }
    }

    // Also check current state
    if let Some(current) = engine.get_node(node_id) {
        let ts = get_timestamp_prop(&current.properties, updated_key)
            .or_else(|| get_timestamp_prop(&current.properties, created_key));

        if let Some(ts) = ts {
            if ts >= start_ms && ts <= end_ms {
                results.push(current.properties.clone());
            }
        }
    }

    results
}

/// Extract a DateTime timestamp from a property list by key.
fn get_timestamp_prop(props: &[(u32, PropertyValue)], key: u32) -> Option<i64> {
    props.iter().find(|(k, _)| *k == key).and_then(|(_, v)| {
        if let PropertyValue::DateTime(ms) = v {
            Some(*ms)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_timestamp_prop_found() {
        let props = vec![
            (1, PropertyValue::String("hello".to_string())),
            (2, PropertyValue::DateTime(1000)),
        ];
        assert_eq!(get_timestamp_prop(&props, 2), Some(1000));
    }

    #[test]
    fn test_get_timestamp_prop_not_found() {
        let props = vec![(1, PropertyValue::String("hello".to_string()))];
        assert_eq!(get_timestamp_prop(&props, 2), None);
    }

    #[test]
    fn test_get_timestamp_prop_wrong_type() {
        let props = vec![(2, PropertyValue::Int64(1000))];
        assert_eq!(get_timestamp_prop(&props, 2), None);
    }
}
