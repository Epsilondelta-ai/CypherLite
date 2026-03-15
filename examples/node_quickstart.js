// node_quickstart.js -- Demonstrates CypherLite usage from Node.js via napi-rs bindings.
//
// This is an ILLUSTRATIVE example showing the Node.js API surface. To run it,
// you must first build the native addon:
//
//   1. Build the napi-rs addon:
//        cd crates/cypherlite-node
//        npx napi build --release
//
//   2. Run:
//        node examples/node_quickstart.js
//
// Prerequisites:
//   - Node.js 18+ (with N-API v9 support)
//   - Rust toolchain (to build the native addon)
//   - npx / @napi-rs/cli

const os = require("os");
const path = require("path");
const fs = require("fs");
const { open, version, features } = require("cypherlite");

async function main() {
  console.log("=== CypherLite Node.js Quickstart ===\n");

  // Print version and features
  console.log(`Version:  ${version()}`);
  console.log(`Features: ${features()}\n`);

  // Open a database in a temporary directory
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "cypherlite-node-"));
  const dbPath = path.join(tmpDir, "quickstart.cyl");
  const db = open(dbPath);

  try {
    // -- CREATE nodes -------------------------------------------------------
    console.log("1. Creating nodes...");
    db.execute("CREATE (a:Person {name: 'Alice', age: 30})");
    db.execute("CREATE (b:Person {name: 'Bob', age: 25})");
    console.log("   Created Alice and Bob\n");

    // -- CREATE relationship ------------------------------------------------
    console.log("2. Creating relationship...");
    db.execute(
      "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) " +
        "CREATE (a)-[:KNOWS {since: 2023}]->(b)"
    );
    console.log("   Alice -[:KNOWS]-> Bob\n");

    // -- MATCH + RETURN: read data ------------------------------------------
    console.log("3. Querying all persons...");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    for (const row of result) {
      console.log(`   ${row["n.name"]} (age: ${row["n.age"]})`);
    }
    console.log();

    // -- Parameterized query ------------------------------------------------
    console.log("4. Parameterized query: find person by name...");
    const found = db.execute(
      "MATCH (n:Person {name: $name}) RETURN n.name, n.age",
      { name: "Alice" }
    );
    for (const row of found) {
      console.log(`   Found: ${row["n.name"]}, age ${row["n.age"]}`);
    }
    console.log();

    // -- UPDATE with SET ----------------------------------------------------
    console.log("5. Updating Bob's age...");
    db.execute("MATCH (b:Person {name: 'Bob'}) SET b.age = 26");
    const updated = db.execute(
      "MATCH (b:Person {name: 'Bob'}) RETURN b.age"
    );
    for (const row of updated) {
      console.log(`   Bob's new age: ${row["b.age"]}`);
    }
    console.log();

    // -- Transaction example ------------------------------------------------
    console.log("6. Transaction example...");
    const tx = db.begin();
    tx.execute("CREATE (c:Person {name: 'Carol', age: 28})");
    tx.commit();
    console.log("   Transaction committed\n");

    // -- DELETE -------------------------------------------------------------
    console.log("7. Deleting Carol...");
    db.execute("MATCH (c:Person {name: 'Carol'}) DETACH DELETE c");
    console.log("   Carol removed\n");
  } finally {
    // -- Cleanup ------------------------------------------------------------
    db.close();
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }

  console.log("=== Done! ===");
}

main().catch(console.error);
