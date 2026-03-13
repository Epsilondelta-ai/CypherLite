// CypherLiteError definitions

/// All errors that can occur in CypherLite operations.
#[derive(thiserror::Error, Debug)]
pub enum CypherLiteError {
    /// Wrapper for standard I/O errors.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// A database page failed integrity checks.
    #[error("Corrupted page {page_id}: {reason}")]
    CorruptedPage {
        /// The page number that is corrupted.
        page_id: u32,
        /// Human-readable description of the corruption.
        reason: String,
    },

    /// A write transaction could not be acquired because another is active.
    #[error("Transaction conflict: write lock unavailable")]
    TransactionConflict,

    /// The buffer pool or disk is full.
    #[error("Out of space: buffer pool or disk full")]
    OutOfSpace,

    /// The database file does not start with the expected magic bytes.
    #[error("Invalid magic number")]
    InvalidMagicNumber,

    /// The database file format version is not supported.
    #[error("Unsupported version: found {found}, supported {supported}")]
    UnsupportedVersion {
        /// The version found in the file.
        found: u32,
        /// The version this build supports.
        supported: u32,
    },

    /// A checksum did not match the expected value.
    #[error("Checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch {
        /// The expected checksum value.
        expected: u64,
        /// The actual checksum value found.
        found: u64,
    },

    /// Serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// The requested node does not exist.
    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    /// The requested edge does not exist.
    #[error("Edge not found: {0}")]
    EdgeNotFound(u64),

    /// A Cypher query could not be parsed.
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        /// The 1-based line number where the error occurred.
        line: usize,
        /// The 1-based column number where the error occurred.
        column: usize,
        /// Description of the parse error.
        message: String,
    },

    /// A semantically invalid query was detected.
    #[error("Semantic error: {0}")]
    SemanticError(String),

    /// An error occurred during query execution.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// The query uses syntax not yet implemented.
    #[error("Unsupported syntax: {0}")]
    UnsupportedSyntax(String),

    /// A constraint (e.g. uniqueness) was violated.
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    /// A datetime string could not be parsed.
    #[error("Invalid datetime format: {0}")]
    InvalidDateTimeFormat(String),

    /// Attempt to write a system-managed property (prefixed with `_`).
    #[error("System property is read-only: {0}")]
    SystemPropertyReadOnly(String),

    /// The database requires features not compiled into this binary.
    #[error("Feature incompatible: database requires flags 0x{db_flags:08X}, compiled with 0x{compiled_flags:08X}")]
    FeatureIncompatible {
        /// The feature flags stored in the database header.
        db_flags: u32,
        /// The feature flags compiled into this binary.
        compiled_flags: u32,
    },

    /// The requested subgraph does not exist.
    #[cfg(feature = "subgraph")]
    #[error("Subgraph not found: {0}")]
    SubgraphNotFound(u64),

    /// An operation requires subgraph support but the feature is not compiled.
    #[cfg(feature = "subgraph")]
    #[error("Feature requires subgraph support (compile with --features subgraph)")]
    FeatureRequiresSubgraph,

    /// The requested hyperedge does not exist.
    #[cfg(feature = "hypergraph")]
    #[error("Hyperedge not found: {0}")]
    HyperEdgeNotFound(u64),
}

