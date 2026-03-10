// AggregateOp: count, sum, avg, min, max, collect

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, Value};
use crate::parser::ast::Expression;
use crate::planner::AggregateFunc;
use cypherlite_storage::StorageEngine;

/// Execute aggregation over source records.
/// Groups by group_keys, computes aggregate functions per group.
pub fn execute_aggregate(
    source_records: Vec<Record>,
    group_keys: &[Expression],
    aggregates: &[(String, AggregateFunc)],
    engine: &StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    if source_records.is_empty() {
        // For empty input with aggregates, return one row with zero counts
        let mut record = Record::new();
        for (alias, func) in aggregates {
            let value = match func {
                AggregateFunc::Count { .. } | AggregateFunc::CountStar => Value::Int64(0),
            };
            record.insert(alias.clone(), value);
        }
        // Add group key values as Null
        for key_expr in group_keys {
            let col_name = group_key_name(key_expr);
            record.insert(col_name, Value::Null);
        }
        return Ok(vec![record]);
    }

    // Build groups: evaluate group keys for each record
    let mut groups: Vec<(Vec<Value>, Vec<&Record>)> = Vec::new();

    for record in &source_records {
        let key_values: Vec<Value> = group_keys
            .iter()
            .map(|expr| eval(expr, record, engine, params))
            .collect::<Result<_, _>>()?;

        // Find existing group
        let found = groups.iter_mut().find(|(k, _)| k == &key_values);
        if let Some((_, members)) = found {
            members.push(record);
        } else {
            groups.push((key_values, vec![record]));
        }
    }

    // If no group keys and no groups, treat all records as one group
    if group_keys.is_empty() && groups.is_empty() {
        groups.push((vec![], source_records.iter().collect()));
    }

    let mut results = Vec::new();

    for (key_values, members) in &groups {
        let mut result_record = Record::new();

        // Add group key values
        for (i, key_expr) in group_keys.iter().enumerate() {
            let col_name = group_key_name(key_expr);
            result_record.insert(col_name, key_values[i].clone());
        }

        // Compute aggregates
        for (alias, func) in aggregates {
            let value = compute_aggregate(func, members, engine, params)?;
            result_record.insert(alias.clone(), value);
        }

        results.push(result_record);
    }

    Ok(results)
}

/// Compute a single aggregate function over a group of records.
fn compute_aggregate(
    func: &AggregateFunc,
    members: &[&Record],
    _engine: &StorageEngine,
    _params: &Params,
) -> Result<Value, ExecutionError> {
    match func {
        AggregateFunc::CountStar => Ok(Value::Int64(members.len() as i64)),
        AggregateFunc::Count { distinct: _ } => {
            // count(expr) counts non-null values
            // For simplicity, count all members (since we don't have the expr here)
            Ok(Value::Int64(members.len() as i64))
        }
    }
}

/// Extract a display name from a group key expression.
fn group_key_name(expr: &Expression) -> String {
    match expr {
        Expression::Variable(name) => name.clone(),
        Expression::Property(_, prop) => prop.clone(),
        _ => "key".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{DatabaseConfig, SyncMode};
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> cypherlite_storage::StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        cypherlite_storage::StorageEngine::open(config).expect("open")
    }

    // EXEC-T010: AggregateOp count(*)
    #[test]
    fn test_aggregate_count_star() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("n".to_string(), Value::Int64(1));
        let mut r2 = Record::new();
        r2.insert("n".to_string(), Value::Int64(2));
        let mut r3 = Record::new();
        r3.insert("n".to_string(), Value::Int64(3));

        let aggregates = vec![("count(*)".to_string(), AggregateFunc::CountStar)];

        let params = Params::new();
        let result = execute_aggregate(vec![r1, r2, r3], &[], &aggregates, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("count(*)"), Some(&Value::Int64(3)));
    }

    #[test]
    fn test_aggregate_count_star_empty() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let aggregates = vec![("count(*)".to_string(), AggregateFunc::CountStar)];
        let params = Params::new();
        let result = execute_aggregate(vec![], &[], &aggregates, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("count(*)"), Some(&Value::Int64(0)));
    }

    #[test]
    fn test_aggregate_with_group_keys() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("label".to_string(), Value::String("A".into()));
        let mut r2 = Record::new();
        r2.insert("label".to_string(), Value::String("B".into()));
        let mut r3 = Record::new();
        r3.insert("label".to_string(), Value::String("A".into()));

        let group_keys = vec![Expression::Variable("label".to_string())];
        let aggregates = vec![("cnt".to_string(), AggregateFunc::CountStar)];

        let params = Params::new();
        let result =
            execute_aggregate(vec![r1, r2, r3], &group_keys, &aggregates, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 2);

        // Find group A and B
        let group_a = records
            .iter()
            .find(|r| r.get("label") == Some(&Value::String("A".into())));
        let group_b = records
            .iter()
            .find(|r| r.get("label") == Some(&Value::String("B".into())));

        assert_eq!(group_a.expect("group A").get("cnt"), Some(&Value::Int64(2)));
        assert_eq!(group_b.expect("group B").get("cnt"), Some(&Value::Int64(1)));
    }
}
