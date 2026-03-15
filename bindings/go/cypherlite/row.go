package cypherlite

// #include "cypherlite.h"
// #include <stdlib.h>
import "C"
import (
	"runtime"
	"unsafe"
)

// Row provides access to a single row in a Result.
type Row struct {
	result *Result
	index  uint64
}

// Row returns a Row accessor for the given row index.
// Returns nil if the index is out of bounds.
func (r *Result) Row(index int) *Row {
	if index < 0 || index >= r.rowCount || r.ptr == nil {
		return nil
	}
	return &Row{result: r, index: uint64(index)}
}

// Get returns the value at the given column index.
// Returns nil for null values, out-of-bounds indices, or if the result is closed.
func (row *Row) Get(colIndex int) interface{} {
	if row.result.ptr == nil {
		return nil
	}
	cv := C.cyl_result_get(row.result.ptr, C.uint64_t(row.index), C.uint32_t(colIndex))
	return cylValueToGo(cv)
}

// GetByName returns the value for the given column name.
// Returns an error if the column name does not exist.
func (row *Row) GetByName(colName string) (interface{}, error) {
	if row.result.ptr == nil {
		return nil, ErrClosed
	}

	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	cName := C.CString(colName)
	defer C.free(unsafe.Pointer(cName))

	var errCode C.CylError
	cv := C.cyl_result_get_by_name(row.result.ptr, C.uint64_t(row.index), cName, &errCode)
	if errCode != C.CYL_OK {
		return nil, errorFromCode(errCode)
	}

	return cylValueToGo(cv), nil
}
