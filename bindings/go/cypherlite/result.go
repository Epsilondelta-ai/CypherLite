package cypherlite

// #include "cypherlite.h"
import "C"

// Result holds the results of a CypherLite query execution.
// The caller must call Close() when done to free underlying resources.
type Result struct {
	ptr      *C.CylResult
	columns  []string
	rowCount int
}

// newResult wraps a C CylResult pointer and pre-caches metadata.
func newResult(ptr *C.CylResult) *Result {
	if ptr == nil {
		return &Result{}
	}

	colCount := int(C.cyl_result_column_count(ptr))
	columns := make([]string, colCount)
	for i := 0; i < colCount; i++ {
		cName := C.cyl_result_column_name(ptr, C.uint32_t(i))
		if cName != nil {
			columns[i] = C.GoString(cName)
		}
	}

	rowCount := int(C.cyl_result_row_count(ptr))

	return &Result{
		ptr:      ptr,
		columns:  columns,
		rowCount: rowCount,
	}
}

// Columns returns the column names in the result.
func (r *Result) Columns() []string {
	return r.columns
}

// RowCount returns the number of rows in the result.
func (r *Result) RowCount() int {
	return r.rowCount
}

// Close frees the underlying C result. Safe to call multiple times.
func (r *Result) Close() {
	if r.ptr != nil {
		C.cyl_result_free(r.ptr)
		r.ptr = nil
	}
}
