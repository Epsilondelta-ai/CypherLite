// Property-based tests for OPTIONAL MATCH and UNWIND (TASK-118).
//
// Tests invariants:
// 1. OPTIONAL MATCH: result rows >= source rows (left join never reduces)
// 2. OPTIONAL MATCH: unmatched rows have NULL for optional variables
// 3. UNWIND: output row count == sum of list lengths from input
// 4. UNWIND: empty list produces zero rows (not NULL row)

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;
use proptest::prelude::*;
use tempfile::tempdir;

fn open_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open db")
}

// ---------------------------------------------------------------------------
// TASK-118: OPTIONAL MATCH property-based tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 30, ..ProptestConfig::default() })]

    /// OPTIONAL MATCH left join invariant: result rows >= MATCH rows.
    /// For N isolated nodes with no outgoing KNOWS edges,
    /// OPTIONAL MATCH (a)-[:KNOWS]->(b) produces exactly N rows with b = NULL.
    #[test]
    fn optional_match_left_join_preserves_all_source_rows(
        node_count in 1usize..=10,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        // Create N isolated Person nodes (no edges)
        for i in 0..node_count {
            db.execute(&format!("CREATE (n:Person {{idx: {i}}})"))
                .expect("create node");
        }

        // OPTIONAL MATCH should return all source rows with NULL for b
        let result = db
            .execute("MATCH (a:Person) OPTIONAL MATCH (a)-[:KNOWS]->(b) RETURN a.idx, b")
            .expect("optional match");

        prop_assert_eq!(
            result.rows.len(),
            node_count,
            "OPTIONAL MATCH should preserve all {} source rows, got {}",
            node_count, result.rows.len()
        );

        // All b values should be NULL (no edges exist)
        for row in &result.rows {
            let b_val = row.get("b");
            prop_assert!(
                matches!(b_val, Some(Value::Null)),
                "b should be NULL for isolated nodes, got {:?}", b_val
            );
        }
    }

    /// OPTIONAL MATCH with some edges: rows >= source node count.
    /// Create a Source node with edges to K targets, plus (N-1) isolated Item nodes.
    /// Result should have edge_count + (total_nodes - 1) rows.
    ///
    /// NOTE: Uses unique labels (Source vs Item) because inline property
    /// filters in MATCH patterns are not yet implemented.
    #[test]
    fn optional_match_with_partial_edges(
        total_nodes in 2usize..=8,
        edge_count in 1usize..=4,
    ) {
        let edge_count = edge_count.min(total_nodes - 1);

        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        // Create the source node with a unique label
        db.execute("CREATE (a:Source {idx: 0})").expect("create source");

        // Create remaining isolated Item nodes
        for i in 1..total_nodes {
            db.execute(&format!("CREATE (n:Item {{idx: {i}}})"))
                .expect("create item");
        }

        // Create edges from the Source node to Target nodes.
        // Use unique Source label so MATCH finds exactly one node.
        for i in 1..=edge_count {
            db.execute(&format!(
                "MATCH (a:Source) CREATE (a)-[:LINK]->(b:Target {{idx: {}}})",
                100 + i
            ))
            .expect("create edge");
        }

        // Query uses a broader label scan: match Source and Item nodes via OPTIONAL MATCH.
        // We need to count: Source (has edge_count links) + (total_nodes-1) Items (no links, NULL).
        let result_source = db
            .execute("MATCH (a:Source) OPTIONAL MATCH (a)-[:LINK]->(b) RETURN a.idx, b")
            .expect("optional match source");
        let result_items = db
            .execute("MATCH (a:Item) OPTIONAL MATCH (a)-[:LINK]->(b) RETURN a.idx, b")
            .expect("optional match items");

        let total_rows = result_source.rows.len() + result_items.rows.len();

        // Source matches edge_count targets, Items each get 1 NULL row
        let expected_rows = edge_count + (total_nodes - 1);
        prop_assert_eq!(
            total_rows,
            expected_rows,
            "expected {} rows ({} edges + {} nulls), got {}",
            expected_rows, edge_count, total_nodes - 1, total_rows
        );
    }
}

// ---------------------------------------------------------------------------
// TASK-118: UNWIND property-based tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, ..ProptestConfig::default() })]

    /// UNWIND list length invariant: output rows == list element count.
    #[test]
    fn unwind_output_count_equals_list_length(list_len in 0usize..=20) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        let elements: Vec<String> = (0..list_len).map(|i| i.to_string()).collect();
        let list_str = elements.join(", ");
        let query = format!("UNWIND [{list_str}] AS x RETURN x");

        let result = db.execute(&query).expect("unwind query");

        prop_assert_eq!(
            result.rows.len(),
            list_len,
            "UNWIND of {}-element list should produce {} rows, got {}",
            list_len, list_len, result.rows.len()
        );
    }

    /// UNWIND preserves element values and order.
    #[test]
    fn unwind_preserves_element_values(list_len in 1usize..=10) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        let elements: Vec<i64> = (0..list_len as i64).collect();
        let list_str = elements.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
        let query = format!("UNWIND [{list_str}] AS x RETURN x");

        let result = db.execute(&query).expect("unwind query");

        let values: Vec<i64> = result.rows.iter()
            .filter_map(|r| r.get_as::<i64>("x"))
            .collect();

        prop_assert_eq!(
            values.len(),
            list_len,
            "should get {} values, got {}", list_len, values.len()
        );

        // Values should match the input list
        for (i, val) in values.iter().enumerate() {
            prop_assert_eq!(
                *val,
                elements[i],
                "element at index {} should be {}, got {}", i, elements[i], val
            );
        }
    }

    /// UNWIND with MATCH: output rows == source_rows * list_length.
    #[test]
    fn unwind_after_match_multiplies_rows(
        node_count in 1usize..=5,
        list_len in 1usize..=5,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        for i in 0..node_count {
            db.execute(&format!("CREATE (n:Thing {{idx: {i}}})"))
                .expect("create");
        }

        let elements: Vec<String> = (0..list_len).map(|i| i.to_string()).collect();
        let list_str = elements.join(", ");
        let query = format!("MATCH (n:Thing) UNWIND [{list_str}] AS x RETURN n.idx, x");

        let result = db.execute(&query).expect("match + unwind");

        let expected = node_count * list_len;
        prop_assert_eq!(
            result.rows.len(),
            expected,
            "{} nodes * {} elements should give {} rows, got {}",
            node_count, list_len, expected, result.rows.len()
        );
    }
}
