#![warn(missing_docs)]
//! CypherLite query engine: lexer, parser, semantic analyzer, planner, and executor.
//!
//! This crate implements the full Cypher query pipeline for the CypherLite
//! embedded graph database. It transforms Cypher query strings through a
//! multi-stage pipeline: lexical analysis (logos), recursive-descent parsing,
//! semantic validation, logical plan generation, and Volcano/Iterator-model
//! execution.

/// Public API entry points: database handle, query execution, and result types.
pub mod api;
/// Volcano/Iterator-model query executor with operator tree evaluation.
pub mod executor;
/// Lexical analyzer (tokenizer) built on the logos crate.
pub mod lexer;
/// Recursive-descent Cypher parser producing an AST.
pub mod parser;
/// Logical query planner and optimizer.
pub mod planner;
/// Semantic analysis: scope resolution, type checking, and validation.
pub mod semantic;

pub use api::{CypherLite, FromValue, QueryResult, Row, Transaction};
pub use executor::{Params, Value};
