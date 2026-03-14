// Property-based tests for CypherLite temporal features (Group Z).
//
// Tests invariants including:
// 1. AT TIME before any version -> empty result
// 2. AT TIME after all versions -> returns latest state
// 3. BETWEEN TIME with start > end -> empty result
// 4. _created_at is always set on CREATE
// 5. DateTime round-trip through CypherLite (datetime() -> return -> parse)
// 6. Version chain ordering is monotonically increasing

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::{Params, Value};
use proptest::prelude::*;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

fn temporal_config() -> ProptestConfig {
    ProptestConfig {
        cases: 50,
        ..ProptestConfig::default()
    }
}

fn fast_config() -> ProptestConfig {
    ProptestConfig {
        cases: 200,
        ..ProptestConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}

/// Create a node at a specific timestamp via params.
fn create_node_at(db: &mut CypherLite, label: &str, props: &str, ts: i64) {
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(ts));
    db.execute_with_params(&format!("CREATE (n:{label} {{{props}}})"), params)
        .expect("create_node_at");
}

/// Update nodes at a specific timestamp via params.
fn update_at(db: &mut CypherLite, query: &str, ts: i64) {
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(ts));
    db.execute_with_params(query, params).expect("update_at");
}

// ---------------------------------------------------------------------------
// 1. AT TIME before any version returns empty
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(temporal_config())]

    /// If a node is created at time `create_ts`, querying AT TIME with any
    /// timestamp strictly before `create_ts` must return zero rows.
    #[test]
    fn at_time_before_creation_returns_empty(
        create_ts in 1000i64..=1_000_000,
        query_offset in 1i64..=999,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        create_node_at(&mut db, "PropNode", "val: 42", create_ts);

        let query_ts = create_ts - query_offset;
        let result = db
            .execute(&format!(
                "MATCH (n:PropNode) AT TIME {query_ts} RETURN n.val"
            ))
            .expect("at time query");
        prop_assert_eq!(
            result.rows.len(),
            0,
            "AT TIME {} (before creation at {}) should return 0 rows, got {}",
            query_ts,
            create_ts,
            result.rows.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 2. AT TIME after all versions returns latest state
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(temporal_config())]

    /// If a node is created at time T1 and updated at T2 (T2 > T1),
    /// querying AT TIME with T3 > T2 must return the updated (latest) state.
    #[test]
    fn at_time_after_all_versions_returns_latest(
        base_ts in 1000i64..=100_000,
        delta1 in 100i64..=10_000,
        delta2 in 100i64..=10_000,
        final_age in 1i64..=999,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        let t1 = base_ts;
        let t2 = base_ts + delta1;
        let t3 = base_ts + delta1 + delta2;

        create_node_at(&mut db, "PropNode", "age: 10", t1);
        update_at(&mut db, &format!("MATCH (n:PropNode) SET n.age = {final_age}"), t2);

        let result = db
            .execute(&format!(
                "MATCH (n:PropNode) AT TIME {t3} RETURN n.age"
            ))
            .expect("at time query");
        prop_assert_eq!(result.rows.len(), 1, "should find exactly 1 node");
        let age = result.rows[0].get_as::<i64>("n.age");
        prop_assert_eq!(age, Some(final_age), "AT TIME after last update should return latest age");
    }
}

