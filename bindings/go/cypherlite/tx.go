package cypherlite

// #include "cypherlite.h"
// #include <stdlib.h>
import "C"
import (
	"errors"
	"runtime"
	"sync"
	"unsafe"
)

// ErrTxClosed is returned when attempting to use a closed transaction.
var ErrTxClosed = errors.New("cypherlite: transaction is closed")

// Tx represents an active CypherLite transaction.
type Tx struct {
	ptr    *C.CylTx
	db     *DB
	closed bool
	mu     sync.Mutex
}

// Begin starts a new transaction on the database.
// While a transaction is active, direct db.Execute calls will fail.
// Use tx.Execute instead.
func (db *DB) Begin() (*Tx, error) {
	db.mu.Lock()
	defer db.mu.Unlock()

	if db.ptr == nil {
		return nil, ErrClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var errCode C.CylError
	ptr := C.cyl_tx_begin(db.ptr, &errCode)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	return &Tx{ptr: ptr, db: db}, nil
}

// Execute runs a Cypher query within the transaction.
func (tx *Tx) Execute(query string) (*Result, error) {
	tx.mu.Lock()
	defer tx.mu.Unlock()

	if tx.closed {
		return nil, ErrTxClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	var errCode C.CylError
	ptr := C.cyl_tx_execute(tx.ptr, cQuery, &errCode)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	return newResult(ptr), nil
}

// ExecuteWithParams runs a parameterized Cypher query within the transaction.
func (tx *Tx) ExecuteWithParams(query string, params map[string]interface{}) (*Result, error) {
	tx.mu.Lock()
	defer tx.mu.Unlock()

	if tx.closed {
		return nil, ErrTxClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	if len(params) == 0 {
		var errCode C.CylError
		ptr := C.cyl_tx_execute(tx.ptr, cQuery, &errCode)
		if ptr == nil {
			return nil, errorFromCode(errCode)
		}
		return newResult(ptr), nil
	}

	cp, err := buildCParams(params)
	if err != nil {
		return nil, err
	}
	defer cp.free()

	var errCode C.CylError
	ptr := C.cyl_tx_execute_with_params(
		tx.ptr,
		cQuery,
		cp.keysPtr(),
		cp.valsPtr(),
		cp.count,
		&errCode,
	)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	return newResult(ptr), nil
}

// Commit commits the transaction and releases the handle.
// After commit, the transaction cannot be used.
func (tx *Tx) Commit() error {
	tx.mu.Lock()
	defer tx.mu.Unlock()

	if tx.closed {
		return ErrTxClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	var errCode C.CylError
	C.cyl_tx_commit(tx.ptr, &errCode)
	tx.closed = true
	tx.ptr = nil

	return errorFromCode(errCode)
}

// Rollback aborts the transaction and releases the handle.
// Note: In the current implementation, rollback is a no-op at the storage level.
func (tx *Tx) Rollback() error {
	tx.mu.Lock()
	defer tx.mu.Unlock()

	if tx.closed {
		return ErrTxClosed
	}

	C.cyl_tx_rollback(tx.ptr)
	tx.closed = true
	tx.ptr = nil

	return nil
}
