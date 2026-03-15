package cypherlite

import (
	"testing"
)

func TestTx_BeginAndCommit(t *testing.T) {
	db := openTestDB(t)

	tx, err := db.Begin()
	if err != nil {
		t.Fatalf("Begin failed: %v", err)
	}

	// Execute within transaction.
	res, err := tx.Execute("CREATE (n:Person {name: 'TxAlice'}) RETURN n.name")
	if err != nil {
		t.Fatalf("tx.Execute failed: %v", err)
	}
	res.Close()

	err = tx.Commit()
	if err != nil {
		t.Fatalf("Commit failed: %v", err)
	}

	// Verify data persisted after commit.
	res2, err := db.Execute("MATCH (n:Person {name: 'TxAlice'}) RETURN n.name")
	if err != nil {
		t.Fatalf("MATCH after commit failed: %v", err)
	}
	defer res2.Close()

	if res2.RowCount() != 1 {
		t.Fatalf("expected 1 row after commit, got %d", res2.RowCount())
	}
}

func TestTx_BeginAndRollback(t *testing.T) {
	db := openTestDB(t)

	tx, err := db.Begin()
	if err != nil {
		t.Fatalf("Begin failed: %v", err)
	}

	res, err := tx.Execute("CREATE (n:Person {name: 'TxRollback'}) RETURN n.name")
	if err != nil {
		t.Fatalf("tx.Execute failed: %v", err)
	}
	res.Close()

	err = tx.Rollback()
	if err != nil {
		t.Fatalf("Rollback failed: %v", err)
	}
}

func TestTx_ExecuteAfterCommit(t *testing.T) {
	db := openTestDB(t)

	tx, err := db.Begin()
	if err != nil {
		t.Fatalf("Begin failed: %v", err)
	}

	err = tx.Commit()
	if err != nil {
		t.Fatalf("Commit failed: %v", err)
	}

	// Execute after commit should fail.
	_, err = tx.Execute("CREATE (n:Test) RETURN n")
	if err == nil {
		t.Fatal("Execute after Commit should return error")
	}
}

func TestTx_DoubleCommit(t *testing.T) {
	db := openTestDB(t)

	tx, err := db.Begin()
	if err != nil {
		t.Fatalf("Begin failed: %v", err)
	}

	err = tx.Commit()
	if err != nil {
		t.Fatalf("first Commit failed: %v", err)
	}

	err = tx.Commit()
	if err == nil {
		t.Fatal("second Commit should return error")
	}
}

func TestTx_BeginOnClosedDB(t *testing.T) {
	db := openTestDB(t)
	db.Close()

	_, err := db.Begin()
	if err == nil {
		t.Fatal("Begin on closed db should return error")
	}
}

func TestTx_ExecuteWithParams(t *testing.T) {
	db := openTestDB(t)

	tx, err := db.Begin()
	if err != nil {
		t.Fatalf("Begin failed: %v", err)
	}

	params := map[string]interface{}{
		"name": "ParamTx",
	}
	res, err := tx.ExecuteWithParams(
		"CREATE (n:Person {name: $name}) RETURN n.name",
		params,
	)
	if err != nil {
		t.Fatalf("tx.ExecuteWithParams failed: %v", err)
	}
	defer res.Close()

	if res.RowCount() != 1 {
		t.Fatalf("expected 1 row, got %d", res.RowCount())
	}

	err = tx.Commit()
	if err != nil {
		t.Fatalf("Commit failed: %v", err)
	}
}
