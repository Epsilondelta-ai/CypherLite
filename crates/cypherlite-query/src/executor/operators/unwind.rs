// UnwindOp: flatten a list expression into individual rows

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, Value};
use crate::parser::ast::Expression;
use cypherlite_storage::StorageEngine;

/// Execute UNWIND: for each source record, evaluate the expression.
/// If the result is a List, emit one row per element with the variable bound.
/// If the result is Null, emit zero rows (skip the source record).
/// If the result is not a List and not Null, return an error.
pub fn execute_unwind(
    source_records: Vec<Record>,
    expr: &Expression,
    variable: &str,
    engine: &StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in &source_records {
        let value = eval(expr, record, engine, params)?;
        match value {
            Value::List(elements) => {
                for element in elements {
                    let mut new_record = record.clone();
                    new_record.insert(variable.to_string(), element);
                    results.push(new_record);
                }
            }
            Value::Null => {
                // UNWIND NULL produces zero rows -- skip this source record.
            }
            _ => {
                return Err(ExecutionError {
                    message: format!(
                        "UNWIND expected a list or null, got {:?}",
                        value
                    ),
                });
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Value;
    use crate::parser::ast::*;
    use cypherlite_core::{DatabaseConfig, SyncMode};
    use cypherlite_storage::StorageEngine;
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // TASK-071: Basic UNWIND list produces one row per element
    #[test]
    fn test_unwind_list_produces_rows() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("data".to_string(), Value::Int64(42));

        let expr = Expression::ListLiteral(vec![
            Expression::Literal(Literal::Integer(1)),
            Expression::Literal(Literal::Integer(2)),
            Expression::Literal(Literal::Integer(3)),
        ]);

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].get("x"), Some(&Value::Int64(1)));
        assert_eq!(records[1].get("x"), Some(&Value::Int64(2)));
        assert_eq!(records[2].get("x"), Some(&Value::Int64(3)));
        // Source columns preserved
        assert_eq!(records[0].get("data"), Some(&Value::Int64(42)));
        assert_eq!(records[1].get("data"), Some(&Value::Int64(42)));
        assert_eq!(records[2].get("data"), Some(&Value::Int64(42)));
    }

    // TASK-071: UNWIND with multiple source records
    #[test]
    fn test_unwind_multiple_source_records() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("name".to_string(), Value::String("Alice".into()));

        let mut r2 = Record::new();
        r2.insert("name".to_string(), Value::String("Bob".into()));

        let expr = Expression::ListLiteral(vec![
            Expression::Literal(Literal::Integer(1)),
            Expression::Literal(Literal::Integer(2)),
        ]);

        let params = Params::new();
        let result = execute_unwind(vec![r1, r2], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        // 2 source records x 2 list elements = 4 output records
        assert_eq!(records.len(), 4);
        assert_eq!(records[0].get("name"), Some(&Value::String("Alice".into())));
        assert_eq!(records[0].get("x"), Some(&Value::Int64(1)));
        assert_eq!(records[1].get("name"), Some(&Value::String("Alice".into())));
        assert_eq!(records[1].get("x"), Some(&Value::Int64(2)));
        assert_eq!(records[2].get("name"), Some(&Value::String("Bob".into())));
        assert_eq!(records[2].get("x"), Some(&Value::Int64(1)));
        assert_eq!(records[3].get("name"), Some(&Value::String("Bob".into())));
        assert_eq!(records[3].get("x"), Some(&Value::Int64(2)));
    }

    // TASK-072: UNWIND empty list -> produces zero rows
    #[test]
    fn test_unwind_empty_list_produces_zero_rows() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("data".to_string(), Value::Int64(42));

        let expr = Expression::ListLiteral(vec![]);

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        assert!(records.is_empty());
    }

    // TASK-072: UNWIND NULL -> produces zero rows
    #[test]
    fn test_unwind_null_produces_zero_rows() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("data".to_string(), Value::Int64(42));

        let expr = Expression::Literal(Literal::Null);

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        assert!(records.is_empty());
    }

    // TASK-072: UNWIND non-list value -> ExecutionError
    #[test]
    fn test_unwind_non_list_returns_error() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let record = Record::new();
        let expr = Expression::Literal(Literal::Integer(42));

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("expected a list or null"),
            "expected list error, got: {}",
            err.message
        );
    }

    // TASK-072: UNWIND non-list string -> ExecutionError
    #[test]
    fn test_unwind_string_returns_error() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let record = Record::new();
        let expr = Expression::Literal(Literal::String("not a list".into()));

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);

        assert!(result.is_err());
    }

    // TASK-071: UNWIND with empty source records
    #[test]
    fn test_unwind_empty_source() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let expr = Expression::ListLiteral(vec![
            Expression::Literal(Literal::Integer(1)),
        ]);

        let params = Params::new();
        let result = execute_unwind(vec![], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        assert!(records.is_empty());
    }

    // TASK-071: UNWIND variable referencing a list in record
    #[test]
    fn test_unwind_variable_reference() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert(
            "items".to_string(),
            Value::List(vec![
                Value::String("a".into()),
                Value::String("b".into()),
            ]),
        );

        let expr = Expression::Variable("items".to_string());

        let params = Params::new();
        let result = execute_unwind(vec![record], &expr, "x", &engine, &params);
        let records = result.expect("should succeed");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].get("x"), Some(&Value::String("a".into())));
        assert_eq!(records[1].get("x"), Some(&Value::String("b".into())));
    }
}
