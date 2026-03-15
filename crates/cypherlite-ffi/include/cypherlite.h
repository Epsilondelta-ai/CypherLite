#ifndef CYPHERLITE_H
#define CYPHERLITE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * FFI error codes returned by CypherLite C API functions.
 */
enum CylError
#ifdef __cplusplus
  : int32_t
#endif // __cplusplus
 {
  /**
   * Operation succeeded.
   */
  CYL_OK = 0,
  /**
   * I/O error (file system, permissions, etc.).
   */
  CYL_ERR_IO = 1,
  /**
   * Corrupted database page.
   */
  CYL_ERR_CORRUPTED_PAGE = 2,
  /**
   * Write transaction conflict (another transaction is active).
   */
  CYL_ERR_TRANSACTION_CONFLICT = 3,
  /**
   * Disk or buffer pool full.
   */
  CYL_ERR_OUT_OF_SPACE = 4,
  /**
   * Invalid magic number in database file.
   */
  CYL_ERR_INVALID_MAGIC = 5,
  /**
   * Unsupported database format version.
   */
  CYL_ERR_UNSUPPORTED_VERSION = 6,
  /**
   * Checksum verification failed.
   */
  CYL_ERR_CHECKSUM_MISMATCH = 7,
  /**
   * Serialization / deserialization failure.
   */
  CYL_ERR_SERIALIZATION = 8,
  /**
   * Referenced node does not exist.
   */
  CYL_ERR_NODE_NOT_FOUND = 9,
  /**
   * Referenced edge does not exist.
   */
  CYL_ERR_EDGE_NOT_FOUND = 10,
  /**
   * Cypher query parse error.
   */
  CYL_ERR_PARSE = 11,
  /**
   * Semantic analysis error.
   */
  CYL_ERR_SEMANTIC = 12,
  /**
   * Query execution error.
   */
  CYL_ERR_EXECUTION = 13,
  /**
   * Unsupported Cypher syntax.
   */
  CYL_ERR_UNSUPPORTED_SYNTAX = 14,
  /**
   * Constraint violation (uniqueness, etc.).
   */
  CYL_ERR_CONSTRAINT_VIOLATION = 15,
  /**
   * Invalid datetime format string.
   */
  CYL_ERR_INVALID_DATE_TIME = 16,
  /**
   * Attempted write to a read-only system property.
   */
  CYL_ERR_SYSTEM_PROPERTY_READ_ONLY = 17,
  /**
   * Feature incompatibility between database file and compiled binary.
   */
  CYL_ERR_FEATURE_INCOMPATIBLE = 18,
  /**
   * Null pointer passed where non-null was required.
   */
  CYL_ERR_NULL_POINTER = 19,
  /**
   * String is not valid UTF-8.
   */
  CYL_ERR_INVALID_UTF8 = 20,
  /**
   * Subgraph not found (requires `subgraph` feature).
   */
  CYL_ERR_SUBGRAPH_NOT_FOUND = 100,
  /**
   * Hyperedge not found (requires `hypergraph` feature).
   */
  CYL_ERR_HYPEREDGE_NOT_FOUND = 200,
  /**
   * Generic plugin error.
   */
  CYL_ERR_PLUGIN = 300,
  /**
   * Requested function not found in plugin registry.
   */
  CYL_ERR_FUNCTION_NOT_FOUND = 301,
  /**
   * Unsupported index type.
   */
  CYL_ERR_UNSUPPORTED_INDEX_TYPE = 302,
  /**
   * Unsupported serialization format.
   */
  CYL_ERR_UNSUPPORTED_FORMAT = 303,
  /**
   * Trigger execution error.
   */
  CYL_ERR_TRIGGER = 304,
};
#ifndef __cplusplus
typedef int32_t CylError;
#endif // __cplusplus

/**
 * Opaque FFI handle to a CypherLite database.
 */
typedef struct CylDb CylDb;

/**
 * Opaque FFI handle owning query results.
 *
 * Stores the original `QueryResult` plus pre-computed CString column names
 * so that `cyl_result_column_name` can return borrowed pointers without
 * per-call allocation.
 */
typedef struct CylResult CylResult;

/**
 * Opaque FFI transaction handle.
 */
typedef struct CylTx CylTx;

/**
 * Byte slice representation for FFI.
 */
typedef struct CylBytes {
  const uint8_t *data;
  uint32_t len;
} CylBytes;

/**
 * List representation for FFI (array of CylValue pointers).
 */
typedef struct CylList {
  const struct CylValue *items;
  uint32_t len;
} CylList;

#if defined(CYL_FEATURE_HYPERGRAPH)
/**
 * Temporal node reference (node_id + timestamp in millis).
 */
typedef struct CylTemporalNode {
  uint64_t node_id;
  int64_t timestamp_ms;
} CylTemporalNode;
#endif

/**
 * Payload union for `CylValue`.
 *
 * Which field is valid depends on the `tag` in the enclosing `CylValue`.
 */
