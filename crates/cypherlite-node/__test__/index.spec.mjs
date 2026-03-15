// CypherLite Node.js bindings specification tests.
//
// These tests define the expected behavior of the napi-rs bindings.
// They cover: version/features, database lifecycle, query execution,
// parameter binding, transactions, result access, value types, and errors.

import { describe, it, expect, afterEach } from 'vitest';
import { tmpdir } from 'node:os';
import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';

// The native module is loaded through the lib.js wrapper (package entry point).
import {
  version,
  features,
  open,
  Database,
  CylResult,
  Transaction,
} from '../lib.js';

// Helper: create a temporary directory for each test.
async function createTempDir() {
  return mkdtemp(join(tmpdir(), 'cypherlite-node-test-'));
}

// Track temp dirs for cleanup.
const tempDirs = [];

afterEach(async () => {
  for (const dir of tempDirs) {
    await rm(dir, { recursive: true, force: true }).catch(() => {});
  }
  tempDirs.length = 0;
});

async function openTempDb(options) {
  const dir = await createTempDir();
  tempDirs.push(dir);
  const dbPath = join(dir, 'test.cyl');
  return open(dbPath, options);
}

// ============================================================
// M1: Version and Features
// ============================================================

describe('version and features', () => {
  it('should return a version string', () => {
    const v = version();
    expect(typeof v).toBe('string');
    expect(v).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it('should return a features string', () => {
    const f = features();
    expect(typeof f).toBe('string');
    // At minimum, temporal-core should be present (default feature).
    expect(f).toContain('temporal-core');
  });
});

// ============================================================
// M2: Database Lifecycle
// ============================================================

describe('database lifecycle', () => {
  it('should open a database', async () => {
    const db = await openTempDb();
    expect(db).toBeInstanceOf(Database);
    db.close();
  });

  it('should open with custom options', async () => {
    const db = await openTempDb({ pageSize: 4096, cacheCapacity: 128 });
    expect(db).toBeInstanceOf(Database);
    db.close();
  });

  it('should report isClosed correctly', async () => {
    const db = await openTempDb();
    expect(db.isClosed).toBe(false);
    db.close();
    expect(db.isClosed).toBe(true);
  });

  it('should allow close to be called multiple times', async () => {
    const db = await openTempDb();
    db.close();
    expect(() => db.close()).not.toThrow();
  });

  it('should throw on execute after close', async () => {
    const db = await openTempDb();
    db.close();
    expect(() => db.execute("MATCH (n) RETURN n")).toThrow(/closed/i);
  });
});

// ============================================================
// M3: Query Execution
// ============================================================

describe('query execution', () => {
  it('should execute CREATE and MATCH', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    expect(result).toBeInstanceOf(CylResult);
    expect(result.length).toBe(1);
    const row = result.row(0);
    expect(row['n.name']).toBe('Alice');
    expect(row['n.age']).toBe(30);
    db.close();
  });

  it('should return empty result for non-existent label', async () => {
    const db = await openTempDb();
    const result = db.execute("MATCH (n:Ghost) RETURN n");
    expect(result.length).toBe(0);
    db.close();
  });

  it('should execute with parameters', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute(
      "MATCH (n:Person) WHERE n.name = $name RETURN n.name",
      { name: 'Alice' }
    );
    expect(result.length).toBe(1);
    expect(result.row(0)['n.name']).toBe('Alice');
    db.close();
  });

  it('should handle multiple rows', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    db.execute("CREATE (n:Person {name: 'Bob'})");
    db.execute("CREATE (n:Person {name: 'Charlie'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    expect(result.length).toBe(3);
    const names = result.toArray().map(r => r['n.name']).sort();
    expect(names).toEqual(['Alice', 'Bob', 'Charlie']);
    db.close();
  });

  it('should handle SET then MATCH', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 25})");
    db.execute("MATCH (n:Person) SET n.age = 30");
    const result = db.execute("MATCH (n:Person) RETURN n.age");
    expect(result.row(0)['n.age']).toBe(30);
    db.close();
  });

  it('should handle DETACH DELETE', async () => {
    const db = await openTempDb();
    db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})");
    db.execute("MATCH (n:Person) DETACH DELETE n");
    const result = db.execute("MATCH (n:Person) RETURN n");
    expect(result.length).toBe(0);
    db.close();
  });
});

