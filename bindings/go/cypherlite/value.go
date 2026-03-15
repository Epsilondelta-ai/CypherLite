package cypherlite

/*
#include "cypherlite.h"

// Helpers for accessing CylValue union fields (Go cannot access C unions directly).
static bool cyl_value_get_bool(CylValue v) { return v.payload.boolean; }
static int64_t cyl_value_get_int64(CylValue v) { return v.payload.int64; }
static double cyl_value_get_float64(CylValue v) { return v.payload.float64; }
static const char* cyl_value_get_string(CylValue v) { return v.payload.string; }
static CylBytes cyl_value_get_bytes(CylValue v) { return v.payload.bytes; }
static uint64_t cyl_value_get_node_id(CylValue v) { return v.payload.node_id; }
static uint64_t cyl_value_get_edge_id(CylValue v) { return v.payload.edge_id; }
static CylList cyl_value_get_list(CylValue v) { return v.payload.list; }
*/
import "C"
import "unsafe"

// CylValue tag constants matching the Rust FFI CylValueTag enum.
const (
	tagNull    = 0
	tagBool    = 1
	tagInt64   = 2
	tagFloat64 = 3
	tagString  = 4
	tagBytes   = 5
	tagList    = 6
	tagNode    = 7
	tagEdge    = 8
	tagDT      = 9
)

// cylValueToGo converts a C CylValue tagged union to a Go value.
func cylValueToGo(v C.CylValue) interface{} {
	switch v.tag {
	case tagNull:
		return nil
	case tagBool:
		return bool(C.cyl_value_get_bool(v))
	case tagInt64:
		return int64(C.cyl_value_get_int64(v))
	case tagFloat64:
		return float64(C.cyl_value_get_float64(v))
	case tagString:
		cStr := C.cyl_value_get_string(v)
		if cStr == nil {
			return nil
		}
		return C.GoString(cStr)
	case tagBytes:
		cb := C.cyl_value_get_bytes(v)
		if cb.data == nil || cb.len == 0 {
			return []byte{}
		}
		return C.GoBytes(unsafe.Pointer(cb.data), C.int(cb.len))
	case tagList:
		cl := C.cyl_value_get_list(v)
		return []interface{}{int(cl.len)} // Placeholder: return list length only
	case tagNode:
		return NodeID(C.cyl_value_get_node_id(v))
	case tagEdge:
		return EdgeID(C.cyl_value_get_edge_id(v))
	case tagDT:
		return DateTime(C.cyl_value_get_int64(v))
	default:
		return nil
	}
}
