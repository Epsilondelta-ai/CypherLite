// Public API: CypherLite, QueryResult, Row, Transaction, Params, Value

use crate::executor::{Params, Record, Value};
use cypherlite_core::{CypherLiteError, DatabaseConfig};
use cypherlite_storage::StorageEngine;
use std::collections::HashMap;

/// Result of executing a Cypher query.
#[derive(Debug)]
pub struct QueryResult {
    /// Column names in order.
    pub columns: Vec<String>,
    /// Rows of data.
    pub rows: Vec<Row>,
}

/// A single row in a query result.
#[derive(Debug)]
pub struct Row {
    values: HashMap<String, Value>,
    columns: Vec<String>,
}

impl Row {
    /// Create a new row from a map of column name -> value and an ordered column list.
    pub fn new(values: HashMap<String, Value>, columns: Vec<String>) -> Self {
        Self { values, columns }
    }

    /// Get a value by column name.
    pub fn get(&self, column: &str) -> Option<&Value> {
        self.values.get(column)
    }

    /// Get a typed value by column name.
    pub fn get_as<T: FromValue>(&self, column: &str) -> Option<T> {
        self.values.get(column).and_then(T::from_value)
    }

    /// Get all column names.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Trait for converting Value to concrete Rust types.
pub trait FromValue: Sized {
    /// Attempt to extract a typed value from a `Value` reference.
    fn from_value(value: &Value) -> Option<Self>;
}

impl FromValue for i64 {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Int64(i) => Some(*i),
            _ => None,
        }
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Float64(f) => Some(*f),
            _ => None,
        }
    }
}

impl FromValue for String {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl FromValue for bool {
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// @MX:ANCHOR: Main CypherLite database interface -- primary public API entry point
// @MX:REASON: fan_in >= 3 (integration tests, user code, transaction wrapper)
/// The main CypherLite database interface.
pub struct CypherLite {
    engine: StorageEngine,
}

impl CypherLite {
    /// Open or create a CypherLite database.
    pub fn open(config: DatabaseConfig) -> Result<Self, CypherLiteError> {
        let engine = StorageEngine::open(config)?;
        Ok(Self { engine })
    }

    /// Execute a Cypher query string.
    pub fn execute(&mut self, query: &str) -> Result<QueryResult, CypherLiteError> {
        self.execute_with_params(query, Params::new())
    }

    /// Execute a Cypher query with parameters.
    pub fn execute_with_params(
        &mut self,
        query: &str,
        params: Params,
    ) -> Result<QueryResult, CypherLiteError> {
        // 1. Parse
        let ast = crate::parser::parse_query(query).map_err(|e| CypherLiteError::ParseError {
            line: e.line,
            column: e.column,
            message: e.message,
        })?;

        // 2. Semantic analysis
        let mut analyzer = crate::semantic::SemanticAnalyzer::new(self.engine.catalog_mut());
        analyzer
            .analyze(&ast)
            .map_err(|e| CypherLiteError::SemanticError(e.message))?;

        // 3. Plan
        let plan = crate::planner::LogicalPlanner::new(self.engine.catalog_mut())
            .plan(&ast)
            .map_err(|e| CypherLiteError::ExecutionError(e.message))?;

        // 4. Optimize (pass-through for now)
        let plan = crate::planner::optimize::optimize(plan);

        // 5. Execute
        let records = crate::executor::execute(&plan, &mut self.engine, &params)
            .map_err(|e| CypherLiteError::ExecutionError(e.message))?;

        // 6. Convert to QueryResult
        let columns = extract_columns(&records);
        let rows = records
            .into_iter()
            .map(|r| Row::new(r, columns.clone()))
            .collect();

        Ok(QueryResult { columns, rows })
    }

    /// Get a reference to the underlying storage engine.
    pub fn engine(&self) -> &StorageEngine {
        &self.engine
    }

    /// Get a mutable reference to the underlying storage engine.
    pub fn engine_mut(&mut self) -> &mut StorageEngine {
        &mut self.engine
    }

    /// Begin a transaction (simplified - wraps execute calls).
    pub fn begin(&mut self) -> Transaction<'_> {
        Transaction {
            db: self,
            committed: false,
        }
    }
}

/// Extract column names from the first record.
fn extract_columns(records: &[Record]) -> Vec<String> {
    if records.is_empty() {
        return vec![];
    }
    let mut cols: Vec<String> = records[0].keys().cloned().collect();
    cols.sort(); // deterministic column order
    cols
}

/// A transaction wrapping CypherLite execute calls.
///
/// Phase 2: simplified transaction without WAL integration.
/// Full rollback requires WAL integration (Phase 3).
pub struct Transaction<'a> {
    db: &'a mut CypherLite,
    committed: bool,
}