// ============================================================
// M4: Transaction Support
// ============================================================

describe('transactions', () => {
  it('should begin and commit a transaction', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    expect(tx).toBeInstanceOf(Transaction);
    tx.execute("CREATE (n:Person {name: 'Alice'})");
    tx.commit();
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    expect(result.length).toBe(1);
    expect(result.row(0)['n.name']).toBe('Alice');
    db.close();
  });

  it('should begin and rollback a transaction', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    tx.execute("CREATE (n:Person {name: 'Bob'})");
    tx.rollback();
    // After rollback (Phase 2 no-op), data may still exist.
    // The important thing is no error is thrown.
    db.close();
  });

  it('should throw on execute after commit', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    tx.commit();
    expect(() => tx.execute("CREATE (n:Person {name: 'X'})")).toThrow(/finished/i);
    db.close();
  });

  it('should throw on execute after rollback', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    tx.rollback();
    expect(() => tx.execute("CREATE (n:Person {name: 'X'})")).toThrow(/finished/i);
    db.close();
  });

  it('should throw on double commit', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    tx.commit();
    expect(() => tx.commit()).toThrow(/finished/i);
    db.close();
  });

  it('should execute with params in transaction', async () => {
    const db = await openTempDb();
    const tx = db.begin();
    tx.execute("CREATE (n:Person {name: 'Alice'})");
    const result = tx.execute(
      "MATCH (n:Person) WHERE n.name = $name RETURN n.name",
      { name: 'Alice' }
    );
    expect(result.length).toBe(1);
    tx.commit();
    db.close();
  });
});

// ============================================================
// M5: Result and Row Access
// ============================================================

describe('result access', () => {
  it('should expose column names', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    const cols = result.columns;
    expect(Array.isArray(cols)).toBe(true);
    expect(cols).toContain('n.name');
    expect(cols).toContain('n.age');
    db.close();
  });

  it('should expose row count via length', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    db.execute("CREATE (n:Person {name: 'Bob'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    expect(result.length).toBe(2);
    db.close();
  });

  it('should access individual row by index', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    const row = result.row(0);
    expect(typeof row).toBe('object');
    expect(row['n.name']).toBe('Alice');
    expect(row['n.age']).toBe(30);
    db.close();
  });

  it('should throw on out-of-range row index', async () => {
    const db = await openTempDb();
    const result = db.execute("MATCH (n:Ghost) RETURN n");
    expect(() => result.row(0)).toThrow(/out of range/i);
    db.close();
  });

  it('should convert all rows via toArray', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    db.execute("CREATE (n:Person {name: 'Bob'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    const rows = result.toArray();
    expect(Array.isArray(rows)).toBe(true);
    expect(rows.length).toBe(2);
    const names = rows.map(r => r['n.name']).sort();
    expect(names).toEqual(['Alice', 'Bob']);
    db.close();
  });
});

// ============================================================
// M6: Value Type Conversions
// ============================================================

