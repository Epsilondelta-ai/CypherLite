package cypherlite

// #include "cypherlite.h"
// #include <stdlib.h>
import "C"
import (
	"fmt"
	"unsafe"
)

// cParams holds C-allocated parameter arrays for FFI calls.
// The caller must call free() when done.
type cParams struct {
	keys             []*C.char
	vals             []C.CylValue
	count            C.uint32_t
	allocatedStrings []unsafe.Pointer
}

// free releases all C-allocated memory in the params.
func (p *cParams) free() {
	for _, k := range p.keys {
		C.free(unsafe.Pointer(k))
	}
	for _, s := range p.allocatedStrings {
		C.free(s)
	}
}

// keysPtr returns a pointer to the first key, suitable for passing to C.
// Returns nil if count is 0.
func (p *cParams) keysPtr() **C.char {
	if p.count == 0 {
		return nil
	}
	return (**C.char)(unsafe.Pointer(&p.keys[0]))
}

// valsPtr returns a pointer to the first value, suitable for passing to C.
// Returns nil if count is 0.
func (p *cParams) valsPtr() *C.CylValue {
	if p.count == 0 {
		return nil
	}
	return (*C.CylValue)(unsafe.Pointer(&p.vals[0]))
}

// buildCParams converts a Go map of parameters to C arrays.
func buildCParams(params map[string]interface{}) (*cParams, error) {
	n := len(params)
	cp := &cParams{
		keys:             make([]*C.char, n),
		vals:             make([]C.CylValue, n),
		count:            C.uint32_t(n),
		allocatedStrings: make([]unsafe.Pointer, 0, n),
	}

	i := 0
	for k, v := range params {
		cp.keys[i] = C.CString(k)
		cVal, strPtr, err := goValueToCylValue(v)
		if err != nil {
			cp.free()
			return nil, err
		}
		cp.vals[i] = cVal
		if strPtr != nil {
			cp.allocatedStrings = append(cp.allocatedStrings, strPtr)
		}
		i++
	}

	return cp, nil
}

// goValueToCylValue converts a Go value to a C CylValue.
// Returns the CylValue, an optional C string pointer that must be freed, and an error.
func goValueToCylValue(v interface{}) (C.CylValue, unsafe.Pointer, error) {
	switch val := v.(type) {
	case nil:
		return C.cyl_param_null(), nil, nil
	case bool:
		return C.cyl_param_bool(C.bool(val)), nil, nil
	case int64:
		return C.cyl_param_int64(C.int64_t(val)), nil, nil
	case int:
		return C.cyl_param_int64(C.int64_t(val)), nil, nil
	case float64:
		return C.cyl_param_float64(C.double(val)), nil, nil
	case string:
		cs := C.CString(val)
		return C.cyl_param_string(cs), unsafe.Pointer(cs), nil
	case []byte:
		if len(val) == 0 {
			return C.cyl_param_bytes(nil, 0), nil, nil
		}
		return C.cyl_param_bytes((*C.uint8_t)(unsafe.Pointer(&val[0])), C.uint32_t(len(val))), nil, nil
	default:
		return C.CylValue{}, nil, fmt.Errorf("cypherlite: unsupported parameter type %T", v)
	}
}
