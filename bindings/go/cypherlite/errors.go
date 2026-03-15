package cypherlite

// #include "cypherlite.h"
import "C"
import (
	"errors"
	"fmt"
)

// Error represents a CypherLite error with a code and message.
type Error struct {
	Code    int32
	Message string
}

// Error implements the error interface.
func (e *Error) Error() string {
	return fmt.Sprintf("cypherlite error %d: %s", e.Code, e.Message)
}

// Sentinel errors for common CypherLite error codes.
var (
	ErrIO                  = errors.New("cypherlite: I/O error")
	ErrCorruptedPage       = errors.New("cypherlite: corrupted page")
	ErrTransactionConflict = errors.New("cypherlite: transaction conflict")
	ErrOutOfSpace          = errors.New("cypherlite: out of space")
	ErrInvalidMagic        = errors.New("cypherlite: invalid magic")
	ErrUnsupportedVersion  = errors.New("cypherlite: unsupported version")
	ErrChecksumMismatch    = errors.New("cypherlite: checksum mismatch")
	ErrSerialization       = errors.New("cypherlite: serialization error")
	ErrNodeNotFound        = errors.New("cypherlite: node not found")
	ErrEdgeNotFound        = errors.New("cypherlite: edge not found")
	ErrParse               = errors.New("cypherlite: parse error")
	ErrSemantic            = errors.New("cypherlite: semantic error")
	ErrExecution           = errors.New("cypherlite: execution error")
	ErrUnsupportedSyntax   = errors.New("cypherlite: unsupported syntax")
	ErrConstraintViolation = errors.New("cypherlite: constraint violation")
	ErrInvalidDateTime     = errors.New("cypherlite: invalid datetime")
	ErrSystemPropertyRO    = errors.New("cypherlite: system property read-only")
	ErrFeatureIncompatible = errors.New("cypherlite: feature incompatible")
	ErrNullPointer         = errors.New("cypherlite: null pointer")
	ErrInvalidUTF8         = errors.New("cypherlite: invalid UTF-8")
	ErrClosed              = errors.New("cypherlite: database is closed")
)

// errorFromCode converts a C error code to a Go error.
// Must be called while the OS thread is still locked (before UnlockOSThread)
// so the thread-local error message is accessible.
func errorFromCode(code C.int32_t) error {
	if code == C.CYL_OK {
		return nil
	}

	// Retrieve the thread-local error message from the Rust FFI.
	var msg string
	cMsg := C.cyl_last_error_message()
	if cMsg != nil {
		msg = C.GoString(cMsg)
	}

	cylErr := &Error{
		Code:    int32(code),
		Message: msg,
	}

	return cylErr
}
