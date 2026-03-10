// WithOp: intermediate projection for WITH clause (scope reset)

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record};
use crate::parser::ast::{Expression, ReturnItem};
use cypherlite_storage::StorageEngine;

/// Execute WITH projection: evaluate each ReturnItem and build new records
/// with only the projected columns. This is similar to ProjectOp but serves
/// as an intermediate step rather than final output.
pub fn execute_with(
    source_records: Vec<Record>,
    items: &[ReturnItem],
    engine: &StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in &source_records {
        let mut projected = Record::new();

        for item in items {
            let value = eval(&item.expr, record, engine, params)?;
            let column_name = match &item.alias {
                Some(alias) => alias.clone(),
                None => expr_display_name(&item.expr),
            };
            projected.insert(column_name, value);
        }

        results.push(projected);
    }

    Ok(results)
}

/// Generate a display name for an expression (used when no alias is provided).
fn expr_display_name(expr: &Expression) -> String {
    match expr {
        Expression::Variable(name) => name.clone(),
        Expression::Property(inner, prop) => {
            format!("{}.{}", expr_display_name(inner), prop)
        }
        Expression::CountStar => "count(*)".to_string(),
        _ => "expr".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Value;
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

    // TASK-062: WithOp projects only specified columns
    #[test]
    fn test_with_projects_specified_columns() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("x".to_string(), Value::Int64(1));
        record.insert("y".to_string(), Value::Int64(2));
        record.insert("z".to_string(), Value::Int64(3));

        // WITH x -- only 'x' survives
        let items = vec![ReturnItem {
            expr: Expression::Variable("x".to_string()),
            alias: None,
        }];

        let params = Params::new();
        let result = execute_with(vec![record], &items, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("x"), Some(&Value::Int64(1)));
        assert!(!records[0].contains_key("y"));
        assert!(!records[0].contains_key("z"));
    }

    // TASK-062: WITH with alias
    #[test]
    fn test_with_alias_renames_column() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

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

        // WITH n.name AS person_name
        let items = vec![ReturnItem {
            expr: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "name".to_string(),
            ),
            alias: Some("person_name".to_string()),
        }];

        let params = Params::new();
        let result = execute_with(vec![record], &items, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].get("person_name"),
            Some(&Value::String("Alice".into()))
        );
        assert!(!records[0].contains_key("n"));
    }

    // TASK-062: WITH multiple items
    #[test]
    fn test_with_multiple_items() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("a".to_string(), Value::Int64(10));
        record.insert("b".to_string(), Value::Int64(20));
        record.insert("c".to_string(), Value::Int64(30));

        // WITH a, b
        let items = vec![
            ReturnItem {
                expr: Expression::Variable("a".to_string()),
                alias: None,
            },
            ReturnItem {
                expr: Expression::Variable("b".to_string()),
                alias: None,
            },
        ];

        let params = Params::new();
        let result = execute_with(vec![record], &items, &engine, &params);
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("a"), Some(&Value::Int64(10)));
        assert_eq!(records[0].get("b"), Some(&Value::Int64(20)));
        assert!(!records[0].contains_key("c"));
    }

    // TASK-064: WITH DISTINCT deduplication (tested at higher level via executor dispatch)
    #[test]
    fn test_with_produces_duplicates_for_distinct_to_handle() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::String("A".into()));
        r1.insert("y".to_string(), Value::Int64(1));
        let mut r2 = Record::new();
        r2.insert("x".to_string(), Value::String("A".into()));
        r2.insert("y".to_string(), Value::Int64(2));

        // WITH x -- both records have x="A", so after projection they are duplicates
        let items = vec![ReturnItem {
            expr: Expression::Variable("x".to_string()),
            alias: None,
        }];

        let params = Params::new();
        let result = execute_with(vec![r1, r2], &items, &engine, &params);
        let records = result.expect("should succeed");
        // Without DISTINCT, duplicates are preserved
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].get("x"), Some(&Value::String("A".into())));
        assert_eq!(records[1].get("x"), Some(&Value::String("A".into())));
    }

    // TASK-062: empty input produces empty output
    #[test]
    fn test_with_empty_input() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let items = vec![ReturnItem {
            expr: Expression::Variable("x".to_string()),
            alias: None,
        }];

        let params = Params::new();
        let result = execute_with(vec![], &items, &engine, &params);
        let records = result.expect("should succeed");
        assert!(records.is_empty());
    }
}
