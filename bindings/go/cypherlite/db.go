package cypherlite

// #include "cypherlite.h"
// #include <stdlib.h>
import "C"
import (
	"runtime"
	"sync"
	"unsafe"
)

// DB represents an open CypherLite database.
type DB struct {
	ptr *C.CylDb
	mu  sync.Mutex
}

// Open opens a CypherLite database at the given file path with default settings.
// The caller must call Close() when done.
func Open(path string) (*DB, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var errCode C.CylError
	ptr := C.cyl_db_open(cPath, &errCode)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	db := &DB{ptr: ptr}
	runtime.SetFinalizer(db, func(d *DB) {
		d.Close()
	})
	return db, nil
}

// OpenWithConfig opens a CypherLite database with explicit page size and cache capacity.
func OpenWithConfig(path string, pageSize, cacheCapacity uint32) (*DB, error) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var errCode C.CylError
	ptr := C.cyl_db_open_with_config(
		cPath,
		C.uint32_t(pageSize),
		C.uint32_t(cacheCapacity),
		&errCode,
	)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	db := &DB{ptr: ptr}
	runtime.SetFinalizer(db, func(d *DB) {
		d.Close()
	})
	return db, nil
}

// Close closes the database and frees the underlying resources.
// Returns ErrClosed if the database is already closed.
func (db *DB) Close() error {
	db.mu.Lock()
	defer db.mu.Unlock()

	if db.ptr == nil {
		return ErrClosed
	}

	C.cyl_db_close(db.ptr)
	db.ptr = nil
	runtime.SetFinalizer(db, nil)
	return nil
}

// IsClosed returns true if the database has been closed.
func (db *DB) IsClosed() bool {
	db.mu.Lock()
	defer db.mu.Unlock()
	return db.ptr == nil
}
