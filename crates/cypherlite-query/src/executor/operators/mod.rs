// Physical operator implementations
/// Aggregate operator (GROUP BY / count / etc.).
pub mod aggregate;
/// CREATE clause execution (nodes and edges).
pub mod create;
/// DELETE / DETACH DELETE execution.
pub mod delete;
/// Single-hop edge expansion.
pub mod expand;
/// Filter operator (WHERE predicates).
pub mod filter;
/// Hyperedge scan operator (hypergraph feature).
#[cfg(feature = "hypergraph")]
pub mod hyperedge_scan;
/// Index-based node lookup.
pub mod index_scan;
/// LIMIT / SKIP operators.
pub mod limit;
/// MERGE clause execution (match-or-create).
pub mod merge;
/// Full node scan (with optional label filter).
pub mod node_scan;
/// OPTIONAL MATCH (left-join) expansion.
pub mod optional_expand;
/// RETURN / WITH projection.
pub mod project;
/// SET property assignment.
pub mod set_props;
/// ORDER BY sort operator.
pub mod sort;
/// Subgraph scan operator (subgraph feature).
#[cfg(feature = "subgraph")]
pub mod subgraph_scan;
/// Temporal edge validity filtering (AT TIME / BETWEEN TIME).
pub mod temporal_filter;
/// Version-store scan for AT TIME / BETWEEN TIME queries.
pub mod temporal_scan;
/// UNWIND list expansion.
pub mod unwind;
/// Variable-length path expansion (BFS with depth bounds).
pub mod var_length_expand;
/// WITH clause operator.
pub mod with;