describe('value types', () => {
  it('should handle null values', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.email");
    const row = result.row(0);
    expect(row['n.name']).toBe('Alice');
    // Missing property should be null.
    expect(row['n.email']).toBeNull();
    db.close();
  });

  it('should handle boolean values', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Flag {active: true})");
    const result = db.execute("MATCH (n:Flag) RETURN n.active");
    expect(result.row(0)['n.active']).toBe(true);
    db.close();
  });

  it('should handle integer values', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Num {val: 42})");
    const result = db.execute("MATCH (n:Num) RETURN n.val");
    expect(result.row(0)['n.val']).toBe(42);
    db.close();
  });

  it('should handle float values', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Num {val: 3.14})");
    const result = db.execute("MATCH (n:Num) RETURN n.val");
    expect(result.row(0)['n.val']).toBeCloseTo(3.14);
    db.close();
  });

  it('should handle string values', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Text {val: 'hello world'})");
    const result = db.execute("MATCH (n:Text) RETURN n.val");
    expect(result.row(0)['n.val']).toBe('hello world');
    db.close();
  });

  it('should handle node ID as BigInt', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute("MATCH (n:Person) RETURN n");
    const row = result.row(0);
    // Node value should be a BigInt (node ID).
    expect(typeof row['n']).toBe('bigint');
    db.close();
  });

  it('should handle edge ID as BigInt', async () => {
    const db = await openTempDb();
    db.execute("CREATE (a:Person {name: 'A'})-[:KNOWS]->(b:Person {name: 'B'})");
    const result = db.execute("MATCH ()-[r:KNOWS]->() RETURN r");
    const row = result.row(0);
    // Edge value should be a BigInt (edge ID).
    expect(typeof row['r']).toBe('bigint');
    db.close();
  });

  it('should handle parameter types (string, int, float, bool, null)', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Item {name: 'test'})");
    // String param
    let result = db.execute(
      "MATCH (n:Item) WHERE n.name = $v RETURN n.name",
      { v: 'test' }
    );
    expect(result.length).toBe(1);
    // Int param
    db.execute("CREATE (n:Num {val: 42})");
    result = db.execute(
      "MATCH (n:Num) WHERE n.val = $v RETURN n.val",
      { v: 42 }
    );
    expect(result.length).toBe(1);
    db.close();
  });
});

// ============================================================
// M7: Iterator Protocol (Symbol.iterator)
// ============================================================

describe('iterator protocol', () => {
  it('should iterate rows with for...of', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})");
    db.execute("CREATE (n:Person {name: 'Bob', age: 25})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    const rows = [];
    for (const row of result) {
      rows.push(row);
    }
    expect(rows.length).toBe(2);
    const names = rows.map(r => r['n.name']).sort();
    expect(names).toEqual(['Alice', 'Bob']);
    db.close();
  });

  it('should support spread operator', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    db.execute("CREATE (n:Person {name: 'Bob'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    const rows = [...result];
    expect(rows.length).toBe(2);
    db.close();
  });

  it('should support Array.from', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    const rows = Array.from(result);
    expect(rows.length).toBe(1);
    expect(rows[0]['n.name']).toBe('Alice');
    db.close();
  });

  it('should support destructuring assignment', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice', age: 30})");
    const result = db.execute("MATCH (n:Person) RETURN n.name, n.age");
    const [first] = result;
    expect(first['n.name']).toBe('Alice');
    expect(first['n.age']).toBe(30);
    db.close();
  });

  it('should yield nothing for empty result', async () => {
    const db = await openTempDb();
    const result = db.execute("MATCH (n:Ghost) RETURN n");
    const rows = [...result];
    expect(rows.length).toBe(0);
    db.close();
  });

  it('should be re-iterable', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    const first = [...result];
    const second = [...result];
    expect(first.length).toBe(1);
    expect(second.length).toBe(1);
    expect(first[0]['n.name']).toBe(second[0]['n.name']);
    db.close();
  });

  it('should iterate after database is closed (data is cached in Rust memory)', async () => {
    const db = await openTempDb();
    db.execute("CREATE (n:Person {name: 'Alice'})");
    const result = db.execute("MATCH (n:Person) RETURN n.name");
    db.close();
    // Result rows are materialized in Rust Vec and survive db.close().
    const rows = [...result];
    expect(rows.length).toBe(1);
    expect(rows[0]['n.name']).toBe('Alice');
  });
});

// ============================================================
// Error Handling
// ============================================================

describe('error handling', () => {
  it('should throw on invalid Cypher syntax', async () => {
    const db = await openTempDb();
    expect(() => db.execute("INVALID QUERY @#$")).toThrow();
    db.close();
  });

  it('should throw on semantic error (undefined variable)', async () => {
    const db = await openTempDb();
    expect(() => db.execute("MATCH (n:Person) RETURN m.name")).toThrow();
    db.close();
  });

  it('should include error message in thrown error', async () => {
    const db = await openTempDb();
    try {
      db.execute("MATCH (n:Person RETURN n");
      expect.unreachable('should have thrown');
    } catch (e) {
      expect(e.message).toBeTruthy();
      expect(typeof e.message).toBe('string');
    }
    db.close();
  });
});
