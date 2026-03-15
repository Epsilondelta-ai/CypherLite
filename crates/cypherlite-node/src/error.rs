// Error conversion from CypherLiteError to napi::Error.

use cypherlite_core::CypherLiteError;

/// Convert a CypherLiteError into a napi::Error suitable for throwing in JS.
pub fn to_napi_error(e: CypherLiteError) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

/// Create an error for a poisoned mutex.
pub fn mutex_poisoned() -> napi::Error {
    napi::Error::from_reason("internal error: mutex poisoned")
}

/// Create an error for a closed database.
pub fn db_closed() -> napi::Error {
    napi::Error::from_reason("database is closed")
}
