// Database lifecycle and query execution for Node.js.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use napi::Env;

use cypherlite_core::DatabaseConfig;
use cypherlite_query::api::CypherLite;

use crate::error::{db_closed, mutex_poisoned, to_napi_error};
use crate::result::CylResult;
use crate::transaction::Transaction;
use crate::value::convert_params;

/// Options for opening a database.
#[napi(object)]
pub struct OpenOptions {
    /// Page size in bytes (default: 4096).
    pub page_size: Option<u32>,
    /// Number of pages in the buffer pool cache (default: 256).
    pub cache_capacity: Option<u32>,
}

/// The main CypherLite database handle for Node.js.
#[napi]
pub struct Database {
    inner: Arc<Mutex<Option<CypherLite>>>,
    in_transaction: Arc<AtomicBool>,
}

/// Open a CypherLite database at the given path.
#[napi]
pub fn open(path: String, options: Option<OpenOptions>) -> napi::Result<Database> {
    let config = DatabaseConfig {
        path: std::path::PathBuf::from(&path),
        page_size: options
            .as_ref()
            .and_then(|o| o.page_size)
            .unwrap_or(4096),
        cache_capacity: options
            .as_ref()
            .and_then(|o| o.cache_capacity)
            .unwrap_or(256) as usize,
        ..Default::default()
    };
    let db = CypherLite::open(config).map_err(to_napi_error)?;
    Ok(Database {
        inner: Arc::new(Mutex::new(Some(db))),
        in_transaction: Arc::new(AtomicBool::new(false)),
    })
}

#[napi]
impl Database {
    /// Execute a Cypher query string.
    ///
    /// Optional second argument provides named parameters as a plain object.
    #[napi]
    pub fn execute(
        &self,
        env: Env,
        query: String,
        params: Option<napi::JsObject>,
    ) -> napi::Result<CylResult> {
        if self.in_transaction.load(Ordering::SeqCst) {
            return Err(napi::Error::from_reason(
                "cannot execute on database while a transaction is active",
            ));
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

    /// Close the database. Safe to call multiple times.
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        let mut guard = self.inner.lock().map_err(|_| mutex_poisoned())?;
        let _ = guard.take();
        Ok(())
    }

    /// Check if the database is closed.
    #[napi(getter, js_name = "isClosed")]
    pub fn is_closed(&self) -> napi::Result<bool> {
        let guard = self.inner.lock().map_err(|_| mutex_poisoned())?;
        Ok(guard.is_none())
    }

    /// Begin a new transaction.
    #[napi]
    pub fn begin(&self) -> napi::Result<Transaction> {
        // Check database is open.
        {
            let guard = self.inner.lock().map_err(|_| mutex_poisoned())?;
            if guard.is_none() {
                return Err(db_closed());
            }
        }

        // Check no transaction is already active.
        if self
            .in_transaction
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(napi::Error::from_reason(
                "a transaction is already active",
            ));
        }

        Ok(Transaction {
            inner: Arc::clone(&self.inner),
            in_transaction: Arc::clone(&self.in_transaction),
            finished: false,
        })
    }
}
