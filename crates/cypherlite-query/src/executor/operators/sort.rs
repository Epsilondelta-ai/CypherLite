// SortOp: full-materialization sort with max_sort_rows guard

use crate::executor::eval::{compare_values, eval};
use crate::executor::{Params, Record, ScalarFnLookup};
use crate::parser::ast::OrderItem;
use cypherlite_storage::StorageEngine;

/// Sort records by order items. Materializes all records then sorts.
pub fn execute_sort(
    mut records: Vec<Record>,
    items: &[OrderItem],
    engine: &StorageEngine,
    params: &Params,
    scalar_fns: &dyn ScalarFnLookup,
) -> Vec<Record> {
    records.sort_by(|a, b| {
        for item in items {
            let val_a = eval(&item.expr, a, engine, params, scalar_fns).unwrap_or(crate::executor::Value::Null);
            let val_b = eval(&item.expr, b, engine, params, scalar_fns).unwrap_or(crate::executor::Value::Null);
            let ord = compare_values(&val_a, &val_b);
            let ord = if item.ascending { ord } else { ord.reverse() };
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Value;
    use crate::parser::ast::*;
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

    #[test]
    fn test_sort_ascending() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::Int64(3));
        let mut r2 = Record::new();
        r2.insert("x".to_string(), Value::Int64(1));
        let mut r3 = Record::new();
        r3.insert("x".to_string(), Value::Int64(2));

        let items = vec![OrderItem {
            expr: Expression::Variable("x".to_string()),
            ascending: true,
        }];

        let params = Params::new();
        let result = execute_sort(vec![r1, r2, r3], &items, &engine, &params, &());
        assert_eq!(result[0].get("x"), Some(&Value::Int64(1)));
        assert_eq!(result[1].get("x"), Some(&Value::Int64(2)));
        assert_eq!(result[2].get("x"), Some(&Value::Int64(3)));
    }

    #[test]
    fn test_sort_descending() {
        let dir = tempdir().expect("tempdir");
        let engine = test_engine(dir.path());

        let mut r1 = Record::new();
        r1.insert("x".to_string(), Value::Int64(1));
        let mut r2 = Record::new();
        r2.insert("x".to_string(), Value::Int64(3));

        let items = vec![OrderItem {
            expr: Expression::Variable("x".to_string()),
            ascending: false,
        }];

        let params = Params::new();
        let result = execute_sort(vec![r1, r2], &items, &engine, &params, &());
        assert_eq!(result[0].get("x"), Some(&Value::Int64(3)));
        assert_eq!(result[1].get("x"), Some(&Value::Int64(1)));
    }
}
