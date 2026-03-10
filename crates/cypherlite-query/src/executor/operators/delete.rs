// DeleteOp: node/edge deletion, ConstraintError if non-detach with edges

use crate::executor::eval::eval;
use crate::executor::{ExecutionError, Params, Record, Value};
use crate::parser::ast::Expression;
use cypherlite_storage::StorageEngine;

/// Delete nodes or edges identified by expressions.
/// If detach is false and a node has edges, returns a ConstraintViolation error.
/// If detach is true, uses engine.delete_node() which cascades edges.
pub fn execute_delete(
    source_records: Vec<Record>,
    exprs: &[Expression],
    detach: bool,
    engine: &mut StorageEngine,
    params: &Params,
) -> Result<Vec<Record>, ExecutionError> {
    // Collect all entity IDs to delete first, then delete.
    // This avoids issues with deleting while iterating.
    let mut node_ids_to_delete = Vec::new();
    let mut edge_ids_to_delete = Vec::new();

    for record in &source_records {
        for expr in exprs {
            let val = eval(expr, record, &*engine, params)?;
            match val {
                Value::Node(nid) => {
                    if !node_ids_to_delete.contains(&nid) {
                        node_ids_to_delete.push(nid);
                    }
                }
                Value::Edge(eid) => {
                    if !edge_ids_to_delete.contains(&eid) {
                        edge_ids_to_delete.push(eid);
                    }
                }
                Value::Null => {
                    // Deleting null is a no-op
                }
                _ => {
                    return Err(ExecutionError {
                        message: "DELETE requires a node or edge value".to_string(),
                    });
                }
            }
        }
    }

    // Delete edges first
    for eid in edge_ids_to_delete {
        engine.delete_edge(eid).map_err(|e| ExecutionError {
            message: format!("failed to delete edge: {}", e),
        })?;
    }

    // Delete nodes
    for nid in node_ids_to_delete {
        if !detach {
            // Check if node has edges
            let edges = engine.get_edges_for_node(nid);
            if !edges.is_empty() {
                return Err(ExecutionError {
                    message: format!(
                        "cannot delete node {} because it still has {} relationship(s). Use DETACH DELETE",
                        nid.0,
                        edges.len()
                    ),
                });
            }
        }
        engine.delete_node(nid).map_err(|e| ExecutionError {
            message: format!("failed to delete node: {}", e),
        })?;
    }

    // DELETE returns the source records (for chaining)
    Ok(source_records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Record;
    use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // EXEC-T006: DeleteOp with relationships -> ConstraintError (without DETACH)
    #[test]
    fn test_delete_node_with_edges_no_detach_fails() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(n1));

        let exprs = vec![Expression::Variable("n".to_string())];
        let params = Params::new();

        let result = execute_delete(vec![record], &exprs, false, &mut engine, &params);
        assert!(result.is_err());
        let err = result.expect_err("should error");
        assert!(err.message.contains("cannot delete node"));
        assert!(err.message.contains("DETACH DELETE"));
    }

    #[test]
    fn test_delete_node_with_detach_succeeds() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let knows_type = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        engine
            .create_edge(n1, n2, knows_type, vec![])
            .expect("edge");

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(n1));

        let exprs = vec![Expression::Variable("n".to_string())];
        let params = Params::new();

        let result = execute_delete(vec![record], &exprs, true, &mut engine, &params);
        assert!(result.is_ok());
        assert!(engine.get_node(n1).is_none());
        assert_eq!(engine.edge_count(), 0);
    }

    #[test]
    fn test_delete_isolated_node() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let n1 = engine.create_node(vec![], vec![]);

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Node(n1));

        let exprs = vec![Expression::Variable("n".to_string())];
        let params = Params::new();

        let result = execute_delete(vec![record], &exprs, false, &mut engine, &params);
        assert!(result.is_ok());
        assert!(engine.get_node(n1).is_none());
    }

    #[test]
    fn test_delete_null_is_noop() {
        let dir = tempdir().expect("tempdir");
        let mut engine = test_engine(dir.path());

        let mut record = Record::new();
        record.insert("n".to_string(), Value::Null);

        let exprs = vec![Expression::Variable("n".to_string())];
        let params = Params::new();

        let result = execute_delete(vec![record], &exprs, false, &mut engine, &params);
        assert!(result.is_ok());
    }
}
