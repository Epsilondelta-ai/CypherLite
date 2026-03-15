package cypherlite

import (
	"strings"
	"testing"
)

func TestVersion_ReturnsNonEmpty(t *testing.T) {
	v := Version()
	if v == "" {
		t.Fatal("Version() returned empty string")
	}
}

func TestVersion_ContainsSemver(t *testing.T) {
	v := Version()
	// Version should contain at least one dot (e.g., "1.1.0")
	if !strings.Contains(v, ".") {
		t.Fatalf("Version() = %q, expected semver-like format with dots", v)
	}
}

func TestFeatures_ReturnsString(t *testing.T) {
	// Features() should return a string (possibly empty if no features enabled).
	// It must not panic.
	f := Features()
	_ = f // Just ensure it does not panic
}
