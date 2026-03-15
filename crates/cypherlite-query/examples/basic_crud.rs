// basic_crud.rs -- Demonstrates basic CRUD operations with CypherLite.
//
// This example covers the fundamental database operations:
//   - CREATE: Insert nodes and relationships with properties
//   - MATCH + RETURN: Query nodes and relationships
//   - SET: Update existing node properties
//   - DELETE: Remove nodes (with DETACH DELETE for nodes with relationships)
//   - MERGE: Upsert pattern (create if not exists)
//
// Feature flags: Works with default features; --all-features enables temporal
// and plugin capabilities (not used in this example).
//
// Run:
//   cargo run -p cypherlite-query --example basic_crud --all-features

use cypherlite_core::{DatabaseConfig, SyncMode};
use cypherlite_query::api::CypherLite;

fn main() {
    // -- Setup: create a temporary database ---------------------------------
    let tmp_dir = tempfile::tempdir().expect("failed to create temp directory");
    let config = DatabaseConfig {
        path: tmp_dir.path().join("example.cyl"),
        wal_sync_mode: SyncMode::Normal,
        ..Default::default()
    };
    let mut db = CypherLite::open(config).expect("failed to open database");
    println!("=== CypherLite Basic CRUD Example ===\n");

    // -- 1. CREATE: Insert nodes with properties ----------------------------
    println!("1. Creating nodes...");
    db.execute("CREATE (a:Person {name: 'Alice', age: 30})")
        .expect("create Alice");
    db.execute("CREATE (b:Person {name: 'Bob', age: 25})")
        .expect("create Bob");
    db.execute("CREATE (c:Person {name: 'Carol', age: 35})")
        .expect("create Carol");
    println!("   Created 3 Person nodes: Alice, Bob, Carol\n");

    // -- 2. CREATE: Insert a connected graph in one statement ---------------
    // Using a single chain CREATE creates nodes with a relationship in one go.
    println!("2. Creating a connected graph with relationships...");
    db.execute(
        "CREATE (x:Team {name: 'Engineering'})-[:HAS_MEMBER]->(y:Employee {name: 'Dave', role: 'lead'})",
    )
    .expect("create team chain");
    println!("   Engineering -[:HAS_MEMBER]-> Dave\n");

    // -- 3. MATCH + RETURN: Query all persons -------------------------------
    println!("3. Querying all Person nodes...");
    let result = db
        .execute("MATCH (n:Person) RETURN n.name, n.age")
        .expect("match all persons");
    for row in &result.rows {
        let name: String = row.get_as("n.name").unwrap_or_default();
        let age: i64 = row.get_as("n.age").unwrap_or_default();
        println!("   {name} (age: {age})");
    }
    println!();

    // -- 4. MATCH: Query by label to read a relationship --------------------
    println!("4. Querying Team -> Employee relationship...");
    let result = db
        .execute("MATCH (t:Team)-[:HAS_MEMBER]->(e:Employee) RETURN t.name, e.name, e.role")
        .expect("match team members");
    for row in &result.rows {
        let team: String = row.get_as("t.name").unwrap_or_default();
        let emp: String = row.get_as("e.name").unwrap_or_default();
        let role: String = row.get_as("e.role").unwrap_or_default();
        println!("   Team '{team}' -> {emp} ({role})");
    }
    println!();

    // -- 5. SET: Update a node property ------------------------------------
    println!("5. Updating Bob's age from 25 to 26...");
    db.execute("MATCH (b:Person {name: 'Bob'}) SET b.age = 26")
        .expect("update Bob's age");
    let result = db
        .execute("MATCH (b:Person {name: 'Bob'}) RETURN b.age")
        .expect("verify update");
    let new_age: i64 = result.rows[0].get_as("b.age").unwrap_or_default();
    println!("   Bob's age is now: {new_age}\n");

    // -- 6. SET: Add a new property to an existing node --------------------
    println!("6. Adding 'email' property to Alice...");
    db.execute("MATCH (a:Person {name: 'Alice'}) SET a.email = 'alice@example.com'")
        .expect("set email");
    let result = db
        .execute("MATCH (a:Person {name: 'Alice'}) RETURN a.email")
        .expect("verify email");
    let email: String = result.rows[0].get_as("a.email").unwrap_or_default();
    println!("   Alice's email: {email}\n");

    // -- 7. DELETE: Remove Carol and her relationships ---------------------
    println!("7. Deleting Carol (DETACH DELETE removes node + edges)...");
    db.execute("MATCH (c:Person {name: 'Carol'}) DETACH DELETE c")
        .expect("delete Carol");
    let result = db
        .execute("MATCH (n:Person) RETURN n.name")
        .expect("count remaining");
    println!(
        "   Remaining persons: {} (Carol removed)",
        result.rows.len()
    );
    for row in &result.rows {
        let name: String = row.get_as("n.name").unwrap_or_default();
        println!("   - {name}");
    }
    println!();

    // -- 8. MERGE: Upsert pattern (create if not exists) -------------------
    println!("8. MERGE: Upsert 'Eve' (create if not exists)...");
    db.execute("MERGE (e:Person {name: 'Eve'}) ON CREATE SET e.age = 28")
        .expect("merge Eve");
    // Run again -- should not create a duplicate, and ON MATCH SET fires
    db.execute("MERGE (e:Person {name: 'Eve'}) ON MATCH SET e.age = 29")
        .expect("merge Eve again");
    let result = db
        .execute("MATCH (e:Person {name: 'Eve'}) RETURN e.age")
        .expect("verify merge");
    let age: i64 = result.rows[0].get_as("e.age").unwrap_or_default();
    println!("   Eve's age after MERGE ON MATCH SET: {age}\n");

    // -- Cleanup -----------------------------------------------------------
    // The temporary directory and database file are automatically removed
    // when `tmp_dir` goes out of scope.
    println!("=== Done! Database cleaned up automatically. ===");
}
