// Integration tests for Groups DD and EE: AT TIME / BETWEEN TIME for edges
//
// DD-T5: AT TIME filters edges during MATCH
// DD-T6: BETWEEN TIME filters edges during MATCH
// EE-T1: VarLengthExpand AT TIME temporal continuity
// EE-T2: VarLengthExpand BETWEEN TIME overlap
// EE-T3: Comprehensive integration tests

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::{Params, Value};
use tempfile::tempdir;

fn test_db(dir: &std::path::Path) -> CypherLite {
    let config = DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    CypherLite::open(config).expect("open")
}


// ======================================================================
// DD-T5: AT TIME filters edges during simple MATCH
// ======================================================================

#[test]
fn dd_t5_at_time_filters_edges_simple() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create two nodes at t=100
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:Start {name: 'A'})-[:LINK]->(b:End {name: 'B'})",
        params,
    )
    .expect("create");

    // Set edge _valid_from=500, _valid_to=1500
    db.execute("MATCH (a:Start)-[r:LINK]->(b:End) SET r._valid_from = 500, r._valid_to = 1500")
        .expect("set valid_from/to");

    // AT TIME 1000 (within validity): edge should be visible
    let result = db
        .execute("MATCH (a:Start)-[r:LINK]->(b:End) AT TIME 1000 RETURN a.name, b.name")
        .expect("query at 1000");
    assert_eq!(result.rows.len(), 1, "edge should be valid at t=1000");

    // AT TIME 2000 (after validity): edge should NOT be visible
    let result = db
        .execute("MATCH (a:Start)-[r:LINK]->(b:End) AT TIME 2000 RETURN a.name, b.name")
        .expect("query at 2000");
    assert_eq!(result.rows.len(), 0, "edge expired at t=2000");

    // AT TIME 300 (before validity): edge should NOT be visible
    let result = db
        .execute("MATCH (a:Start)-[r:LINK]->(b:End) AT TIME 300 RETURN a.name, b.name")
        .expect("query at 300");
    assert_eq!(result.rows.len(), 0, "edge not yet valid at t=300");
}

// ======================================================================
// DD-T5: Edge without _valid_from is always valid (backward compat)
// ======================================================================

#[test]
fn dd_t5_edge_without_valid_from_always_valid() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create edge without setting _valid_from/_valid_to explicitly.
    // Note: edge CREATE auto-injects _valid_from, so we need to check
    // if the auto-injected _valid_from makes it visible at the right time.
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:X {name: 'A'})-[:REL]->(b:Y {name: 'B'})",
        params,
    )
    .expect("create");

    // Edge was created at t=100, _valid_from should be 100 (auto-injected).
    // At t=200 (after creation), it should be visible.
    let result = db
        .execute("MATCH (a:X)-[r:REL]->(b:Y) AT TIME 200 RETURN a.name")
        .expect("query at 200");
    assert_eq!(result.rows.len(), 1, "edge with auto _valid_from should be visible after creation");

    // At t=50 (before creation), edge should NOT be visible.
    let result = db
        .execute("MATCH (a:X)-[r:REL]->(b:Y) AT TIME 50 RETURN a.name")
        .expect("query at 50");
    assert_eq!(result.rows.len(), 0, "edge should not be visible before _valid_from");
}

// ======================================================================
// DD-T6: BETWEEN TIME filters edges
// ======================================================================

#[test]
fn dd_t6_between_time_filters_edges() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create nodes and edge at t=1000 (within the BETWEEN range [1000,2000])
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(1000));
    db.execute_with_params(
        "CREATE (a:Start2 {name: 'A'})-[:CONN]->(b:End2 {name: 'B'})",
        params,
    )
    .expect("create");

    // Set edge validity window [500, 1500) -- overlaps [1000,2000]
    db.execute("MATCH (a:Start2)-[r:CONN]->(b:End2) SET r._valid_from = 500, r._valid_to = 1500")
        .expect("set");

    // BETWEEN TIME 1000 AND 2000 -- overlaps with [500, 1500)
    // Node created at t=1000 (in range), edge overlaps
    let result = db
        .execute("MATCH (a:Start2)-[r:CONN]->(b:End2) BETWEEN TIME 1000 AND 2000 RETURN a.name")
        .expect("between overlapping");
    assert!(!result.rows.is_empty(), "edge overlaps with query range (got {} rows)", result.rows.len());

    // BETWEEN TIME 2000 AND 3000 -- no overlap with [500, 1500)
    let result = db
        .execute("MATCH (a:Start2)-[r:CONN]->(b:End2) BETWEEN TIME 2000 AND 3000 RETURN a.name")
        .expect("between no overlap");
    assert_eq!(result.rows.len(), 0, "edge does not overlap with query range");
}

// ======================================================================
// EE-T1: VarLengthExpand AT TIME temporal continuity
// ======================================================================

