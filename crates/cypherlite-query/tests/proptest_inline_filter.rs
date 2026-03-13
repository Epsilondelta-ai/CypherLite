// Property-based tests for inline property filters in MATCH patterns (Phase 8c, SPEC-DB-008).
//
// Invariants tested:
// 1. MATCH (n:Label {key: value}) returns exactly the nodes whose property matches
// 2. Inline filter with non-matching value returns empty result
// 3. Multiple nodes with varying property values: filter returns correct subset

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
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

/// Strategy producing a safe string value for property values (no single quotes
/// or backslashes that would break Cypher string literals).
fn safe_prop_value() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z][a-zA-Z0-9 _]{0,15}")
        .expect("regex should compile")
}

// ---------------------------------------------------------------------------
// Invariant 1: Inline filter returns exactly the nodes with matching property
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    /// Create N nodes with unique string property values, then verify that
    /// MATCH (n:Label {name: target_value}) returns exactly 1 node.
    #[test]
    fn prop_inline_filter_returns_exact_match(
        n in 2usize..6,
        target_idx in 0usize..6,
    ) {
        // Ensure target_idx is within bounds
        let target_idx = target_idx % n;

        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Generate unique values for each node
        let values: Vec<String> = (0..n)
            .map(|i| format!("val_{}", i))
            .collect();

        // Create nodes with unique property values
        for val in &values {
            let q = format!("CREATE (:PF {{name: '{}'}})", val);
            db.execute(&q).expect("create node");
        }

        // Query with inline filter targeting one specific value
        let target = &values[target_idx];
        let q = format!("MATCH (n:PF {{name: '{}'}}) RETURN n.name", target);
        let result = db.execute(&q).expect("query");

        prop_assert_eq!(
            result.rows.len(), 1,
            "inline filter for '{}' should return exactly 1 node, got {}",
            target, result.rows.len()
        );
        let returned_name = result.rows[0].get_as::<String>("n.name");
        prop_assert_eq!(
            returned_name.as_deref(), Some(target.as_str()),
            "returned name should match target"
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant 2: Non-matching filter returns empty result
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    /// Create nodes with generated property values, then filter by a value
    /// known to be absent. Result should be empty.
    #[test]
    fn prop_inline_filter_no_match_returns_empty(
        n in 1usize..5,
    ) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Create nodes with prefix "exist_"
        for i in 0..n {
            let q = format!("CREATE (:NM {{tag: 'exist_{}'}})", i);
            db.execute(&q).expect("create node");
        }

        // Query with a value that does not exist
        let q = "MATCH (n:NM {tag: 'does_not_exist'}) RETURN n.tag";
        let result = db.execute(q).expect("query");

        prop_assert!(
            result.rows.is_empty(),
            "non-matching inline filter should return 0 rows, got {}",
            result.rows.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant 3: Random string property values filter correctly
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    /// Generate a random string property value, create exactly 2 nodes: one
    /// with the target value and one with a different value. Verify filter
    /// returns exactly 1 node.
    #[test]
    fn prop_inline_filter_random_string_values(
        target_val in safe_prop_value(),
    ) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Use a fixed "other" value that is guaranteed to differ
        let other_val = format!("{}_other", target_val);

        db.execute(&format!("CREATE (:RS {{tag: '{}'}})", target_val))
            .expect("create target");
        db.execute(&format!("CREATE (:RS {{tag: '{}'}})", other_val))
            .expect("create other");

        // Filter by target value
        let q = format!("MATCH (n:RS {{tag: '{}'}}) RETURN n.tag", target_val);
        let result = db.execute(&q).expect("query");

        prop_assert_eq!(
            result.rows.len(), 1,
            "filter for '{}' should return exactly 1 node, got {}",
            target_val, result.rows.len()
        );
        let returned = result.rows[0].get_as::<String>("n.tag");
        prop_assert_eq!(
            returned.as_deref(), Some(target_val.as_str()),
            "returned tag should match target"
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant 4: Integer property filter correctness
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    /// Create nodes with integer property values, verify inline filter by
    /// integer returns exact match.
    #[test]
    fn prop_inline_filter_integer_values(
        target in 0i64..100,
        n in 2usize..6,
    ) {
        let dir = tempdir().expect("tmpdir");
        let mut db = test_db(dir.path());

        // Create nodes with sequential integer values, ensuring target is included
        let mut created_values: Vec<i64> = (0..n as i64)
            .map(|i| target + i + 1) // values above target
            .collect();
        created_values.push(target); // add the target value

        for val in &created_values {
            let q = format!("CREATE (:IV {{score: {}}})", val);
            db.execute(&q).expect("create node");
        }

        // Filter by target integer value
        let q = format!("MATCH (n:IV {{score: {}}}) RETURN n.score", target);
        let result = db.execute(&q).expect("query");

        prop_assert_eq!(
            result.rows.len(), 1,
            "filter for score={} should return exactly 1 node, got {}",
            target, result.rows.len()
        );
        let returned = result.rows[0].get_as::<i64>("n.score");
        prop_assert_eq!(
            returned, Some(target),
            "returned score should match target"
        );
    }
}
