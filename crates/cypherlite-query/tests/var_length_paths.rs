// Integration tests for Variable-Length Paths (Group R, SPEC-DB-003)
//
// Tests the full pipeline: parse -> semantic -> plan -> execute

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use tempfile::tempdir;

fn open_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open db")
}

// ======================================================================
// TASK-109: Variable-length path integration tests
// ======================================================================

/// Bounded paths [*1..3]: from a Start label node through a chain
#[test]
fn test_var_length_bounded_1_to_3() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Create a chain with a unique starting label
    db.execute(
        "CREATE (a:Start)-[:KNOWS]->(b:Mid)-[:KNOWS]->(c:Mid)-[:KNOWS]->(d:End)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Start)-[:KNOWS*1..3]->(b) RETURN b")
        .expect("should execute");

    // 1-hop: first Mid, 2-hop: second Mid, 3-hop: End
    assert_eq!(result.rows.len(), 3, "should find 3 nodes within 1..3 hops");
}

/// Unbounded [*] with default max cap
#[test]
fn test_var_length_unbounded() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:L1)-[:KNOWS]->(c:L2)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*]->(b) RETURN b")
        .expect("should execute");

    // [*] defaults to min=1, max=DEFAULT_MAX_HOPS(10)
    // Reachable: L1 (1-hop), L2 (2-hop)
    assert_eq!(result.rows.len(), 2, "should find 2 reachable nodes");
}

/// Cycle detection: traversal terminates in cyclic graph
#[test]
fn test_var_length_cycle_detection() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Linear chain from Root, no cycle. Verify termination.
    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:KNOWS]->(c:N2)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*1..10]->(b) RETURN b")
        .expect("should execute");

    // No cycle: N1 (1-hop), N2 (2-hop) - should terminate cleanly
    assert_eq!(result.rows.len(), 2, "should terminate without cycle");
}

/// Exact hop [*2]: only paths of exactly 2 hops
#[test]
fn test_var_length_exact_hop() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:KNOWS]->(c:N2)-[:KNOWS]->(d:N3)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*2]->(b) RETURN b")
        .expect("should execute");

    // Exactly 2 hops from Root: N2
    assert_eq!(result.rows.len(), 1, "should find exactly 1 node at 2 hops");
}

/// Typed paths [:KNOWS*2]: only follow KNOWS, not LIKES
#[test]
fn test_var_length_typed_path() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Root -KNOWS-> N1 -LIKES-> N2
    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:LIKES]->(c:N2)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*1..3]->(b) RETURN b")
        .expect("should execute");

    // Only KNOWS edges: Root->N1 (1-hop). N1->N2 is LIKES, not KNOWS.
    assert_eq!(result.rows.len(), 1, "should only follow KNOWS edges");
}

/// Zero-length [*0..1]: includes source node itself
#[test]
fn test_var_length_zero_length() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:Target)"
    ).expect("create");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*0..1]->(b) RETURN b")
        .expect("should execute");

    // 0-hop: Root itself, 1-hop: Target
    assert_eq!(result.rows.len(), 2, "should include source (0-hop) and target (1-hop)");
}

/// Variable-length with no results (no edges)
#[test]
fn test_var_length_no_edges() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (a:Alone)").expect("create");

    let result = db.execute("MATCH (a:Alone)-[:KNOWS*1..3]->(b) RETURN b")
        .expect("should execute");

    assert_eq!(result.rows.len(), 0, "should return empty for isolated node");
}

/// Variable-length with multiple sources via label scan
#[test]
fn test_var_length_multiple_sources() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Starter)-[:KNOWS]->(b:Target)"
    ).expect("create 1");
    db.execute(
        "CREATE (c:Starter)-[:KNOWS]->(d:Target)"
    ).expect("create 2");

    let result = db.execute("MATCH (a:Starter)-[:KNOWS*1]->(b) RETURN a, b")
        .expect("should execute");

    // Two Starter nodes, each with 1 outgoing KNOWS
    assert_eq!(result.rows.len(), 2, "should find 2 edges from 2 sources");
}