impl<'a> Transaction<'a> {
    /// Execute a query within this transaction.
    pub fn execute(&mut self, query: &str) -> Result<QueryResult, CypherLiteError> {
        self.db.execute(query)
    }

    /// Execute a query with parameters within this transaction.
    pub fn execute_with_params(
        &mut self,
        query: &str,
        params: Params,
    ) -> Result<QueryResult, CypherLiteError> {
        self.db.execute_with_params(query, params)
    }

    /// Commit the transaction.
    pub fn commit(mut self) -> Result<(), CypherLiteError> {
        self.committed = true;
        Ok(())
    }

    /// Rollback the transaction (discard changes).
    /// For Phase 2, this is a no-op since we don't have WAL integration yet.
    pub fn rollback(mut self) -> Result<(), CypherLiteError> {
        self.committed = true; // prevent double-rollback
                               // Phase 2: no actual rollback - in-memory changes remain
                               // Full rollback requires WAL integration (Phase 3)
        Ok(())
    }
}

impl<'a> Drop for Transaction<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Auto-rollback on drop (no-op for Phase 2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::SyncMode;
    use tempfile::tempdir;

    fn test_config(dir: &std::path::Path) -> DatabaseConfig {
        DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        }
    }

    // ======================================================================
    // TASK-054: QueryResult, Row, FromValue tests
    // ======================================================================

    #[test]
    fn test_row_get_existing_column() {
        let mut values = HashMap::new();
        values.insert("name".to_string(), Value::String("Alice".into()));
        let row = Row::new(values, vec!["name".to_string()]);
        assert_eq!(row.get("name"), Some(&Value::String("Alice".into())));
    }

    #[test]
    fn test_row_get_missing_column() {
        let row = Row::new(HashMap::new(), vec![]);
        assert_eq!(row.get("missing"), None);
    }

    #[test]
    fn test_row_get_as_i64() {
        let mut values = HashMap::new();
        values.insert("age".to_string(), Value::Int64(30));
        let row = Row::new(values, vec!["age".to_string()]);
        assert_eq!(row.get_as::<i64>("age"), Some(30));
    }

    #[test]
    fn test_row_get_as_f64() {
        let mut values = HashMap::new();
        values.insert("score".to_string(), Value::Float64(3.14));
        let row = Row::new(values, vec!["score".to_string()]);
        assert_eq!(row.get_as::<f64>("score"), Some(3.14));
    }

    #[test]
    fn test_row_get_as_string() {
        let mut values = HashMap::new();
        values.insert("name".to_string(), Value::String("Bob".into()));
        let row = Row::new(values, vec!["name".to_string()]);
        assert_eq!(row.get_as::<String>("name"), Some("Bob".to_string()));
    }

    #[test]
    fn test_row_get_as_bool() {
        let mut values = HashMap::new();
        values.insert("active".to_string(), Value::Bool(true));
        let row = Row::new(values, vec!["active".to_string()]);
        assert_eq!(row.get_as::<bool>("active"), Some(true));
    }

    #[test]
    fn test_row_get_as_wrong_type() {
        let mut values = HashMap::new();
        values.insert("age".to_string(), Value::String("thirty".into()));
        let row = Row::new(values, vec!["age".to_string()]);
        assert_eq!(row.get_as::<i64>("age"), None);
    }

    #[test]
    fn test_row_columns() {
        let row = Row::new(HashMap::new(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(row.columns(), &["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn test_query_result_empty() {
        let result = QueryResult {
            columns: vec![],
            rows: vec![],
        };
        assert!(result.rows.is_empty());
        assert!(result.columns.is_empty());
    }

    #[test]
    fn test_from_value_null_returns_none() {
        assert_eq!(i64::from_value(&Value::Null), None);
        assert_eq!(f64::from_value(&Value::Null), None);
        assert_eq!(String::from_value(&Value::Null), None);
        assert_eq!(bool::from_value(&Value::Null), None);
    }

    #[test]
    fn test_extract_columns_empty_records() {
        let records: Vec<Record> = vec![];
        assert!(extract_columns(&records).is_empty());
    }

    #[test]
    fn test_extract_columns_deterministic_order() {
        let mut r = Record::new();
        r.insert("b".to_string(), Value::Int64(1));
        r.insert("a".to_string(), Value::Int64(2));
        let cols = extract_columns(&[r]);
        assert_eq!(cols, vec!["a".to_string(), "b".to_string()]);
    }

    // ======================================================================
    // TASK-055: CypherLite::open(), execute() tests
    // ======================================================================

    #[test]
    fn test_cypherlite_open() {
        let dir = tempdir().expect("tempdir");
        let db = CypherLite::open(test_config(dir.path()));
        assert!(db.is_ok());
    }

    #[test]
    fn test_cypherlite_engine_accessors() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        assert_eq!(db.engine().node_count(), 0);
        assert_eq!(db.engine_mut().edge_count(), 0);
    }

    // ======================================================================
    // TASK-056: Transaction tests
    // ======================================================================

    #[test]
    fn test_transaction_commit() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        let tx = db.begin();
        assert!(tx.commit().is_ok());
    }

    #[test]
    fn test_transaction_rollback() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        let tx = db.begin();
        assert!(tx.rollback().is_ok());
    }

    #[test]
    fn test_transaction_auto_rollback_on_drop() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");
        {
            let _tx = db.begin();
            // dropped without commit or rollback -- should not panic
        }
    }

    // ======================================================================
    // TASK-057: End-to-end integration tests
    // ======================================================================

    // INT-T001: open -> CREATE -> MATCH
    #[test]
    fn int_t001_create_then_match() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        // Create a node
        db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
            .expect("create");

        // Query the node
        let result = db
            .execute("MATCH (n:Person) RETURN n.name, n.age")
            .expect("match");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("n.name"),
            Some("Alice".to_string())
        );
        assert_eq!(result.rows[0].get_as::<i64>("n.age"), Some(30));
    }

    // INT-T002: Parameter binding $name
    #[test]
    fn int_t002_parameter_binding() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (n:Person {name: 'Alice'})")
            .expect("create");

        let mut params = Params::new();
        params.insert("name".to_string(), Value::String("Alice".into()));

        let result = db
            .execute_with_params(
                "MATCH (n:Person) WHERE n.name = $name RETURN n.name",
                params,
            )
            .expect("match with params");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("n.name"),
            Some("Alice".to_string())
        );
    }

    // INT-T003: Transaction commit
    #[test]
    fn int_t003_transaction_commit() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        {
            let mut tx = db.begin();
            tx.execute("CREATE (n:Person {name: 'Bob'})")
                .expect("create in tx");
            tx.commit().expect("commit");
        }

        // Verify data persists after commit
        let result = db
            .execute("MATCH (n:Person) RETURN n.name")
            .expect("match after commit");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("n.name"),
            Some("Bob".to_string())
        );
    }

    // INT-T004: Invalid Cypher -> ParseError (no panic)
    #[test]
    fn int_t004_invalid_cypher_parse_error() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        let result = db.execute("INVALID QUERY @#$");
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        // Should be a parse error, not a panic
        assert!(
            matches!(err, CypherLiteError::ParseError { .. }),
            "expected ParseError, got: {err}"
        );
    }

    // INT-T005: MATCH non-existent label -> empty result (not error)
    #[test]
    fn int_t005_match_nonexistent_label_empty() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        let result = db
            .execute("MATCH (n:NonExistent) RETURN n")
            .expect("should succeed with empty result");
        assert!(result.rows.is_empty());
    }

    // INT-T006: SET then MATCH to verify change
    #[test]
    fn int_t006_set_then_match() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (n:Person {name: 'Alice', age: 25})")
            .expect("create");

        db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

        let result = db
            .execute("MATCH (n:Person) RETURN n.age")
            .expect("match after set");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].get_as::<i64>("n.age"), Some(30));
    }

    // INT-T007: DETACH DELETE
    #[test]
    fn int_t007_detach_delete() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
            .expect("create");

        // Verify nodes exist
        let result = db
            .execute("MATCH (n:Person) RETURN n.name")
            .expect("match before delete");
        assert_eq!(result.rows.len(), 2);

        // Detach delete all Person nodes
        db.execute("MATCH (n:Person) DETACH DELETE n")
            .expect("detach delete");

        // Verify no nodes remain
        let result = db
            .execute("MATCH (n:Person) RETURN n.name")
            .expect("match after delete");
        assert!(result.rows.is_empty());
    }

    // AC-001: MATCH (n:Person) RETURN n.name with 3 Person nodes
    #[test]
    fn ac_001_match_return_three_persons() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (n:Person {name: 'Alice'})").expect("c1");
        db.execute("CREATE (n:Person {name: 'Bob'})").expect("c2");
        db.execute("CREATE (n:Person {name: 'Charlie'})")
            .expect("c3");

        let result = db.execute("MATCH (n:Person) RETURN n.name").expect("match");
        assert_eq!(result.rows.len(), 3);

        let mut names: Vec<String> = result
            .rows
            .iter()
            .filter_map(|r| r.get_as::<String>("n.name"))
            .collect();
        names.sort();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    // AC-002: CREATE (a:Person {name: "Alice"}) then MATCH verify
    #[test]
    fn ac_002_create_then_match_verify() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (a:Person {name: 'Alice'})")
            .expect("create");

        let result = db.execute("MATCH (n:Person) RETURN n.name").expect("match");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("n.name"),
            Some("Alice".to_string())
        );
    }

    // AC-003: CREATE relationship then traverse
    #[test]
    fn ac_003_create_relationship_then_traverse() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
            .expect("create relationship");

        let result = db
            .execute("MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN b.name")
            .expect("traverse");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("b.name"),
            Some("Bob".to_string())
        );
    }

    // AC-004: WHERE n.age > 28 filter
    #[test]
    fn ac_004_where_filter() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (n:Person {name: 'Alice', age: 30})")
            .expect("c1");
        db.execute("CREATE (n:Person {name: 'Bob', age: 25})")
            .expect("c2");
        db.execute("CREATE (n:Person {name: 'Charlie', age: 35})")
            .expect("c3");

        let result = db
            .execute("MATCH (n:Person) WHERE n.age > 28 RETURN n.name")
            .expect("filter");
        assert_eq!(result.rows.len(), 2);

        let mut names: Vec<String> = result
            .rows
            .iter()
            .filter_map(|r| r.get_as::<String>("n.name"))
            .collect();
        names.sort();
        assert_eq!(names, vec!["Alice", "Charlie"]);
    }

    // AC-006: Syntax error detection with position
    #[test]
    fn ac_006_syntax_error_detection() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        let result = db.execute("MATCH (n:Person RETURN n");
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        match err {
            CypherLiteError::ParseError {
                line,
                column,
                message,
            } => {
                assert!(line >= 1, "line should be >= 1, got {line}");
                assert!(column >= 1, "column should be >= 1, got {column}");
                assert!(!message.is_empty(), "error message should not be empty");
            }
            other => panic!("expected ParseError, got: {other}"),
        }
    }

    // AC-007: Type mismatch error (undefined variable as semantic error)
    #[test]
    fn ac_007_semantic_error() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        // Reference undefined variable 'm' instead of 'n'
        let result = db.execute("MATCH (n:Person) RETURN m.name");
        assert!(result.is_err());
        let err = result.expect_err("should fail");
        assert!(
            matches!(err, CypherLiteError::SemanticError(_)),
            "expected SemanticError, got: {err}"
        );
    }

    // AC-010: NULL handling (IS NOT NULL, missing property returns NULL)
    #[test]
    fn ac_010_null_handling() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        // Create nodes: one with email, one without
        db.execute("CREATE (n:Person {name: 'Alice', email: 'alice@example.com'})")
            .expect("c1");
        db.execute("CREATE (n:Person {name: 'Bob'})").expect("c2");

        // Query for a property that may be missing
        let result = db
            .execute("MATCH (n:Person) RETURN n.name, n.email")
            .expect("match");
        assert_eq!(result.rows.len(), 2);

        // One row should have email, one should have Null
        let mut found_null = false;
        let mut found_email = false;
        for row in &result.rows {
            match row.get("n.email") {
                Some(Value::String(s)) if !s.is_empty() => found_email = true,
                Some(Value::Null) | None => found_null = true,
                _ => {}
            }
        }
        assert!(found_email, "should find at least one row with email");
        assert!(found_null, "should find at least one row with null email");
    }

    // Additional: IS NOT NULL filter
    #[test]
    fn ac_010_is_not_null_filter() {
        let dir = tempdir().expect("tempdir");
        let mut db = CypherLite::open(test_config(dir.path())).expect("open");

        db.execute("CREATE (n:Person {name: 'Alice', email: 'alice@example.com'})")
            .expect("c1");
        db.execute("CREATE (n:Person {name: 'Bob'})").expect("c2");

        let result = db
            .execute("MATCH (n:Person) WHERE n.email IS NOT NULL RETURN n.name")
            .expect("filter not null");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get_as::<String>("n.name"),
            Some("Alice".to_string())
        );
    }
}
