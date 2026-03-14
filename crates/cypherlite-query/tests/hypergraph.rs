// Integration tests for Hypergraph (Phase 7c+7d+7e)
//
// PP-003: Hyperedge creation, querying, and traversal
// NN-001: TemporalRef lazy resolution via VersionStore
//
// NOTE: :INVOLVES virtual relationship expansion is tested at the unit level
// in executor/operators/expand.rs. Integration tests focus on CREATE HYPEREDGE
// and MATCH HYPEREDGE through the full API pipeline.

#![cfg(feature = "hypergraph")]

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;
use tempfile::tempdir;

fn test_config(dir: &std::path::Path) -> DatabaseConfig {
    DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    }
}

// PP-003: CREATE HYPEREDGE basic with single node
#[test]
fn test_create_hyperedge_single_source() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:HESrc {name: 'Alice'})")
        .expect("create");

    // MATCH single node, create hyperedge with it as source
    db.execute("MATCH (a:HESrc) CREATE HYPEREDGE (h:Solo) FROM (a) TO ()")
        .expect("create hyperedge");

    // Verify hyperedge exists
    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 1, "should find one hyperedge");
}

// PP-003: MATCH HYPEREDGE scan -- create multiple hyperedges, verify count
#[test]
fn test_match_hyperedge_scan_multiple() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:ScanX {name: 'A'})").expect("a");
    db.execute("CREATE (b:ScanY {name: 'B'})").expect("b");

    db.execute("MATCH (a:ScanX) CREATE HYPEREDGE (h1:TypeA) FROM (a) TO ()")
        .expect("he1");
    db.execute("MATCH (b:ScanY) CREATE HYPEREDGE (h2:TypeB) FROM (b) TO ()")
        .expect("he2");

    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 2, "should find 2 hyperedges");
}

// PP-003: Empty FROM/TO participant lists
#[test]
fn test_create_hyperedge_empty_participants() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE HYPEREDGE (h:Empty) FROM () TO ()")
        .expect("empty hyperedge");

    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 1, "should create one empty hyperedge");
}

// PP-003: Multiple hyperedges with different types
#[test]
fn test_multiple_hyperedge_types() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.execute("CREATE (a:MultiSrc {name: 'A'})").expect("a");

    db.execute("MATCH (a:MultiSrc) CREATE HYPEREDGE (h1:Meeting) FROM (a) TO ()")
        .expect("meeting");
    db.execute("MATCH (a:MultiSrc) CREATE HYPEREDGE (h2:Call) FROM (a) TO ()")
        .expect("call");
    db.execute("MATCH (a:MultiSrc) CREATE HYPEREDGE (h3:Email) FROM (a) TO ()")
        .expect("email");

    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("all");
    assert_eq!(result.rows.len(), 3, "should have 3 hyperedges total");
}

// PP-003: Hyperedge with two nodes via relationship chain
#[test]
fn test_hyperedge_two_participants_via_chain() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create a chain: a->b
    db.execute("CREATE (a:TwoA {name: 'start'})-[:NEXT]->(b:TwoB {name: 'end'})")
        .expect("chain");

    // MATCH a->b, create hyperedge FROM (a) TO (b)
    db.execute("MATCH (a:TwoA)-[:NEXT]->(b:TwoB) CREATE HYPEREDGE (h:Step) FROM (a) TO (b)")
        .expect("step hyperedge");

    // Verify existence
    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 1, "should have 1 hyperedge");
}

// NN-001: TemporalRef in hyperedge -- create with AT TIME syntax
#[test]
fn test_temporal_ref_hyperedge_creation() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create two nodes
    db.execute("CREATE (a:TempSrc {name: 'Alice'})-[:KNOWS]->(b:TempTgt {name: 'target'})")
        .expect("create chain");

    // Create hyperedge with temporal reference
    db.execute(
        "MATCH (a:TempSrc)-[:KNOWS]->(b:TempTgt) CREATE HYPEREDGE (h:Snap) FROM (a AT TIME 100) TO (b)",
    )
    .expect("temporal hyperedge");

    // Verify the hyperedge was created
    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 1, "temporal hyperedge should exist");
}

// PP-003: Standalone CREATE HYPEREDGE with no source MATCH
#[test]
fn test_create_hyperedge_standalone() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create hyperedge with empty participants (no MATCH needed)
    db.execute("CREATE HYPEREDGE (h:Standalone) FROM () TO ()")
        .expect("standalone");

    let result = db.execute("MATCH HYPEREDGE (h) RETURN h").expect("match");
    assert_eq!(result.rows.len(), 1);
}
