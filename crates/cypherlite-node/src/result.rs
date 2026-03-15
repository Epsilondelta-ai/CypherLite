// Query result wrapper for Node.js.

use cypherlite_query::api::QueryResult;
use cypherlite_query::executor::Value;
use napi::Env;

use crate::value::rust_to_js;

/// A query result containing columns and rows.
///
/// Row data is stored as Rust types and converted to JS on access.
#[napi]
pub struct CylResult {
    columns: Vec<String>,
    /// Each row is a Vec of Values in column order.
    rows: Vec<Vec<Value>>,
}

impl CylResult {
    /// Create from a Rust QueryResult.
    pub fn from_query_result(qr: QueryResult) -> Self {
        let columns = qr.columns;
        let rows: Vec<Vec<Value>> = qr
            .rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|col| row.get(col).cloned().unwrap_or(Value::Null))
                    .collect()
            })
            .collect();
        Self { columns, rows }
    }
}

#[napi]
impl CylResult {
    /// Column names as an array of strings.
    #[napi(getter)]
    pub fn columns(&self) -> Vec<String> {
        self.columns.clone()
    }

    /// Number of rows in the result.
    #[napi(getter)]
    pub fn length(&self) -> u32 {
        self.rows.len() as u32
    }

    /// Get a single row by index as a plain JS object.
    #[napi]
    pub fn row(&self, env: Env, index: u32) -> napi::Result<napi::JsObject> {
        let idx = index as usize;
        if idx >= self.rows.len() {
            return Err(napi::Error::from_reason(format!(
                "row index out of range: {} >= {}",
                idx,
                self.rows.len()
            )));
        }
        self.row_to_object(&env, idx)
    }

    /// Convert all rows to an array of plain JS objects.
    #[napi(js_name = "toArray")]
    pub fn to_array(&self, env: Env) -> napi::Result<Vec<napi::JsObject>> {
        let mut result = Vec::with_capacity(self.rows.len());
        for i in 0..self.rows.len() {
            result.push(self.row_to_object(&env, i)?);
        }
        Ok(result)
    }
}

impl CylResult {
    /// Build a JS object for a single row.
    fn row_to_object(&self, env: &Env, row_idx: usize) -> napi::Result<napi::JsObject> {
        let mut obj = env.create_object()?;
        let row = &self.rows[row_idx];
        for (i, col) in self.columns.iter().enumerate() {
            let js_val = rust_to_js(env, &row[i])?;
            obj.set_named_property(col, js_val)?;
        }
        Ok(obj)
    }
}
