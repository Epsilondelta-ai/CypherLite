package cypherlite

import (
	"testing"
)

func TestValue_StringType(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:V {s: 'hello'}) RETURN n.s")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	s, ok := val.(string)
	if !ok {
		t.Fatalf("expected string, got %T: %v", val, val)
	}
	if s != "hello" {
		t.Fatalf("expected %q, got %q", "hello", s)
	}
}

func TestValue_Int64Type(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:V {i: 42}) RETURN n.i")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	i, ok := val.(int64)
	if !ok {
		t.Fatalf("expected int64, got %T: %v", val, val)
	}
	if i != 42 {
		t.Fatalf("expected 42, got %d", i)
	}
}

func TestValue_Float64Type(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:V {f: 3.14}) RETURN n.f")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	f, ok := val.(float64)
	if !ok {
		t.Fatalf("expected float64, got %T: %v", val, val)
	}
	if f < 3.13 || f > 3.15 {
		t.Fatalf("expected ~3.14, got %f", f)
	}
}

func TestValue_BoolType(t *testing.T) {
	db := openTestDB(t)

	res, err := db.Execute("CREATE (n:V {b: true}) RETURN n.b")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	b, ok := val.(bool)
	if !ok {
		t.Fatalf("expected bool, got %T: %v", val, val)
	}
	if !b {
		t.Fatal("expected true, got false")
	}
}

func TestValue_NullType(t *testing.T) {
	db := openTestDB(t)

	// Accessing a non-existent property returns null.
	res, err := db.Execute("CREATE (n:V) RETURN n.nonexistent")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	if val != nil {
		t.Fatalf("expected nil, got %T: %v", val, val)
	}
}

func TestValue_NodeIDType(t *testing.T) {
	db := openTestDB(t)

	// RETURN id(n) should return a NodeID.
	res, err := db.Execute("CREATE (n:V) RETURN id(n)")
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	defer res.Close()

	row := res.Row(0)
	val := row.Get(0)

	// Node id can come back as int64 or NodeID depending on FFI implementation.
	switch val.(type) {
	case int64:
		// acceptable
	case NodeID:
		// acceptable
	default:
		t.Fatalf("expected int64 or NodeID, got %T: %v", val, val)
	}
}

func TestValue_ParamTypes(t *testing.T) {
	db := openTestDB(t)

	// Test that various Go types can be used as parameters and round-trip correctly.
	tests := []struct {
		name     string
		paramVal interface{}
		query    string
		check    func(t *testing.T, val interface{})
	}{
		{
			name:     "bool_param",
			paramVal: true,
			query:    "CREATE (n:P {v: $v}) RETURN n.v",
			check: func(t *testing.T, val interface{}) {
				b, ok := val.(bool)
				if !ok {
					t.Fatalf("expected bool, got %T", val)
				}
				if !b {
					t.Fatal("expected true")
				}
			},
		},
		{
			name:     "int64_param",
			paramVal: int64(99),
			query:    "CREATE (n:P {v: $v}) RETURN n.v",
			check: func(t *testing.T, val interface{}) {
				i, ok := val.(int64)
				if !ok {
					t.Fatalf("expected int64, got %T", val)
				}
				if i != 99 {
					t.Fatalf("expected 99, got %d", i)
				}
			},
		},
		{
			name:     "float64_param",
			paramVal: 2.718,
			query:    "CREATE (n:P {v: $v}) RETURN n.v",
			check: func(t *testing.T, val interface{}) {
				f, ok := val.(float64)
				if !ok {
					t.Fatalf("expected float64, got %T", val)
				}
				if f < 2.71 || f > 2.72 {
					t.Fatalf("expected ~2.718, got %f", f)
				}
			},
		},
		{
			name:     "string_param",
			paramVal: "world",
			query:    "CREATE (n:P {v: $v}) RETURN n.v",
			check: func(t *testing.T, val interface{}) {
				s, ok := val.(string)
				if !ok {
					t.Fatalf("expected string, got %T", val)
				}
				if s != "world" {
					t.Fatalf("expected %q, got %q", "world", s)
				}
			},
		},
		{
			name:     "nil_param",
			paramVal: nil,
			query:    "CREATE (n:P {v: $v}) RETURN n.v",
			check: func(t *testing.T, val interface{}) {
				if val != nil {
					t.Fatalf("expected nil, got %T: %v", val, val)
				}
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			params := map[string]interface{}{"v": tc.paramVal}
			res, err := db.ExecuteWithParams(tc.query, params)
			if err != nil {
				t.Fatalf("ExecuteWithParams failed: %v", err)
			}
			defer res.Close()

			row := res.Row(0)
			if row == nil {
				t.Fatal("expected a row")
			}
			val := row.Get(0)
			tc.check(t, val)
		})
	}
}