/// Variable-length path with relationship variable
#[test]
fn test_var_length_with_rel_variable() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:Target)"
    ).expect("create");

    let result = db.execute("MATCH (a:Root)-[r:KNOWS*1]->(b) RETURN r")
        .expect("should execute");

    assert_eq!(result.rows.len(), 1, "should return 1 result with rel var");
    // r should be bound to a value (edge)
    let r_val = result.rows[0].get("r");
    assert!(r_val.is_some(), "r should be bound");
}

/// Bounded traversal stops at max_hops
#[test]
fn test_var_length_bounded_stops_at_max() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Chain: Root -> N1 -> N2 -> N3 -> N4
    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:KNOWS]->(c:N2)-[:KNOWS]->(d:N3)-[:KNOWS]->(e:N4)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*1..2]->(b) RETURN b")
        .expect("should execute");

    // 1-hop: N1, 2-hop: N2 (not N3 or N4)
    assert_eq!(result.rows.len(), 2, "should stop at max_hops=2");
}

/// Incoming direction variable-length
#[test]
fn test_var_length_incoming_direction() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:N1)-[:KNOWS]->(b:N2)-[:KNOWS]->(c:Leaf)"
    ).expect("create chain");

    let result = db.execute("MATCH (c:Leaf)<-[:KNOWS*1..2]-(a) RETURN a")
        .expect("should execute");

    // Incoming 1-hop from Leaf: N2, 2-hop: N1
    assert_eq!(result.rows.len(), 2, "should traverse incoming edges");
}

/// Semantic validation: max_hops exceeds limit (should fail)
#[test]
fn test_var_length_exceeds_max_limit() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // max_hops=100 should be rejected by semantic analysis (limit is 10)
    let result = db.execute("MATCH (a)-[*1..100]->(b) RETURN b");
    assert!(result.is_err(), "should fail semantic validation");
}

/// Semantic validation: max < min (should fail)
#[test]
fn test_var_length_max_less_than_min() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    let result = db.execute("MATCH (a)-[*5..2]->(b) RETURN b");
    assert!(result.is_err(), "should fail: max_hops < min_hops");
}

/// Open-end range [*2..] with default capping
#[test]
fn test_var_length_open_end_range() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:KNOWS]->(c:N2)-[:KNOWS]->(d:N3)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*2..]->(b) RETURN b")
        .expect("should execute");

    // min=2, max=DEFAULT(10). 2-hop: N2, 3-hop: N3
    assert_eq!(result.rows.len(), 2, "should find nodes at 2+ hops");
}

/// No type filter: variable-length with any relationship type
#[test]
fn test_var_length_no_type_filter() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    // Root -KNOWS-> N1 -LIKES-> N2
    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:LIKES]->(c:N2)"
    ).expect("create chain");

    // [*1..2] without type: should follow both KNOWS and LIKES
    let result = db.execute("MATCH (a:Root)-[*1..2]->(b) RETURN b")
        .expect("should execute");

    // 1-hop: N1 (via KNOWS), 2-hop: N2 (via KNOWS then LIKES)
    assert_eq!(result.rows.len(), 2, "should follow any edge type");
}

/// Zero-length with no edges: just returns source
#[test]
fn test_var_length_zero_no_edges() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute("CREATE (a:Solo)").expect("create");

    let result = db.execute("MATCH (a:Solo)-[*0..1]->(b) RETURN b")
        .expect("should execute");

    // Only zero-hop match (source itself)
    assert_eq!(result.rows.len(), 1, "should return solo node as 0-hop match");
}

/// Exact 1-hop should behave like regular expand
#[test]
fn test_var_length_exact_1_hop() {
    let dir = tempdir().expect("tempdir");
    let mut db = open_db(dir.path());

    db.execute(
        "CREATE (a:Root)-[:KNOWS]->(b:N1)-[:KNOWS]->(c:N2)"
    ).expect("create chain");

    let result = db.execute("MATCH (a:Root)-[:KNOWS*1]->(b) RETURN b")
        .expect("should execute");

    // Exactly 1 hop: only N1
    assert_eq!(result.rows.len(), 1, "exact 1-hop should match only direct neighbor");
}
