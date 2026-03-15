package cypherlite

import (
	"testing"
)

func TestResult_Columns(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:Person {name: 'Alice', age: 30}) RETURN n.name, n.age")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	cols := res.Columns()
	if len(cols) != 2 {
		t.Fatalf("expected 2 columns, got %d", len(cols))
	}
	// Column order may vary; check that both names are present.
	colSet := map[string]bool{}
	for _, c := range cols {
		colSet[c] = true
	}
	if !colSet["n.name"] {
		t.Fatalf("expected column 'n.name' in %v", cols)
	}
	if !colSet["n.age"] {
		t.Fatalf("expected column 'n.age' in %v", cols)
	}
}

func TestResult_RowCount(t *testing.T) {
	db := openTestDB(t)

	// Create multiple nodes.
	_, err := db.Execute("CREATE (a:Animal {name: 'Cat'})")
	if err != nil {
		t.Fatalf("CREATE Cat failed: %v", err)
	}
	_, err = db.Execute("CREATE (b:Animal {name: 'Dog'})")
	if err != nil {
		t.Fatalf("CREATE Dog failed: %v", err)
	}

	res, err := db.Execute("MATCH (n:Animal) RETURN n.name")
	if err != nil {
		t.Fatalf("MATCH failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 2 {
		t.Fatalf("expected 2 rows, got %d", res.RowCount())
	}
}

func TestResult_Row_Get(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:Person {name: 'Alice'}) RETURN n.name")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	if row == nil {
		t.Fatal("Row(0) returned nil")
	}

	val := row.Get(0)
	strVal, ok := val.(string)
	if !ok {
		t.Fatalf("expected string, got %T: %v", val, val)
	}
	if strVal != "Alice" {
		t.Fatalf("expected %q, got %q", "Alice", strVal)
	}
}

func TestResult_Row_GetByName(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:Person {name: 'Bob'}) RETURN n.name")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	if row == nil {
		t.Fatal("Row(0) returned nil")
	}

	val, err := row.GetByName("n.name")
	if err != nil {
		t.Fatalf("GetByName failed: %v", err)
	}
	strVal, ok := val.(string)
	if !ok {
		t.Fatalf("expected string, got %T: %v", val, val)
	}
	if strVal != "Bob" {
		t.Fatalf("expected %q, got %q", "Bob", strVal)
	}
}

func TestResult_Row_OutOfBounds(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:Person {name: 'Alice'}) RETURN n.name")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(99)
	if row != nil {
		t.Fatal("Row(99) should return nil for out-of-bounds index")
	}
}

func TestResult_EmptyResult(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("MATCH (n:NonExistent) RETURN n")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 0 {
		t.Fatalf("expected 0 rows, got %d", res.RowCount())
	}
}

func TestResult_DoubleClose(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:Person {name: 'Test'}) RETURN n.name")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}

	res.Close()
	res.Close() // Should not panic.
}
