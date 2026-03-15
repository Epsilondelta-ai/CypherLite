package cypherlite

/*
#cgo CFLAGS: -I${SRCDIR}/../../../crates/cypherlite-ffi/include
#cgo LDFLAGS: -L${SRCDIR}/../../../target/release -lcypherlite_ffi
#cgo darwin LDFLAGS: -framework CoreFoundation -framework Security -lpthread -ldl -lm
#cgo linux LDFLAGS: -lpthread -ldl -lm

#include "cypherlite.h"
#include <stdlib.h>
*/
import "C"
