// BufferPool: LRU cache, pin/unpin, dirty tracking, eviction

use std::collections::{HashMap, VecDeque};

use cypherlite_core::{CypherLiteError, PageId, Result};

use super::PAGE_SIZE;

/// A cached page frame in the buffer pool.
#[derive(Debug)]
struct BufferFrame {
    page_id: PageId,
    data: [u8; PAGE_SIZE],
    dirty: bool,
    pin_count: u32,
}

/// LRU buffer pool for caching database pages in memory.
///
/// REQ-BUF-001: LRU cache, default 256 pages (1MB).
/// REQ-BUF-005: Active transaction pages are pinned (no eviction).
pub struct BufferPool {
    frames: HashMap<PageId, usize>, // page_id -> index in pool
    pool: Vec<BufferFrame>,
    lru_order: VecDeque<PageId>, // front = least recently used
    capacity: usize,
}

impl BufferPool {
    /// Create a new buffer pool with the given capacity in pages.
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: HashMap::with_capacity(capacity),
            pool: Vec::with_capacity(capacity),
            lru_order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Returns the pool capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of pages currently in the pool.
    pub fn size(&self) -> usize {
        self.pool.len()
    }

    /// Insert a page into the buffer pool. If the page is already cached,
    /// update its data.
    pub fn insert(&mut self, page_id: PageId, data: [u8; PAGE_SIZE]) {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.pool[idx].data = data;
            self.touch(page_id);
        } else {
            let idx = self.pool.len();
            self.pool.push(BufferFrame {
                page_id,
                data,
                dirty: false,
                pin_count: 0,
            });
            self.frames.insert(page_id, idx);
            self.lru_order.push_back(page_id);
        }
    }

    /// Fetch a page from the pool. Returns None if not cached.
    /// REQ-BUF-002: Returns None when page not in pool (caller must load from disk/WAL).
    pub fn get(&mut self, page_id: PageId) -> Option<&[u8; PAGE_SIZE]> {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.touch(page_id);
            Some(&self.pool[idx].data)
        } else {
            None
        }
    }

    /// Get a mutable reference to page data. Marks the page dirty.
    pub fn get_mut(&mut self, page_id: PageId) -> Option<&mut [u8; PAGE_SIZE]> {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.touch(page_id);
            self.pool[idx].dirty = true;
            Some(&mut self.pool[idx].data)
        } else {
            None
        }
    }

    /// Pin a page to prevent eviction.
    /// REQ-BUF-005: Pin active transaction pages.
    pub fn pin(&mut self, page_id: PageId) {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.pool[idx].pin_count += 1;
        }
    }

    /// Unpin a page, allowing eviction when unpinned.
    pub fn unpin(&mut self, page_id: PageId) {
        if let Some(&idx) = self.frames.get(&page_id) {
            if self.pool[idx].pin_count > 0 {
                self.pool[idx].pin_count -= 1;
            }
        }
    }

    /// Mark a page as dirty.
    /// REQ-BUF-004: Dirty pages must be written to WAL before eviction.
    pub fn mark_dirty(&mut self, page_id: PageId) {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.pool[idx].dirty = true;
        }
    }

    /// Returns true if the page is dirty.
    pub fn is_dirty(&self, page_id: PageId) -> bool {
        self.frames
            .get(&page_id)
            .map(|&idx| self.pool[idx].dirty)
            .unwrap_or(false)
    }

    /// Returns true if the page is pinned.
    pub fn is_pinned(&self, page_id: PageId) -> bool {
        self.frames
            .get(&page_id)
            .map(|&idx| self.pool[idx].pin_count > 0)
            .unwrap_or(false)
    }

    /// Check if the pool is full.
    pub fn is_full(&self) -> bool {
        self.pool.len() >= self.capacity
    }

    /// Evict the LRU unpinned page.
    /// REQ-BUF-003: Evict LRU unpinned page when pool full.
    /// REQ-BUF-006: If all pages pinned, returns OutOfSpace error.
    ///
    /// Returns the evicted (page_id, data, dirty) tuple so the caller can
    /// write dirty pages to WAL.
    pub fn evict(&mut self) -> Result<Option<(PageId, [u8; PAGE_SIZE], bool)>> {
        // Find first unpinned page in LRU order
        let evict_pos = self.lru_order.iter().position(|pid| {
            self.frames
                .get(pid)
                .map(|&idx| self.pool[idx].pin_count == 0)
                .unwrap_or(false)
        });

        match evict_pos {
            Some(pos) => {
                let page_id = self.lru_order.remove(pos).expect("lru entry");
                let idx = self.frames.remove(&page_id).expect("frame index");
                let frame = self.pool.swap_remove(idx);

                self.fix_swapped_frame(idx);
                Ok(Some((frame.page_id, frame.data, frame.dirty)))
            }
            None if self.pool.is_empty() => Ok(None),
            None => Err(CypherLiteError::OutOfSpace),
        }
    }

    /// Remove a specific page from the pool (used during invalidation).
    pub fn remove(&mut self, page_id: PageId) -> Option<([u8; PAGE_SIZE], bool)> {
        if let Some(idx) = self.frames.remove(&page_id) {
            let frame = self.pool.swap_remove(idx);
            self.fix_swapped_frame(idx);
            self.lru_order.retain(|p| *p != page_id);
            Some((frame.data, frame.dirty))
        } else {
            None
        }
    }

    /// Clear dirty flag for a page (after WAL write).
    pub fn clear_dirty(&mut self, page_id: PageId) {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.pool[idx].dirty = false;
        }
    }

    // After swap_remove at idx, update the displaced frame's index in the map.
    fn fix_swapped_frame(&mut self, idx: usize) {
        if idx < self.pool.len() {
            let swapped_page_id = self.pool[idx].page_id;
            self.frames.insert(swapped_page_id, idx);
        }
    }

    // Move page_id to the back (most recently used) of the LRU queue
    fn touch(&mut self, page_id: PageId) {
        self.lru_order.retain(|p| *p != page_id);
        self.lru_order.push_back(page_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-BUF-001: LRU cache with configurable capacity
    #[test]
    fn test_buffer_pool_creation() {
        let pool = BufferPool::new(256);
        assert_eq!(pool.capacity(), 256);
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_insert_and_fetch() {
        let mut pool = BufferPool::new(10);
        let mut data = [0u8; PAGE_SIZE];
        data[0] = 0xAB;
        pool.insert(PageId(2), data);

        let fetched = pool.get(PageId(2)).expect("should be cached");
        assert_eq!(fetched[0], 0xAB);
    }

    // REQ-BUF-002: Returns None when page not in pool
    #[test]
    fn test_fetch_missing_page_returns_none() {
        let mut pool = BufferPool::new(10);
        assert!(pool.get(PageId(99)).is_none());
    }

    // REQ-BUF-004: Dirty tracking
    #[test]
    fn test_mark_dirty() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        assert!(!pool.is_dirty(PageId(2)));
        pool.mark_dirty(PageId(2));
        assert!(pool.is_dirty(PageId(2)));
    }

    // REQ-BUF-005: Pin prevents eviction
    #[test]
    fn test_pin_and_unpin() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        assert!(!pool.is_pinned(PageId(2)));
        pool.pin(PageId(2));
        assert!(pool.is_pinned(PageId(2)));
        pool.unpin(PageId(2));
        assert!(!pool.is_pinned(PageId(2)));
    }

    // REQ-BUF-003: Evict LRU unpinned page
    #[test]
    fn test_evict_lru_unpinned() {
        let mut pool = BufferPool::new(3);
        pool.insert(PageId(2), [1u8; PAGE_SIZE]);
        pool.insert(PageId(3), [2u8; PAGE_SIZE]);
        pool.insert(PageId(4), [3u8; PAGE_SIZE]);

        let evicted = pool.evict().expect("evict").expect("some page");
        // LRU is PageId(2) since it was inserted first
        assert_eq!(evicted.0, PageId(2));
        assert_eq!(evicted.1[0], 1);
        assert!(!evicted.2); // not dirty
    }

    // REQ-BUF-003: Evict skips pinned pages
    #[test]
    fn test_evict_skips_pinned() {
        let mut pool = BufferPool::new(3);
        pool.insert(PageId(2), [1u8; PAGE_SIZE]);
        pool.insert(PageId(3), [2u8; PAGE_SIZE]);
        pool.insert(PageId(4), [3u8; PAGE_SIZE]);

        pool.pin(PageId(2)); // Pin LRU page

        let evicted = pool.evict().expect("evict").expect("some page");
        // Should skip PageId(2) and evict PageId(3)
        assert_eq!(evicted.0, PageId(3));
    }

    // REQ-BUF-006: All pages pinned -> OutOfSpace
    #[test]
    fn test_evict_all_pinned_returns_error() {
        let mut pool = BufferPool::new(2);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);
        pool.pin(PageId(2));
        pool.pin(PageId(3));

        let result = pool.evict();
        assert!(matches!(result, Err(CypherLiteError::OutOfSpace)));
    }

    // REQ-BUF-004: Dirty page eviction reports dirty flag
    #[test]
    fn test_evict_dirty_page_reports_dirty() {
        let mut pool = BufferPool::new(2);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);
        pool.mark_dirty(PageId(2));

        let evicted = pool.evict().expect("evict").expect("some page");
        assert_eq!(evicted.0, PageId(2));
        assert!(evicted.2); // dirty
    }

    #[test]
    fn test_get_mut_marks_dirty() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        assert!(!pool.is_dirty(PageId(2)));

        let data = pool.get_mut(PageId(2)).expect("get_mut");
        data[0] = 0xFF;
        assert!(pool.is_dirty(PageId(2)));
    }

    #[test]
    fn test_lru_ordering_updated_on_access() {
        let mut pool = BufferPool::new(3);
        pool.insert(PageId(2), [1u8; PAGE_SIZE]);
        pool.insert(PageId(3), [2u8; PAGE_SIZE]);
        pool.insert(PageId(4), [3u8; PAGE_SIZE]);

        // Access PageId(2) to make it most recently used
        pool.get(PageId(2));

        // Now LRU is PageId(3)
        let evicted = pool.evict().expect("evict").expect("page");
        assert_eq!(evicted.0, PageId(3));
    }

    #[test]
    fn test_remove_page() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0xAB; PAGE_SIZE]);
        assert_eq!(pool.size(), 1);

        let removed = pool.remove(PageId(2));
        assert!(removed.is_some());
        assert_eq!(pool.size(), 0);
        assert!(pool.get(PageId(2)).is_none());
    }

    #[test]
    fn test_clear_dirty() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.mark_dirty(PageId(2));
        assert!(pool.is_dirty(PageId(2)));
        pool.clear_dirty(PageId(2));
        assert!(!pool.is_dirty(PageId(2)));
    }

    #[test]
    fn test_is_full() {
        let mut pool = BufferPool::new(2);
        assert!(!pool.is_full());
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        assert!(!pool.is_full());
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);
        assert!(pool.is_full());
    }

    // REQ-BUF-007: User-configurable capacity
    #[test]
    fn test_custom_capacity() {
        let pool = BufferPool::new(1024);
        assert_eq!(pool.capacity(), 1024);
    }

    #[test]
    fn test_insert_update_existing() {
        let mut pool = BufferPool::new(10);
        pool.insert(PageId(2), [0xAA; PAGE_SIZE]);
        pool.insert(PageId(2), [0xBB; PAGE_SIZE]); // update
        let data = pool.get(PageId(2)).expect("cached");
        assert_eq!(data[0], 0xBB);
        assert_eq!(pool.size(), 1); // should not duplicate
    }

    #[test]
    fn test_evict_empty_pool_returns_none() {
        let mut pool = BufferPool::new(10);
        let result = pool.evict().expect("no error");
        assert!(result.is_none());
    }
}
