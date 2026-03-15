package cypherlite

// #include "cypherlite.h"
// #include <stdlib.h>
import "C"
import (
	"runtime"
	"unsafe"
)

// Execute runs a Cypher query string on the database and returns the result.
func (db *DB) Execute(query string) (*Result, error) {
	db.mu.Lock()
	defer db.mu.Unlock()

	if db.ptr == nil {
		return nil, ErrClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	var errCode C.CylError
	ptr := C.cyl_db_execute(db.ptr, cQuery, &errCode)
	if ptr == nil {
		return nil, errorFromCode(errCode)
	}

	return newResult(ptr), nil
}

// ExecuteWithParams runs a parameterized Cypher query on the database.
// Parameters are passed as a map of name -> value.
// Supported value types: nil, bool, int64, int, float64, string, []byte.
func (db *DB) ExecuteWithParams(query string, params map[string]interface{}) (*Result, error) {
	db.mu.Lock()
	defer db.mu.Unlock()

	if db.ptr == nil {
		return nil, ErrClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cQuery := C.CString(query)
	defer C.free(unsafe.Pointer(cQuery))

	if len(params) == 0 {
		var errCode C.CylError
		ptr := C.cyl_db_execute(db.ptr, cQuery, &errCode)
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
	ptr := C.cyl_db_execute_with_params(
		db.ptr,
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
