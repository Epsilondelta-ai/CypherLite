// go_quickstart.go -- Demonstrates CypherLite usage from Go via CGo bindings.
//
// This is an ILLUSTRATIVE example showing the Go API surface. To run it,
// you must first build the CypherLite C static library and have the Go
// bindings package available:
//
//   1. Build the C static library:
//        cargo build -p cypherlite-ffi --release --all-features
//
//   2. Run from the bindings/go directory:
//        cd bindings/go
//        CGO_LDFLAGS="-L../../target/release -lcypherlite_ffi ..." go run ../../examples/go_quickstart.go
//
// Prerequisites:
//   - Go 1.21+
//   - Rust toolchain (to build the static library)
//   - C compiler (for CGo)

package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/example/cypherlite/bindings/go/cypherlite"
)

func main() {
	fmt.Println("=== CypherLite Go Quickstart ===")
	fmt.Println()

	// Print version and features
	fmt.Printf("Version:  %s\n", cypherlite.Version())
	fmt.Printf("Features: %s\n\n", cypherlite.Features())

	// Open a database in a temporary directory
	tmpDir, err := os.MkdirTemp("", "cypherlite-go-*")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmpDir)

	dbPath := filepath.Join(tmpDir, "quickstart.cyl")
	db, err := cypherlite.Open(dbPath)
	if err != nil {
		panic(fmt.Sprintf("failed to open database: %v", err))
	}
	defer db.Close()

	// -- CREATE nodes -------------------------------------------------------
	fmt.Println("1. Creating nodes...")
	if _, err := db.Execute("CREATE (a:Person {name: 'Alice', age: 30})"); err != nil {
		panic(err)
	}
	if _, err := db.Execute("CREATE (b:Person {name: 'Bob', age: 25})"); err != nil {
		panic(err)
	}
	fmt.Println("   Created Alice and Bob")
	fmt.Println()

	// -- CREATE relationship ------------------------------------------------
	fmt.Println("2. Creating relationship...")
	_, err = db.Execute(
		"MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) " +
			"CREATE (a)-[:KNOWS {since: 2023}]->(b)",
	)
	if err != nil {
		panic(err)
	}
	fmt.Println("   Alice -[:KNOWS]-> Bob")
	fmt.Println()

	// -- MATCH + RETURN: read data ------------------------------------------
	fmt.Println("3. Querying all persons...")
	result, err := db.Execute("MATCH (n:Person) RETURN n.name, n.age")
	if err != nil {
		panic(err)
	}
	for result.Next() {
		row := result.Row()
		name, _ := row.GetString("n.name")
		age, _ := row.GetInt64("n.age")
		fmt.Printf("   %s (age: %d)\n", name, age)
	}
	fmt.Println()

	// -- UPDATE with SET ----------------------------------------------------
	fmt.Println("4. Updating Bob's age...")
	_, err = db.Execute("MATCH (b:Person {name: 'Bob'}) SET b.age = 26")
	if err != nil {
		panic(err)
	}
	result, err = db.Execute("MATCH (b:Person {name: 'Bob'}) RETURN b.age")
	if err != nil {
		panic(err)
	}
	if result.Next() {
		row := result.Row()
		age, _ := row.GetInt64("b.age")
		fmt.Printf("   Bob's new age: %d\n", age)
	}
	fmt.Println()

	// -- Transaction example ------------------------------------------------
	fmt.Println("5. Transaction example...")
	tx, err := db.Begin()
	if err != nil {
		panic(err)
	}
	_, err = tx.Execute("CREATE (c:Person {name: 'Carol', age: 28})")
	if err != nil {
		tx.Rollback()
		panic(err)
	}
	if err := tx.Commit(); err != nil {
		panic(err)
	}
	fmt.Println("   Transaction committed")
	fmt.Println()

	// -- DELETE -------------------------------------------------------------
	fmt.Println("6. Deleting Carol...")
	_, err = db.Execute("MATCH (c:Person {name: 'Carol'}) DETACH DELETE c")
	if err != nil {
		panic(err)
	}
	fmt.Println("   Carol removed")
	fmt.Println()

	fmt.Println("=== Done! ===")
}
