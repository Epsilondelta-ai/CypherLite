// FilterOp: applies predicate via eval()

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup, Value};
use crate::parser::ast::Expression;
use cypherlite_storage::StorageEngine;

/// Filter records by evaluating a predicate expression.
/// Only records where predicate evaluates to Value::Bool(true) are kept.
pub fn execute_filter(
    source_records: Vec<Record>,
    predicate: &Expression,
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in source_records {
        let val = eval(predicate, &record, engine, params, scalar_fns)?;
        if val == Value::Bool(true) {
            results.push(record);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Record;
    use crate::parser::ast::*;
    use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
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

    // EXEC-T003: FilterOp with inequality
    #[test]
    fn test_filter_with_inequality() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let age_key = engine.get_or_create_prop_key("age");
        let n1 = engine.create_node(
            vec![],
            vec![(age_key, cypherlite_core::PropertyValue::Int64(25))],
        );
        let n2 = engine.create_node(
            vec![],
            vec![(age_key, cypherlite_core::PropertyValue::Int64(35))],
        );
        let n3 = engine.create_node(
            vec![],
            vec![(age_key, cypherlite_core::PropertyValue::Int64(45))],
        );

        let mut r1 = Record::new();
        r1.insert("n".to_string(), Value::Node(n1));
        let mut r2 = Record::new();
        r2.insert("n".to_string(), Value::Node(n2));
        let mut r3 = Record::new();
        r3.insert("n".to_string(), Value::Node(n3));

        // n.age > 30
        let predicate = Expression::BinaryOp(
            BinaryOp::Gt,
            Box::new(Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "age".to_string(),
            )),
            Box::new(Expression::Literal(Literal::Integer(30))),
        );

        let params = Params::new();
        let result = execute_filter(vec![r1, r2, r3], &predicate, &engine, &params, &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_filter_keeps_matching_only() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::Int64(10));
        let mut r2 = Record::new();
        r2.insert("x".to_string(), Value::Int64(20));

        // x = 10
        let predicate = Expression::BinaryOp(
            BinaryOp::Eq,
            Box::new(Expression::Variable("x".to_string())),
            Box::new(Expression::Literal(Literal::Integer(10))),
        );

        let params = Params::new();
        let result = execute_filter(vec![r1, r2], &predicate, &engine, &params, &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("x"), Some(&Value::Int64(10)));
    }

    #[test]
    fn test_filter_null_predicate_excluded() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::Null);

        // x = 1 (Null = 1 -> false due to null semantics)
        let predicate = Expression::BinaryOp(
            BinaryOp::Eq,
            Box::new(Expression::Variable("x".to_string())),
            Box::new(Expression::Literal(Literal::Integer(1))),
        );

        let params = Params::new();
        let result = execute_filter(vec![r1], &predicate, &engine, &params, &());
        assert!(result.expect("should succeed").is_empty());
    }
}