/// Convenience type alias for CypherLite operations.
pub type Result<T> = std::result::Result<T, CypherLiteError>;

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-PAGE-007: InvalidMagicNumber error exists
    #[test]
    fn test_invalid_magic_number_error() {
        let err = CypherLiteError::InvalidMagicNumber;
        assert_eq!(format!("{err}"), "Invalid magic number");
    }

    // REQ-PAGE-008: UnsupportedVersion error with found/supported
    #[test]
    fn test_unsupported_version_error() {
        let err = CypherLiteError::UnsupportedVersion {
            found: 99,
            supported: 1,
        };
        assert_eq!(
            format!("{err}"),
            "Unsupported version: found 99, supported 1"
        );
    }

    // REQ-WAL-007: Checksum mismatch error
    #[test]
    fn test_checksum_mismatch_error() {
        let err = CypherLiteError::ChecksumMismatch {
            expected: 100,
            found: 200,
        };
        assert_eq!(
            format!("{err}"),
            "Checksum mismatch: expected 100, found 200"
        );
    }

    // REQ-TX-010: Transaction conflict error
    #[test]
    fn test_transaction_conflict_error() {
        let err = CypherLiteError::TransactionConflict;
        assert_eq!(
            format!("{err}"),
            "Transaction conflict: write lock unavailable"
        );
    }

    // REQ-BUF-006: Out of space error
    #[test]
    fn test_out_of_space_error() {
        let err = CypherLiteError::OutOfSpace;
        assert_eq!(format!("{err}"), "Out of space: buffer pool or disk full");
    }

    #[test]
    fn test_corrupted_page_error() {
        let err = CypherLiteError::CorruptedPage {
            page_id: 5,
            reason: "bad header".to_string(),
        };
        assert_eq!(format!("{err}"), "Corrupted page 5: bad header");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: CypherLiteError = io_err.into();
        assert!(matches!(err, CypherLiteError::IoError(_)));
        assert!(format!("{err}").contains("file missing"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CypherLiteError>();
        assert_sync::<CypherLiteError>();
    }

    #[test]
    fn test_node_not_found_error() {
        let err = CypherLiteError::NodeNotFound(42);
        assert_eq!(format!("{err}"), "Node not found: 42");
    }

    #[test]
    fn test_edge_not_found_error() {
        let err = CypherLiteError::EdgeNotFound(99);
        assert_eq!(format!("{err}"), "Edge not found: 99");
    }

    // REQ-QUERY-001: Parse error with location
    #[test]
    fn test_parse_error() {
        let err = CypherLiteError::ParseError {
            line: 3,
            column: 10,
            message: "unexpected token".to_string(),
        };
        assert_eq!(
            format!("{err}"),
            "Parse error at line 3, column 10: unexpected token"
        );
    }

    // REQ-QUERY-002: Semantic error
    #[test]
    fn test_semantic_error() {
        let err = CypherLiteError::SemanticError("unknown label".to_string());
        assert_eq!(format!("{err}"), "Semantic error: unknown label");
    }

    // REQ-QUERY-003: Execution error
    #[test]
    fn test_execution_error() {
        let err = CypherLiteError::ExecutionError("division by zero".to_string());
        assert_eq!(format!("{err}"), "Execution error: division by zero");
    }

    // REQ-QUERY-004: Unsupported syntax
    #[test]
    fn test_unsupported_syntax_error() {
        let err = CypherLiteError::UnsupportedSyntax("MERGE".to_string());
        assert_eq!(format!("{err}"), "Unsupported syntax: MERGE");
    }

    // REQ-QUERY-005: Constraint violation
    #[test]
    fn test_constraint_violation_error() {
        let err = CypherLiteError::ConstraintViolation("unique key violated".to_string());
        assert_eq!(
            format!("{err}"),
            "Constraint violation: unique key violated"
        );
    }

    // V-003: SystemPropertyReadOnly error
    #[test]
    fn test_system_property_read_only_error() {
        let err = CypherLiteError::SystemPropertyReadOnly("_created_at".to_string());
        assert_eq!(
            format!("{err}"),
            "System property is read-only: _created_at"
        );
    }

    // U-002: InvalidDateTimeFormat error
    #[test]
    fn test_invalid_datetime_format_error() {
        let err = CypherLiteError::InvalidDateTimeFormat("bad input".to_string());
        assert_eq!(
            format!("{err}"),
            "Invalid datetime format: bad input"
        );
    }

    // AA-T4: FeatureIncompatible error
    #[test]
    fn test_feature_incompatible_error() {
        let err = CypherLiteError::FeatureIncompatible {
            db_flags: 0x03,
            compiled_flags: 0x01,
        };
        assert_eq!(
            format!("{err}"),
            "Feature incompatible: database requires flags 0x00000003, compiled with 0x00000001"
        );
    }

    // ======================================================================
    // GG-001: SubgraphNotFound error
    // ======================================================================

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_subgraph_not_found_error() {
        let err = CypherLiteError::SubgraphNotFound(42);
        assert_eq!(format!("{err}"), "Subgraph not found: 42");
    }

    #[cfg(feature = "subgraph")]
    #[test]
    fn test_feature_requires_subgraph_error() {
        let err = CypherLiteError::FeatureRequiresSubgraph;
        assert_eq!(
            format!("{err}"),
            "Feature requires subgraph support (compile with --features subgraph)"
        );
    }
}
