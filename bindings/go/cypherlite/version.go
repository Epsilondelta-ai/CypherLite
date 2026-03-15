package cypherlite

// #include "cypherlite.h"
import "C"

// Version returns the CypherLite library version string (e.g., "1.1.0").
func Version() string {
	cStr := C.cyl_version()
	return C.GoString(cStr)
}

// Features returns a comma-separated list of enabled feature flags.
// Returns an empty string if no features are enabled.
func Features() string {
	cStr := C.cyl_features()
	return C.GoString(cStr)
}
