package cypherlite

import (
	"os"
	"path/filepath"
	"testing"
)

func tempDBPath(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	return filepath.Join(dir, "test.cypher")
}

func TestOpen_Success(t *testing.T) {
	path := tempDBPath(t)
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open(%q) failed: %v", path, err)
	}
	if db == nil {
		t.Fatal("Open returned nil db without error")
	}
	defer db.Close()
}

func TestOpen_InvalidPath(t *testing.T) {
	// Opening a file in a non-existent directory should fail.
	_, err := Open("/nonexistent/directory/path/db.cypher")
	if err == nil {
		t.Fatal("Open with invalid path should return error")
	}
}

func TestClose_DoubleClose(t *testing.T) {
	path := tempDBPath(t)
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}

	// First close should succeed.
	err = db.Close()
	if err != nil {
		t.Fatalf("first Close failed: %v", err)
	}

	// Second close should not panic and should return an error.
	err = db.Close()
	if err == nil {
		t.Fatal("second Close should return error")
	}
}

func TestOpenWithConfig_Success(t *testing.T) {
	path := tempDBPath(t)
	db, err := OpenWithConfig(path, 4096, 64)
	if err != nil {
		t.Fatalf("OpenWithConfig failed: %v", err)
	}
	defer db.Close()
}

func TestOpen_CreatesFile(t *testing.T) {
	path := tempDBPath(t)
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	defer db.Close()

	// Check that the database file was created.
	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Fatalf("database file not created at %q", path)
	}
}

func TestDB_IsClosed(t *testing.T) {
	path := tempDBPath(t)
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}

	if db.IsClosed() {
		t.Fatal("db should not be closed right after Open")
	}

	db.Close()

	if !db.IsClosed() {
		t.Fatal("db should be closed after Close()")
	}
}