typedef union CylValuePayload {
  /**
   * CylValueTag::Bool
   */
  bool boolean;
  /**
   * CylValueTag::Int64 / DateTime
   */
  int64_t int64;
  /**
   * CylValueTag::Float64
   */
  double float64;
  /**
   * CylValueTag::String -- pointer to null-terminated UTF-8.
   * For values returned by cyl_row_get, this is borrowed from the result.
   * For parameter values, the caller owns the string.
   */
  const char *string;
  /**
   * CylValueTag::Bytes -- pointer + length.
   */
  struct CylBytes bytes;
  /**
   * CylValueTag::Node -- node id.
   */
  uint64_t node_id;
  /**
   * CylValueTag::Edge -- edge id.
   */
  uint64_t edge_id;
  /**
   * CylValueTag::List -- pointer + length.
   */
  struct CylList list;
#if defined(CYL_FEATURE_HYPERGRAPH)
  /**
   * CylValueTag::TemporalNode -- (node_id, timestamp_ms).
   */
  struct CylTemporalNode temporal_node
#endif
  ;
} CylValuePayload;

/**
 * A tagged union representing a single query value for FFI.
 */
typedef struct CylValue {
  uint8_t tag;
  union CylValuePayload payload;
} CylValue;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Return the library version as a C string.
 *
 * The returned pointer is valid for the lifetime of the process and MUST NOT
 * be freed by the caller.
 */
const char *cyl_version(void);

/**
 * Return a comma-separated list of enabled feature flags as a C string.
 *
 * The returned pointer is valid for the lifetime of the process and MUST NOT
 * be freed by the caller.  If no features are enabled the string is empty
 * (a single NUL byte).
 */
const char *cyl_features(void);

/**
 * Open a CypherLite database at the given file path with default settings.
 *
 * Returns a heap-allocated `CylDb` pointer on success, or `NULL` on failure
 * (with `*error_out` set to the error code).
 *
 * The caller MUST eventually call `cyl_db_close()` to free the handle.
 *
 * # Safety
 *
 * - `path` must be a valid, null-terminated C string.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylDb *cyl_db_open(const char *path, CylError *error_out);

/**
 * Open a CypherLite database with explicit page size and cache capacity.
 *
 * # Safety
 *
 * - `path` must be a valid, null-terminated C string.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylDb *cyl_db_open_with_config(const char *path,
                                      uint32_t page_size,
                                      uint32_t cache_capacity,
                                      CylError *error_out);

/**
 * Close and free a CypherLite database handle.
 *
 * This is a no-op if `db` is `NULL`. After this call the pointer is invalid.
 *
 * # Safety
 *
 * - `db` must be a pointer previously returned by `cyl_db_open` (or NULL).
 * - `db` must not be used after this call.
 */
void cyl_db_close(struct CylDb *db);

/**
 * Retrieve the most recent error message as a C string.
 *
 * Returns `NULL` if no error has been recorded on the current thread.
 * The returned pointer is valid until the next FFI call on the same thread.
 * The caller MUST NOT free the returned pointer.
 */
const char *cyl_last_error_message(void);

/**
 * Clear the thread-local error state.
 */
void cyl_clear_error(void);

/**
 * Execute a Cypher query string on the database.
 *
 * Returns a heap-allocated `CylResult` on success, or `NULL` on failure.
 * The caller MUST call `cyl_result_free()` to release the result.
 *
 * # Safety
 *
 * - `db` must be a valid `CylDb` pointer (not in-transaction).
 * - `query` must be a valid, null-terminated C string.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylResult *cyl_db_execute(struct CylDb *db, const char *query, CylError *error_out);

/**
 * Execute a Cypher query with named parameters.
 *
 * Parameters are passed as parallel arrays of keys and values plus a count.
 * Each `param_keys[i]` is a null-terminated C string and `param_values[i]`
 * is a `CylValue` (see value.rs).
 *
 * # Safety
 *
 * - `db` must be a valid `CylDb` pointer (not in-transaction).
 * - `query` must be a valid, null-terminated C string.
 * - `param_keys` must point to an array of `param_count` valid C strings.
 * - `param_values` must point to an array of `param_count` `CylValue`s.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylResult *cyl_db_execute_with_params(struct CylDb *db,
                                             const char *query,
                                             const char *const *param_keys,
                                             const struct CylValue *param_values,
                                             uint32_t param_count,
                                             CylError *error_out);

/**
 * Free a CylResult handle. No-op if `result` is NULL.
 *
 * # Safety
 *
 * - `result` must be a pointer returned by `cyl_db_execute` (or NULL).
 */
void cyl_result_free(struct CylResult *result);

/**
 * Return the number of columns in the result.
 *
 * Returns 0 if `result` is NULL.
 *
 * # Safety
 *
 * - `result` must be a valid `CylResult` pointer (or NULL).
 */
uint32_t cyl_result_column_count(const struct CylResult *result);

/**
 * Return the name of the column at `index` as a C string.
 *
 * The returned pointer is borrowed from the CylResult and valid until the
 * result is freed. Returns NULL if `result` is NULL or `index` is out of
 * range.
 *
 * # Safety
 *
 * - `result` must be a valid `CylResult` pointer (or NULL).
 */
const char *cyl_result_column_name(const struct CylResult *result, uint32_t index);