#[test]
fn ee_t1_var_length_at_time_temporal_continuity() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create chain: A->B->C with different temporal windows
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:Chain {name: 'A'})-[:STEP]->(b:Chain {name: 'B'})",
        params,
    )
    .expect("create A->B");

    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (b2:Chain2 {name: 'B2'})-[:STEP]->(c:Chain2 {name: 'C'})",
        params,
    )
    .expect("create B2->C");

    // Set A->B valid [100, 2000) -- use unique labels so we target the right edge
    db.execute("MATCH (a:Chain)-[r:STEP]->(b:Chain) SET r._valid_from = 100, r._valid_to = 2000")
        .expect("set A->B validity");

    // Set B2->C valid [1500, 3000)
    db.execute("MATCH (b:Chain2)-[r:STEP]->(c:Chain2) SET r._valid_from = 1500, r._valid_to = 3000")
        .expect("set B->C validity");

    // AT TIME 1000: A->B is valid but this is an isolated chain test
    // With A->B valid at t=1000, variable-length should reach B
    let result = db
        .execute("MATCH (a:Chain {name: 'A'})-[*1..2]->(x) AT TIME 1000 RETURN x.name")
        .expect("var length at 1000");
    assert_eq!(result.rows.len(), 1, "only A->B is valid at t=1000");
}

// ======================================================================
// EE-T1: All edges in path must be valid at AT TIME
// ======================================================================

#[test]
fn ee_t1_all_edges_must_be_valid_in_path() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create a 3-node chain using unique labels: P1->P2->P3
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(50));
    db.execute_with_params(
        "CREATE (a:P1 {name: 'A'})-[:HOP]->(b:P2 {name: 'B'})-[:HOP]->(c:P3 {name: 'C'})",
        params,
    )
    .expect("create chain");

    // Set edge A->B valid [100, 2000)
    db.execute("MATCH (a:P1)-[r:HOP]->(b:P2) SET r._valid_from = 100, r._valid_to = 2000")
        .expect("set A->B");

    // Set edge B->C valid [500, 1500)
    db.execute("MATCH (b:P2)-[r:HOP]->(c:P3) SET r._valid_from = 500, r._valid_to = 1500")
        .expect("set B->C");

    // AT TIME 1000: both edges valid -> path A->B->C exists
    let result = db
        .execute("MATCH (a:P1 {name: 'A'})-[*1..2]->(x) AT TIME 1000 RETURN x.name")
        .expect("at 1000");
    assert_eq!(result.rows.len(), 2, "both hops valid at t=1000: A->B and A->B->C");

    // AT TIME 1800: A->B valid, but B->C expired -> only A->B
    let result = db
        .execute("MATCH (a:P1 {name: 'A'})-[*1..2]->(x) AT TIME 1800 RETURN x.name")
        .expect("at 1800");
    assert_eq!(result.rows.len(), 1, "only A->B valid at t=1800");

    // AT TIME 50: neither edge valid -> no results
    let result = db
        .execute("MATCH (a:P1 {name: 'A'})-[*1..2]->(x) AT TIME 50 RETURN x.name")
        .expect("at 50");
    assert_eq!(result.rows.len(), 0, "no edges valid at t=50");
}

// ======================================================================
// EE-T2: VarLengthExpand BETWEEN TIME overlap
// ======================================================================

// Debug: check that BETWEEN TIME works for a simple case
#[test]
fn ee_t2_between_time_simple_node_edge() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create at t=100
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(100));
    db.execute_with_params(
        "CREATE (a:BT {name: 'A'})-[:BT_LINK]->(b:BT {name: 'B'})",
        params,
    )
    .expect("create");

    // Without temporal: should find the edge
    let result = db
        .execute("MATCH (a:BT {name: 'A'})-[r:BT_LINK]->(b:BT) RETURN a.name")
        .expect("no temporal");
    assert_eq!(result.rows.len(), 1, "edge exists without temporal filter");

    // BETWEEN TIME 50 AND 200: node was created at t=100, _created_at=100 is in [50,200]
    let result = db
        .execute("MATCH (a:BT {name: 'A'})-[r:BT_LINK]->(b:BT) BETWEEN TIME 50 AND 200 RETURN a.name")
        .expect("between 50 200");
    assert!(!result.rows.is_empty(), "node and edge created at t=100 should be in [50,200] (got {} rows)", result.rows.len());
}

