// Transaction wrapper for Node.js.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use napi::Env;

use cypherlite_query::api::CypherLite;

use crate::error::{db_closed, mutex_poisoned, to_napi_error};
use crate::result::CylResult;
use crate::value::convert_params;

/// A transaction wrapping CypherLite execute calls.
///
/// Shares the database Mutex with the parent Database object.
#[napi]
pub struct Transaction {
    pub(crate) inner: Arc<Mutex<Option<CypherLite>>>,
    pub(crate) in_transaction: Arc<AtomicBool>,
    pub(crate) finished: bool,
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Auto-rollback: clear the transaction flag.
        self.finish();
    }
}

impl Transaction {
    /// Mark this transaction as finished and clear the in_transaction flag.
    fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
            self.in_transaction.store(false, Ordering::SeqCst);
        }
    }
}

#[napi]
impl Transaction {
    /// Execute a Cypher query within this transaction.
    #[napi]
    pub fn execute(
        &mut self,
        env: Env,
        query: String,
        params: Option<napi::JsObject>,
    ) -> napi::Result<CylResult> {
        if self.finished {
            return Err(napi::Error::from_reason("transaction is already finished"));
        }
        let rust_params = convert_params(&env, params)?;
        let mut guard = self.inner.lock().map_err(|_| mutex_poisoned())?;
        let db = guard.as_mut().ok_or_else(db_closed)?;
        let qr = if rust_params.is_empty() {
            db.execute(&query).map_err(to_napi_error)?
        } else {
            db.execute_with_params(&query, rust_params)
                .map_err(to_napi_error)?
        };
        Ok(CylResult::from_query_result(qr))
    }

    /// Commit the transaction.
    #[napi]
    pub fn commit(&mut self) -> napi::Result<()> {
        if self.finished {
            return Err(napi::Error::from_reason("transaction is already finished"));
        }
        self.finish();
        Ok(())
    }

    /// Rollback the transaction (Phase 2: no-op at storage level).
    #[napi]
    pub fn rollback(&mut self) -> napi::Result<()> {
        if self.finished {
            return Err(napi::Error::from_reason("transaction is already finished"));
        }
        self.finish();
        Ok(())
    }
}
