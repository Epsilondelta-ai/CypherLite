// CypherLiteError definitions

/// All errors that can occur in CypherLite operations.
#[derive(thiserror::Error, Debug)]
pub enum CypherLiteError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Corrupted page {page_id}: {reason}")]
    CorruptedPage { page_id: u32, reason: String },

    #[error("Transaction conflict: write lock unavailable")]
    TransactionConflict,

    #[error("Out of space: buffer pool or disk full")]
    OutOfSpace,

    #[error("Invalid magic number")]
    InvalidMagicNumber,

    #[error("Unsupported version: found {found}, supported {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },

    #[error("Checksum mismatch: expected {expected}, found {found}")]
    ChecksumMismatch { expected: u64, found: u64 },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    #[error("Edge not found: {0}")]
    EdgeNotFound(u64),
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
}
