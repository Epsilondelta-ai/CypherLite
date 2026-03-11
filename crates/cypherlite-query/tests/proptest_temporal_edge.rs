// Property-based tests for temporal edge filtering (Group FF-T2).
//
// Invariants tested:
// 1. Edge without _valid_from is always visible (backward compat)
// 2. AT TIME T with _valid_from > T -> edge not visible
// 3. AT TIME T with _valid_from <= T and no _valid_to -> edge visible
// 4. AT TIME T with _valid_from <= T < _valid_to -> edge visible
// 5. AT TIME T with _valid_to <= T -> edge not visible

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::{Params, Value};
use proptest::prelude::*;
use tempfile::tempdir;

fn proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 30,
        ..ProptestConfig::default()
    }
}

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

// Invariant 1: Edge without _valid_from is always visible at any time after creation
proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn prop_edge_no_valid_from_always_visible(t in 200i64..100_000) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Create at t=100 (auto-injected _valid_from = 100)
        let mut params = Params::new();
        params.insert("__query_start_ms__".to_string(), Value::Int64(100));
        db.execute_with_params(
            "CREATE (a:PropN {name: 'A'})-[:PROP_REL]->(b:PropN {name: 'B'})",
            params,
        ).expect("create");

        // AT TIME t (t >= 200 > 100=creation time): edge should always be visible
        let query = format!("MATCH (a:PropN {{name: 'A'}})-[r:PROP_REL]->(b:PropN) AT TIME {} RETURN a.name", t);
        let result = db.execute(&query).expect("query");
        prop_assert_eq!(result.rows.len(), 1,
            "edge with auto _valid_from=100 should be visible at t={}", t);
    }
}

// Invariant 2: AT TIME before _valid_from -> edge not visible
proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn prop_edge_not_visible_before_valid_from(
        valid_from in 500i64..10_000,
        offset in 1i64..400,
    ) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Create at t=10 (node early enough)
        let mut params = Params::new();
        params.insert("__query_start_ms__".to_string(), Value::Int64(10));
        db.execute_with_params(
            "CREATE (a:PropN2 {name: 'A'})-[:PROP_REL2]->(b:PropN2 {name: 'B'})",
            params,
        ).expect("create");

        // Set explicit _valid_from
        let set_query = format!(
            "MATCH (a:PropN2)-[r:PROP_REL2]->(b:PropN2) SET r._valid_from = {}",
            valid_from
        );
        db.execute(&set_query).expect("set");

        // AT TIME (valid_from - offset): before _valid_from -> not visible
        let t = valid_from - offset;
        let query = format!(
            "MATCH (a:PropN2 {{name: 'A'}})-[r:PROP_REL2]->(b:PropN2) AT TIME {} RETURN a.name",
            t
        );
        let result = db.execute(&query).expect("query");
        prop_assert_eq!(result.rows.len(), 0,
            "edge with _valid_from={} should not be visible at t={}", valid_from, t);
    }
}

// Invariant 3: AT TIME within validity window -> visible
proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn prop_edge_visible_within_window(
        valid_from in 100i64..5_000,
        window_size in 100i64..5_000,
        offset in 0i64..99,
    ) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        let valid_to = valid_from + window_size;

        // Create at t=10
        let mut params = Params::new();
        params.insert("__query_start_ms__".to_string(), Value::Int64(10));
        db.execute_with_params(
            "CREATE (a:PropN3 {name: 'A'})-[:PROP_REL3]->(b:PropN3 {name: 'B'})",
            params,
        ).expect("create");

        // Set validity window
        let set_query = format!(
            "MATCH (a:PropN3)-[r:PROP_REL3]->(b:PropN3) SET r._valid_from = {}, r._valid_to = {}",
            valid_from, valid_to
        );
        db.execute(&set_query).expect("set");

        // AT TIME within window
        let t = valid_from + (offset * window_size / 100).max(0);
        if t < valid_to {
            let query = format!(
                "MATCH (a:PropN3 {{name: 'A'}})-[r:PROP_REL3]->(b:PropN3) AT TIME {} RETURN a.name",
                t
            );
            let result = db.execute(&query).expect("query");
            prop_assert_eq!(result.rows.len(), 1,
                "edge with validity [{}, {}) should be visible at t={}", valid_from, valid_to, t);
        }
    }
}
