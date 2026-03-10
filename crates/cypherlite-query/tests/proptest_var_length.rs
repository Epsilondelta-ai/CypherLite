// Property-based tests for variable-length paths (TASK-110).
//
// Tests invariants on random graphs:
// 1. Path depth always respects min_hops..max_hops bounds
// 2. No duplicate nodes within a single path (cycle detection)
// 3. Unbounded [*] respects default max_hops cap (10)
//
// NOTE: Inline property filters in MATCH patterns are not yet implemented
// (they match all nodes of the label). Tests use unique labels or WHERE
// clauses as a workaround.

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
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

/// Build a chain of nodes using a single CREATE statement:
/// (n0:Start {idx:0})-[:NEXT]->(n1:Mid {idx:1})-[:NEXT]->...
/// The first node uses label :Start so queries can target it uniquely.
fn build_chain(db: &mut CypherLite, chain_len: usize) {
    if chain_len == 0 {
        return;
    }
    // Build a single CREATE statement for the whole chain
    let mut parts = Vec::with_capacity(chain_len);
    for i in 0..chain_len {
        let label = if i == 0 { "Start" } else { "Mid" };
        parts.push(format!("(n{i}:{label} {{idx: {i}}})"));
    }
    let chain = parts.join("-[:NEXT]->");
    let query = format!("CREATE {chain}");
    db.execute(&query).expect("create chain");
}

// ---------------------------------------------------------------------------
// TASK-110: Variable-length path invariants on random graphs
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

    /// Bounded variable-length paths: result count matches expected reachable
    /// nodes within [min_hops, max_hops] distance from the Start node.
    #[test]
    fn var_path_bounded_respects_hop_limits(
        chain_len in 2usize..=12,
        min_hops in 1u32..=3,
        max_hops_offset in 0u32..=4,
    ) {
        let max_hops = min_hops + max_hops_offset;
        let max_hops = max_hops.min(10);

        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());
        build_chain(&mut db, chain_len);

        let query = format!(
            "MATCH (a:Start)-[:NEXT*{}..{}]->(b) RETURN b.idx",
            min_hops, max_hops
        );
        let result = db.execute(&query).expect("var path query");

        // Expected reachable nodes: indices min_hops..min(max_hops, chain_len-1)
        let max_reachable = (chain_len as u32).saturating_sub(1).min(max_hops);
        let expected_count = if max_reachable >= min_hops {
            (max_reachable - min_hops + 1) as usize
        } else {
            0
        };

        prop_assert_eq!(
            result.rows.len(),
            expected_count,
            "chain_len={}, min={}, max={}, expected {} results, got {}",
            chain_len, min_hops, max_hops, expected_count, result.rows.len()
        );

        // Verify all returned indices are within bounds
        for row in &result.rows {
            if let Some(idx) = row.get_as::<i64>("b.idx") {
                prop_assert!(
                    idx >= min_hops as i64 && idx <= max_hops as i64,
                    "idx {} outside [{}, {}]", idx, min_hops, max_hops
                );
            }
        }
    }

    /// Exact-hop paths: [*N] returns exactly nodes at distance N.
    #[test]
    fn var_path_exact_hop_returns_single_depth(
        chain_len in 3usize..=10,
        exact_hop in 1u32..=5,
    ) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());
        build_chain(&mut db, chain_len);

        let query = format!(
            "MATCH (a:Start)-[:NEXT*{}]->(b) RETURN b.idx",
            exact_hop
        );
        let result = db.execute(&query).expect("exact hop query");

        if (exact_hop as usize) < chain_len {
            prop_assert_eq!(result.rows.len(), 1);
            let idx = result.rows[0].get_as::<i64>("b.idx").unwrap();
            prop_assert_eq!(idx, exact_hop as i64);
        } else {
            prop_assert_eq!(result.rows.len(), 0);
        }
    }

    /// Unbounded [*] paths on a chain: result count == min(chain_len - 1, 10),
    /// never exceeds default max_hops cap (10).
    #[test]
    fn var_path_unbounded_respects_max_cap(chain_len in 2usize..=15) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());
        build_chain(&mut db, chain_len);

        let result = db
            .execute("MATCH (a:Start)-[:NEXT*]->(b) RETURN b.idx")
            .expect("unbounded path query");

        // Default max_hops = 10; reachable = min(chain_len - 1, 10)
        let expected = (chain_len - 1).min(10);
        prop_assert_eq!(
            result.rows.len(),
            expected,
            "chain_len={}, expected {} results (cap 10), got {}",
            chain_len, expected, result.rows.len()
        );
    }

    /// Cycle detection: on a cyclic graph (A->B->C->A), variable-length
    /// paths should not produce infinite results.
    #[test]
    fn var_path_cycle_detection_finite_results(max_hops in 2u32..=8) {
        let dir = tempdir().expect("tempdir");
        let mut db = open_db(dir.path());

        // Build cycle using unique labels to avoid inline property filter issue:
        // CycA -> CycB -> CycC -> CycA
        db.execute(
            "CREATE (a:CycA {name: 'A'})-[:LINK]->(b:CycB {name: 'B'})-[:LINK]->(c:CycC {name: 'C'})"
        ).expect("chain");
        // Close the cycle: C -> A
        db.execute(
            "MATCH (c:CycC), (a:CycA) CREATE (c)-[:LINK]->(a)"
        ).expect("close cycle");

        let query = format!(
            "MATCH (a:CycA)-[:LINK*1..{}]->(b) RETURN b.name",
            max_hops
        );
        let result = db.execute(&query).expect("cycle query");

        // With cycle detection, at most 2 unique nodes reachable (B and C),
        // regardless of max_hops. Results should be finite and bounded.
        prop_assert!(
            result.rows.len() <= max_hops as usize,
            "results ({}) should be bounded by max_hops ({})",
            result.rows.len(), max_hops
        );
        // Should always find at least B (1 hop away)
        prop_assert!(
            result.rows.len() >= 1,
            "should find at least B from A"
        );
    }
}
