// knowledge_graph.rs -- Demonstrates a GraphRAG / knowledge graph use case.
//
// This example builds a small knowledge graph of technical concepts and their
// relationships, then runs various queries that are common in Retrieval-Augmented
// Generation (RAG) pipelines:
//   - Entity creation with typed labels and rich properties
//   - Relationship traversal (direct and multi-hop)
//   - Variable-length path queries for discovering transitive connections
//   - Aggregation with WITH clause
//   - Pattern matching with WHERE filters
//   - Property index for fast lookups
//
// Feature flags: Uses temporal-core (default) features. Run with --all-features
// to enable the full feature set.
//
// Run:
//   cargo run -p cypherlite-query --example knowledge_graph --all-features

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;

fn main() {
    // -- Setup --------------------------------------------------------------
    let tmp_dir = tempfile::tempdir().expect("failed to create temp directory");
    let config = DatabaseConfig {
        path: tmp_dir.path().join("knowledge.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("failed to open database");
    println!("=== CypherLite Knowledge Graph Example ===\n");

    // -- 1. Build the knowledge graph using single-chain CREATE -------------
    // Each chain creates connected nodes in one statement, ensuring
    // relationships are properly established.
    println!("1. Building knowledge graph...\n");

    // Core concept chain: RAG -> Knowledge Graph -> Graph Database
    db.execute(
        "CREATE (r:AI {name: 'RAG', importance: 9})\
         -[:USES]->\
         (kg:AI {name: 'Knowledge Graph', importance: 9})\
         -[:IS_A]->\
         (gdb:DB {name: 'Graph Database', importance: 9})",
    )
    .expect("create RAG chain");

    // Graph Database -> Property Graph -> Index -> B-Tree
    db.execute(
        "CREATE (gdb:DB {name: 'Property Graph', importance: 8})\
         -[:USES]->\
         (idx:DS {name: 'Index', importance: 7})\
         -[:IMPLEMENTED_BY]->\
         (bt:DS {name: 'B-Tree', importance: 6})",
    )
    .expect("create storage chain");

    // Query language chain: Cypher -> Pattern Matching
    db.execute(
        "CREATE (cy:QL {name: 'Cypher', importance: 8})\
         -[:USES]->\
         (pm:QL {name: 'Pattern Matching', importance: 7})",
    )
    .expect("create cypher chain");

    // Traversal chain: Traversal -> BFS, Traversal -> DFS
    db.execute(
        "CREATE (tr:Algo {name: 'Traversal', importance: 8})\
         -[:INCLUDES]->\
         (bfs:Algo {name: 'BFS', importance: 6})",
    )
    .expect("create traversal-bfs chain");
    db.execute(
        "CREATE (tr:Algo {name: 'Traversal2', importance: 8})\
         -[:INCLUDES]->\
         (dfs:Algo {name: 'DFS', importance: 6})",
    )
    .expect("create traversal-dfs chain");

    // Researcher and engineer nodes
    db.execute("CREATE (a:Researcher {name: 'Alice', field: 'AI'})")
        .expect("create Alice");
    db.execute("CREATE (b:Engineer {name: 'Bob', field: 'databases'})")
        .expect("create Bob");

    println!("   Created concept nodes (AI, DB, DS, QL, Algo labels)");
    println!("   Created relationship chains between concepts");
    println!("   Created Researcher and Engineer nodes\n");

    // -- 2. Direct relationship query: Cypher -> Pattern Matching -----------
    println!("2. What does Cypher use? (single-hop traversal)");
    let result = db
        .execute("MATCH (c:QL {name: 'Cypher'})-[:USES]->(target) RETURN target.name")
        .expect("cypher uses query");
    for row in &result.rows {
        let name: String = row.get_as("target.name").unwrap_or_default();
        println!("   Cypher -> {name}");
    }
    println!();

    // -- 3. Variable-length paths: RAG -> ... all reachable concepts --------
    println!("3. Variable-length paths: All concepts reachable from 'RAG' (1..3 hops)");
    let result = db
        .execute("MATCH (start:AI {name: 'RAG'})-[*1..3]->(related) RETURN related.name")
        .expect("var-length traversal");
    for row in &result.rows {
        let name: String = row.get_as("related.name").unwrap_or_default();
        println!("   -> {name}");
    }
    println!();

    // -- 4. Filtered query: High-importance concepts in DB domain -----------
    println!("4. High-importance DB concepts (importance >= 8):");
    let result = db
        .execute("MATCH (c:DB) WHERE c.importance >= 8 RETURN c.name, c.importance")
        .expect("filtered query");
    for row in &result.rows {
        let name: String = row.get_as("c.name").unwrap_or_default();
        let imp: i64 = row.get_as("c.importance").unwrap_or_default();
        println!("   {name} (importance: {imp})");
    }
    println!();

    // -- 5. Aggregation: Count concepts per label using WITH ----------------
    println!("5. Count of AI concepts:");
    let result = db
        .execute("MATCH (c:AI) WITH count(c) AS cnt RETURN cnt")
        .expect("count AI");
    for row in &result.rows {
        let cnt: i64 = row.get_as("cnt").unwrap_or_default();
        println!("   AI concepts: {cnt}");
    }
    let result = db
        .execute("MATCH (c:Algo) WITH count(c) AS cnt RETURN cnt")
        .expect("count Algo");
    for row in &result.rows {
        let cnt: i64 = row.get_as("cnt").unwrap_or_default();
        println!("   Algo concepts: {cnt}");
    }
    println!();

    // -- 6. Index for fast lookup -------------------------------------------
    println!("6. Creating index on :AI(name) for fast lookups...");
    db.execute("CREATE INDEX idx_ai_name ON :AI(name)")
        .expect("create index");
    // Queries filtering on AI.name now use the index automatically
    let result = db
        .execute("MATCH (c:AI {name: 'RAG'}) RETURN c.name, c.importance")
        .expect("indexed lookup");
    for row in &result.rows {
        let name: String = row.get_as("c.name").unwrap_or_default();
        let imp: i64 = row.get_as("c.importance").unwrap_or_default();
        println!("   Found via index: {name} (importance: {imp})");
    }
    println!();

    // -- 7. Multi-hop: Knowledge Graph -> Graph Database (IS_A path) --------
    println!("7. Traversal: Knowledge Graph -[:IS_A]-> Graph Database");
    let result = db
        .execute("MATCH (kg:AI {name: 'Knowledge Graph'})-[:IS_A]->(target:DB) RETURN target.name")
        .expect("is_a traversal");
    for row in &result.rows {
        let name: String = row.get_as("target.name").unwrap_or_default();
        println!("   Knowledge Graph IS_A {name}");
    }
    println!();

    // -- 8. OPTIONAL MATCH: Researchers and their optional fields -----------
    println!("8. Researchers (OPTIONAL MATCH for outgoing edges):");
    let result = db
        .execute("MATCH (r:Researcher) RETURN r.name, r.field")
        .expect("researchers");
    for row in &result.rows {
        let name: String = row.get_as("r.name").unwrap_or_default();
        let field: String = row.get_as("r.field").unwrap_or_default();
        println!("   {name} - field: {field}");
    }
    let result = db
        .execute("MATCH (e:Engineer) RETURN e.name, e.field")
        .expect("engineers");
    for row in &result.rows {
        let name: String = row.get_as("e.name").unwrap_or_default();
        let field: String = row.get_as("e.field").unwrap_or_default();
        println!("   {name} - field: {field}");
    }
    println!();

    // -- Cleanup -----------------------------------------------------------
    println!("=== Done! Knowledge graph cleaned up automatically. ===");
}
