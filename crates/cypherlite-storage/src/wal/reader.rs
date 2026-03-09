// WalReader: DashMap index, page read routing (WAL-first)

use std::collections::HashMap;

use cypherlite_core::PageId;

use super::WalFrame;
use crate::page::PAGE_SIZE;

/// In-memory index mapping page IDs to their latest WAL frame data.
///
/// REQ-WAL-003: When reading page, check WAL index first.
pub struct WalReader {
    /// Maps page_id -> latest frame data for that page.
    index: HashMap<u32, WalFrame>,
    /// The frame number up to which this reader is consistent.
    snapshot_frame: u64,
}

impl WalReader {
    /// Create a new empty WAL reader.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            snapshot_frame: 0,
        }
    }

    /// Create a WAL reader with a specific snapshot point.
    /// REQ-TX-001: Snapshot current WAL frame index as read consistency point.
    pub fn with_snapshot(snapshot_frame: u64) -> Self {
        Self {
            index: HashMap::new(),
            snapshot_frame,
        }
    }

    /// Returns the snapshot frame number.
    pub fn snapshot_frame(&self) -> u64 {
        self.snapshot_frame
    }

    /// Index a committed frame. Only indexes frames up to snapshot_frame.
    pub fn index_frame(&mut self, frame: WalFrame) {
        if self.snapshot_frame == 0 || frame.frame_number <= self.snapshot_frame {
            // Latest frame for a page wins (later frames overwrite earlier ones)
            let supersedes = self
                .index
                .get(&frame.page_number)
                .is_none_or(|existing| existing.frame_number < frame.frame_number);
            if supersedes {
                self.index.insert(frame.page_number, frame);
            }
        }
    }

    /// Read a page from the WAL index.
    /// REQ-WAL-003: Returns Some if page found in WAL, None if must read from disk.
    pub fn read_page(&self, page_id: PageId) -> Option<&[u8; PAGE_SIZE]> {
        self.index.get(&page_id.0).map(|frame| &frame.page_data)
    }

    /// Check if a page exists in the WAL index.
    pub fn contains_page(&self, page_id: PageId) -> bool {
        self.index.contains_key(&page_id.0)
    }

    /// Returns the number of pages in the index.
    pub fn page_count(&self) -> usize {
        self.index.len()
    }

    /// Clear the WAL reader index (used after checkpoint).
    pub fn clear(&mut self) {
        self.index.clear();
    }

    /// Update snapshot frame (used when WAL index is atomically updated).
    /// REQ-TX-002: Update WAL index atomically on commit.
    pub fn set_snapshot_frame(&mut self, frame: u64) {
        self.snapshot_frame = frame;
    }

    /// Get all indexed page IDs.
    pub fn indexed_pages(&self) -> Vec<PageId> {
        self.index.keys().map(|&k| PageId(k)).collect()
    }
}

impl Default for WalReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(frame_number: u64, page_number: u32, byte: u8) -> WalFrame {
        WalFrame::new(frame_number, page_number, 100, 12345, [byte; PAGE_SIZE])
    }

    // REQ-WAL-003: Check WAL index first
    #[test]
    fn test_wal_reader_empty_returns_none() {
        let reader = WalReader::new();
        assert!(reader.read_page(PageId(5)).is_none());
    }

    #[test]
    fn test_index_and_read_page() {
        let mut reader = WalReader::new();
        reader.index_frame(make_frame(1, 5, 0xAB));
        let data = reader.read_page(PageId(5)).expect("found");
        assert_eq!(data[0], 0xAB);
    }

    // REQ-WAL-003: Latest frame for a page wins
    #[test]
    fn test_later_frame_overwrites_earlier() {
        let mut reader = WalReader::new();
        reader.index_frame(make_frame(1, 5, 0xAA));
        reader.index_frame(make_frame(2, 5, 0xBB));
        let data = reader.read_page(PageId(5)).expect("found");
        assert_eq!(data[0], 0xBB);
    }

    // REQ-TX-001: Snapshot isolation
    #[test]
    fn test_snapshot_isolation() {
        let mut reader = WalReader::with_snapshot(1);
        reader.index_frame(make_frame(1, 5, 0xAA)); // within snapshot
        reader.index_frame(make_frame(2, 5, 0xBB)); // beyond snapshot - should be ignored
        let data = reader.read_page(PageId(5)).expect("found");
        assert_eq!(data[0], 0xAA); // should see frame 1, not frame 2
    }

    #[test]
    fn test_contains_page() {
        let mut reader = WalReader::new();
        assert!(!reader.contains_page(PageId(5)));
        reader.index_frame(make_frame(1, 5, 0xAB));
        assert!(reader.contains_page(PageId(5)));
    }

    #[test]
    fn test_page_count() {
        let mut reader = WalReader::new();
        assert_eq!(reader.page_count(), 0);
        reader.index_frame(make_frame(1, 5, 0xAA));
        reader.index_frame(make_frame(2, 6, 0xBB));
        assert_eq!(reader.page_count(), 2);
    }

    #[test]
    fn test_clear_index() {
        let mut reader = WalReader::new();
        reader.index_frame(make_frame(1, 5, 0xAA));
        assert_eq!(reader.page_count(), 1);
        reader.clear();
        assert_eq!(reader.page_count(), 0);
    }

    #[test]
    fn test_set_snapshot_frame() {
        let mut reader = WalReader::new();
        assert_eq!(reader.snapshot_frame(), 0);
        reader.set_snapshot_frame(42);
        assert_eq!(reader.snapshot_frame(), 42);
    }

    #[test]
    fn test_indexed_pages() {
        let mut reader = WalReader::new();
        reader.index_frame(make_frame(1, 5, 0xAA));
        reader.index_frame(make_frame(2, 10, 0xBB));
        let mut pages = reader.indexed_pages();
        pages.sort();
        assert_eq!(pages, vec![PageId(5), PageId(10)]);
    }
}