/**
 * Return the number of rows in the result.
 *
 * Returns 0 if `result` is NULL.
 *
 * # Safety
 *
 * - `result` must be a valid `CylResult` pointer (or NULL).
 */
uint64_t cyl_result_row_count(const struct CylResult *result);

/**
 * Get a value from a specific row and column index.
 *
 * Returns a CylValue by value. For String/Bytes the internal pointers borrow
 * from the CylResult -- they are valid until `cyl_result_free()` is called.
 *
 * Returns a Null CylValue if any argument is NULL or out of range.
 *
 * # Safety
 *
 * - `result` must be a valid `CylResult` pointer (or NULL).
 */
struct CylValue cyl_result_get(const struct CylResult *result,
                               uint64_t row_index,
                               uint32_t col_index);

/**
 * Get a value from a specific row by column name.
 *
 * Returns a CylValue by value. For String/Bytes the internal pointers borrow
 * from the CylResult.
 *
 * Returns a Null CylValue if any argument is NULL, the column does not exist,
 * or the row index is out of range.
 *
 * # Safety
 *
 * - `result` must be a valid `CylResult` pointer (or NULL).
 * - `col_name` must be a valid, null-terminated C string (or NULL).
 */
struct CylValue cyl_result_get_by_name(const struct CylResult *result,
                                       uint64_t row_index,
                                       const char *col_name,
                                       CylError *error_out);

/**
 * Begin a transaction on the database.
 *
 * Returns a heap-allocated `CylTx` on success, or `NULL` if the database
 * already has an active transaction or `db` is null.
 *
 * While a transaction is active, `cyl_db_execute()` will return
 * `CYL_ERR_TRANSACTION_CONFLICT`. Use `cyl_tx_execute()` instead.
 *
 * # Safety
 *
 * - `db` must be a valid `CylDb` pointer.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylTx *cyl_tx_begin(struct CylDb *db, CylError *error_out);

/**
 * Execute a Cypher query within the transaction.
 *
 * # Safety
 *
 * - `tx` must be a valid `CylTx` pointer.
 * - `query` must be a valid, null-terminated C string.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylResult *cyl_tx_execute(struct CylTx *tx, const char *query, CylError *error_out);

/**
 * Execute a Cypher query with parameters within the transaction.
 *
 * # Safety
 *
 * - `tx` must be a valid `CylTx` pointer.
 * - `query` must be a valid, null-terminated C string.
 * - `param_keys` must point to an array of `param_count` valid C strings.
 * - `param_values` must point to an array of `param_count` `CylValue`s.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
struct CylResult *cyl_tx_execute_with_params(struct CylTx *tx,
                                             const char *query,
                                             const char *const *param_keys,
                                             const struct CylValue *param_values,
                                             uint32_t param_count,
                                             CylError *error_out);

/**
 * Commit the transaction and free the CylTx handle.
 *
 * After this call the `tx` pointer is invalid. The database handle becomes
 * available for non-transactional queries again.
 *
 * # Safety
 *
 * - `tx` must be a valid `CylTx` pointer.
 * - `error_out` must point to a valid `CylError` or be `NULL`.
 */
void cyl_tx_commit(struct CylTx *tx, CylError *error_out);

/**
 * Rollback the transaction and free the CylTx handle.
 *
 * Note: In the current Phase 2 implementation rollback is a no-op at the
 * storage level (changes already applied are not undone). Full WAL rollback
 * will be added in a future phase.
 *
 * # Safety
 *
 * - `tx` must be a valid `CylTx` pointer (or NULL for no-op).
 */
void cyl_tx_rollback(struct CylTx *tx);

/**
 * Free a CylTx handle. If the transaction was not committed or rolled back,
 * it is automatically rolled back.
 *
 * No-op if `tx` is NULL.
 *
 * # Safety
 *
 * - `tx` must be a pointer returned by `cyl_tx_begin` (or NULL).
 */
void cyl_tx_free(struct CylTx *tx);

/**
 * Create a null CylValue parameter.
 */
struct CylValue cyl_param_null(void);

/**
 * Create a boolean CylValue parameter.
 */
struct CylValue cyl_param_bool(bool value);

/**
 * Create an integer CylValue parameter.
 */
struct CylValue cyl_param_int64(int64_t value);

/**
 * Create a floating-point CylValue parameter.
 */
struct CylValue cyl_param_float64(double value);

/**
 * Create a string CylValue parameter.
 *
 * The `value` pointer is stored directly -- the caller must keep the C
 * string alive until the parameter is consumed by `cyl_db_execute_with_params`.
 *
 * # Safety
 *
 * - `value` must be a valid, null-terminated C string (or NULL for Null).
 */
struct CylValue cyl_param_string(const char *value);

/**
 * Create a bytes CylValue parameter.
 *
 * The data pointer is stored directly -- the caller must keep the buffer
 * alive until the parameter is consumed.
 *
 * # Safety
 *
 * - `data` must point to at least `len` bytes (or be NULL for empty).
 */
struct CylValue cyl_param_bytes(const uint8_t *data, uint32_t len);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* CYPHERLITE_H */