// ---------------------------------------------------------------------------
// 3. BETWEEN TIME with start > end returns empty
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(temporal_config())]

    /// BETWEEN TIME with start strictly greater than end must return zero rows.
    #[test]
    fn between_time_inverted_range_returns_empty(
        create_ts in 1000i64..=100_000,
        range_base in 500i64..=50_000,
        range_gap in 1i64..=10_000,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        create_node_at(&mut db, "PropNode", "val: 1", create_ts);

        let start = range_base + range_gap; // start > end
        let end = range_base;

        let result = db
            .execute(&format!(
                "MATCH (n:PropNode) BETWEEN TIME {start} AND {end} RETURN n.val"
            ))
            .expect("between time query");
        prop_assert_eq!(
            result.rows.len(),
            0,
            "BETWEEN TIME with start({}) > end({}) should return 0 rows, got {}",
            start,
            end,
            result.rows.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 4. _created_at is always set on CREATE
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(temporal_config())]

    /// Every node created via CREATE must have a _created_at property set.
    #[test]
    fn created_at_always_set_on_create(
        ts in 1000i64..=1_000_000,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        create_node_at(&mut db, "PropNode", "val: 1", ts);

        let result = db
            .execute("MATCH (n:PropNode) RETURN n._created_at")
            .expect("query _created_at");
        prop_assert_eq!(result.rows.len(), 1, "should find exactly 1 node");
        // _created_at is a DateTime value (not Int64), so use get() to access raw Value
        let created_at = result.rows[0].get("n._created_at");
        match created_at {
            Some(Value::DateTime(millis)) => {
                prop_assert_eq!(
                    *millis, ts,
                    "_created_at should equal creation timestamp {}", ts
                );
            }
            other => {
                prop_assert!(
                    false,
                    "_created_at should be DateTime({}), got {:?}",
                    ts, other
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5. DateTime round-trip: datetime(string) -> RETURN -> consistent millis
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(fast_config())]

    /// For valid date components, `datetime('YYYY-MM-DDTHH:MM:SSZ')` parsed via
    /// a MATCH ... WHERE datetime(...) > 0 RETURN query must produce consistent
    /// and deterministic results.
    #[test]
    fn datetime_parsing_consistent(
        year in 1970i64..=2100,
        month in 1u32..=12,
        day in 1u32..=28, // stay within valid range for all months
        hour in 0u32..=23,
        minute in 0u32..=59,
        second in 0u32..=59,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        let iso = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hour, minute, second
        );

        // Create a dummy node so MATCH has something to scan
        db.execute("CREATE (n:DtTest {v: 1})").expect("create");

        // Parse via CypherLite's datetime() function in a MATCH context
        let query = format!(
            "MATCH (n:DtTest) RETURN datetime('{iso}')"
        );
        let result = db.execute(&query).expect("datetime parse");
        prop_assert_eq!(result.rows.len(), 1, "should return exactly 1 row");

        // Parse same string again -- should produce the same value
        let result2 = db.execute(&query).expect("datetime parse 2");
        prop_assert_eq!(result2.rows.len(), 1);

        // Both calls must return the same value (idempotency)
        // datetime() returns Value::DateTime(millis), not Value::Int64
        let col = &result.columns[0];
        let v1 = match result.rows[0].get(col) {
            Some(Value::DateTime(ms)) => Some(*ms),
            Some(Value::Int64(ms)) => Some(*ms),
            _ => None,
        };
        let v2 = match result2.rows[0].get(col) {
            Some(Value::DateTime(ms)) => Some(*ms),
            Some(Value::Int64(ms)) => Some(*ms),
            _ => None,
        };
        prop_assert_eq!(v1, v2, "datetime('{}') should be idempotent", iso);

        // Value must be non-None (valid parse)
        prop_assert!(v1.is_some(), "datetime('{}') should produce a value", iso);
    }
}

// ---------------------------------------------------------------------------
// 6. Version chain ordering is monotonically increasing
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 30, ..ProptestConfig::default() })]

    /// When multiple updates are performed at increasing timestamps,
    /// BETWEEN TIME over the full range should return versions in
    /// monotonically increasing timestamp order (as observed by _updated_at).
    #[test]
    fn version_chain_monotonically_ordered(
        base_ts in 1000i64..=10_000,
        num_updates in 2u32..=5,
        delta in 100i64..=1000,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = test_db(dir.path());

        // Create initial node
        create_node_at(&mut db, "ChainNode", "step: 0", base_ts);

        // Perform num_updates updates at increasing timestamps
        for i in 1..=num_updates {
            let ts = base_ts + (i as i64) * delta;
            update_at(
                &mut db,
                &format!("MATCH (n:ChainNode) SET n.step = {i}"),
                ts,
            );
        }

        // Query the full range
        let start = base_ts - 1;
        let end = base_ts + (num_updates as i64 + 1) * delta;
        let result = db
            .execute(&format!(
                "MATCH (n:ChainNode) BETWEEN TIME {start} AND {end} RETURN n._updated_at"
            ))
            .expect("between time query");

        // Collect _updated_at values (DateTime type, not Int64)
        let timestamps: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| match r.get("n._updated_at") {
                Some(Value::DateTime(millis)) => Some(*millis),
                _ => None,
            })
            .collect();

        // Verify monotonically non-decreasing
        for window in timestamps.windows(2) {
            prop_assert!(
                window[0] <= window[1],
                "version timestamps should be monotonically non-decreasing: {} > {}",
                window[0],
                window[1]
            );
        }
    }
}
