// Transaction trait definitions

/// A read-only view of the database at a specific point in time.
pub trait TransactionView {
    /// Returns the WAL frame number representing this transaction's snapshot.
    fn snapshot_frame(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTxView {
        frame: u64,
    }

    impl TransactionView for MockTxView {
        fn snapshot_frame(&self) -> u64 {
            self.frame
        }
    }

    // REQ-TX-001: Read transactions capture snapshot point
    #[test]
    fn test_transaction_view_snapshot_frame() {
        let view = MockTxView { frame: 42 };
        assert_eq!(view.snapshot_frame(), 42);
    }

    #[test]
    fn test_transaction_view_zero_frame() {
        let view = MockTxView { frame: 0 };
        assert_eq!(view.snapshot_frame(), 0);
    }

    // Verify trait is object-safe
    #[test]
    fn test_transaction_view_is_object_safe() {
        let view: Box<dyn TransactionView> = Box::new(MockTxView { frame: 10 });
        assert_eq!(view.snapshot_frame(), 10);
    }
}
