// ProjectOp: evaluates RETURN expressions, applies column aliases

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, ScalarFnLookup};
use crate::parser::ast::{Expression, ReturnItem};
use cypherlite_storage::StorageEngine;

/// Project specific expressions from source records.
/// Each ReturnItem is evaluated and given a column name (alias or expression text).
pub fn execute_project(
    source_records: Vec<Record>,
    items: &[ReturnItem],
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Result<Vec<Record>, ExecutionError> {
    let mut results = Vec::new();

    for record in &source_records {
        let mut projected = Record::new();

        for item in items {
            let value = eval(&item.expr, record, engine, params, scalar_fns)?;
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
///
/// For property access (e.g., `n.name`), returns "n.name" instead of "expr".
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
    use cypherlite_core::{DatabaseConfig, LabelRegistry, NodeId, SyncMode};
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

    // EXEC-T004: ProjectOp column rename (RETURN AS alias)
    #[test]
    fn test_project_with_alias() {
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

        let items = vec![ReturnItem {
            expr: Expression::Property(
                Box::new(Expression::Variable("n".to_string())),
                "name".to_string(),
            ),
            alias: Some("person_name".to_string()),
        }];

        let params = Params::new();
        let result = execute_project(vec![record], &items, &engine, &params, &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].get("person_name"),
            Some(&Value::String("Alice".into()))
        );
    }

    #[test]
    fn test_project_without_alias_uses_variable_name() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(NodeId(1)));

        let items = vec![ReturnItem {
            expr: Expression::Variable("n".to_string()),
            alias: None,
        }];

        let params = Params::new();
        let result = execute_project(vec![record], &items, &engine, &params, &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert!(records[0].contains_key("n"));
    }

    #[test]
    fn test_project_multiple_columns() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("x".to_string(), Value::Int64(1));
        record.insert("y".to_string(), Value::Int64(2));

        let items = vec![
            ReturnItem {
                expr: Expression::Variable("x".to_string()),
                alias: None,
            },
            ReturnItem {
                expr: Expression::Variable("y".to_string()),
                alias: Some("val".to_string()),
            },
        ];

        let params = Params::new();
        let result = execute_project(vec![record], &items, &engine, &params, &());
        let records = result.expect("should succeed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].get("x"), Some(&Value::Int64(1)));
        assert_eq!(records[0].get("val"), Some(&Value::Int64(2)));
    }
}