#[test]
fn ee_t2_var_length_between_time_overlap() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create chain Q1->Q2->Q3 at t=800 (within the BETWEEN range [800,1200])
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(800));
    db.execute_with_params(
        "CREATE (a:Q1 {name: 'A'})-[:LINK]->(b:Q2 {name: 'B'})-[:LINK]->(c:Q3 {name: 'C'})",
        params,
    )
    .expect("create");

    // Edge Q1->Q2 valid [100, 1000) -- overlaps [800,1200]
    db.execute("MATCH (a:Q1)-[r:LINK]->(b:Q2) SET r._valid_from = 100, r._valid_to = 1000")
        .expect("set Q1->Q2");

    // Edge Q2->Q3 valid [500, 2000) -- overlaps [800,1200]
    db.execute("MATCH (b:Q2)-[r:LINK]->(c:Q3) SET r._valid_from = 500, r._valid_to = 2000")
        .expect("set Q2->Q3");

    // BETWEEN TIME 800 AND 1200: both edges overlap, node was created at 800 in range
    let result = db
        .execute("MATCH (a:Q1 {name: 'A'})-[*1..2]->(x) BETWEEN TIME 800 AND 1200 RETURN x.name")
        .expect("between 800 1200");
    assert!(result.rows.len() >= 2, "both edges should overlap [800,1200] (got {} rows)", result.rows.len());

    // BETWEEN TIME 1100 AND 1500: Q1->Q2 does NOT overlap (ends at 1000)
    // But even if Q2->Q3 overlaps, the first hop Q1->Q2 fails
    let result = db
        .execute("MATCH (a:Q1 {name: 'A'})-[*1..2]->(x) BETWEEN TIME 1100 AND 1500 RETURN x.name")
        .expect("between 1100 1500");
    assert_eq!(result.rows.len(), 0, "Q1->Q2 not valid in [1100,1500], breaks path");
}

// ======================================================================
// EE-T3: Mixed edges -- some valid, some not
// ======================================================================

#[test]
fn ee_t3_mixed_edge_validity() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    // Create star: Center -> A, Center -> B
    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(50));
    db.execute_with_params(
        "CREATE (c:Hub {name: 'Center'})-[:RAY]->(a:Spoke {name: 'A'})",
        params.clone(),
    )
    .expect("create center->A");

    db.execute_with_params(
        "CREATE (c2:Hub2 {name: 'Center2'})-[:RAY]->(b:Spoke2 {name: 'B'})",
        params,
    )
    .expect("create center2->B");

    // Center->A valid [100, 500)
    db.execute("MATCH (c:Hub)-[r:RAY]->(a:Spoke) SET r._valid_from = 100, r._valid_to = 500")
        .expect("set center->A");

    // Center2->B valid [400, 900)
    db.execute("MATCH (c:Hub2)-[r:RAY]->(b:Spoke2) SET r._valid_from = 400, r._valid_to = 900")
        .expect("set center2->B");

    // AT TIME 300: only Center->A should be visible (Hub label)
    let result = db
        .execute("MATCH (c:Hub)-[r:RAY]->(x) AT TIME 300 RETURN x.name")
        .expect("at 300");
    assert_eq!(result.rows.len(), 1, "only Center->A valid at 300");

    // AT TIME 600: Center->A expired, only Center2->B visible (Hub2 label)
    let result = db
        .execute("MATCH (c:Hub2)-[r:RAY]->(x) AT TIME 600 RETURN x.name")
        .expect("at 600");
    assert_eq!(result.rows.len(), 1, "only Center2->B valid at 600");

    // AT TIME 1000: both expired
    let result = db
        .execute("MATCH (c:Hub)-[r:RAY]->(x) AT TIME 1000 RETURN x.name")
        .expect("at 1000 hub");
    assert_eq!(result.rows.len(), 0, "Center->A expired at 1000");

    let result = db
        .execute("MATCH (c:Hub2)-[r:RAY]->(x) AT TIME 1000 RETURN x.name")
        .expect("at 1000 hub2");
    assert_eq!(result.rows.len(), 0, "Center2->B expired at 1000");
}

// ======================================================================
// EE-T3: Edge with only _valid_from (no _valid_to) is open-ended
// ======================================================================

#[test]
fn ee_t3_open_ended_edge_validity() {
    let dir = tempdir().expect("tmpdir");
    let mut db = test_db(dir.path());

    let mut params = Params::new();
    params.insert("__query_start_ms__".to_string(), Value::Int64(50));
    db.execute_with_params(
        "CREATE (a:Open {name: 'A'})-[:OPEN]->(b:Open {name: 'B'})",
        params,
    )
    .expect("create");

    // Set only _valid_from (no _valid_to) -> open-ended
    db.execute("MATCH (a:Open {name: 'A'})-[r:OPEN]->(b:Open) SET r._valid_from = 500")
        .expect("set valid_from only");

    // AT TIME 300: before _valid_from -> not valid
    let result = db
        .execute("MATCH (a:Open {name: 'A'})-[r:OPEN]->(b:Open) AT TIME 300 RETURN a.name")
        .expect("at 300");
    assert_eq!(result.rows.len(), 0, "before _valid_from");

    // AT TIME 1000: after _valid_from with no _valid_to -> valid forever
    let result = db
        .execute("MATCH (a:Open {name: 'A'})-[r:OPEN]->(b:Open) AT TIME 1000 RETURN a.name")
        .expect("at 1000");
    assert_eq!(result.rows.len(), 1, "open-ended edge valid after _valid_from");

    // AT TIME 999999: still valid
    let result = db
        .execute("MATCH (a:Open {name: 'A'})-[r:OPEN]->(b:Open) AT TIME 999999 RETURN a.name")
        .expect("at far future");
    assert_eq!(result.rows.len(), 1, "open-ended edge valid far in future");
}
