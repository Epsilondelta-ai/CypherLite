package cypherlite

import (
	"testing"
)

func openTestDB(t *testing.T) *DB {
	t.Helper()
	path := tempDBPath(t)
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func TestExecute_CreateAndMatch(t *testing.T) {
	db := openTestDB(t)

	// CREATE a node.
	res, err := db.Execute("CREATE (n:Person {name: 'Alice'}) RETURN n.name")
	if err != nil {
		t.Fatalf("Execute CREATE failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 1 {
		t.Fatalf("expected 1 row, got %d", res.RowCount())
	}
}

func TestExecute_InvalidQuery(t *testing.T) {
	db := openTestDB(t)

	_, err := db.Execute("THIS IS NOT VALID CYPHER !!!")
	if err == nil {
		t.Fatal("Execute with invalid query should return error")
	}
}

func TestExecute_ClosedDB(t *testing.T) {
	db := openTestDB(t)
	db.Close()

	_, err := db.Execute("MATCH (n) RETURN n")
	if err == nil {
		t.Fatal("Execute on closed db should return error")
	}
}

func TestExecuteWithParams_String(t *testing.T) {
	db := openTestDB(t)

	params := map[string]interface{}{
		"name": "Bob",
	}
	res, err := db.ExecuteWithParams(
		"CREATE (n:Person {name: $name}) RETURN n.name",
		params,
	)
	if err != nil {
		t.Fatalf("ExecuteWithParams failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 1 {
		t.Fatalf("expected 1 row, got %d", res.RowCount())
	}
}

func TestExecuteWithParams_MultipleTypes(t *testing.T) {
	db := openTestDB(t)

	params := map[string]interface{}{
		"name": "Charlie",
		"age":  int64(30),
	}
	res, err := db.ExecuteWithParams(
		"CREATE (n:Person {name: $name, age: $age}) RETURN n.name, n.age",
		params,
	)
	if err != nil {
		t.Fatalf("ExecuteWithParams failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 1 {
		t.Fatalf("expected 1 row, got %d", res.RowCount())
	}
}

func TestExecuteWithParams_UnsupportedType(t *testing.T) {
	db := openTestDB(t)

	params := map[string]interface{}{
		"data": struct{}{}, // unsupported type
	}
	_, err := db.ExecuteWithParams(
		"CREATE (n:Test {data: $data}) RETURN n",
		params,
	)
	if err == nil {
		t.Fatal("ExecuteWithParams with unsupported type should return error")
	}
}
