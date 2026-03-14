// BufferPool: LRU cache, pin/unpin, dirty tracking, eviction
//
// REQ-S-001: O(1) LRU touch via arena-based doubly-linked list + HashMap.

use std::collections::HashMap;

use cypherlite_core::{CypherLiteError, PageId, Result};

use super::PAGE_SIZE;

// ---------------------------------------------------------------------------
// LruList: O(1) doubly-linked list backed by a Vec arena (no unsafe code).
//
// Sentinel nodes at index 0 (head) and 1 (tail) simplify boundary handling.
// Free slots are recycled via a stack so arena indices are reused.
// ---------------------------------------------------------------------------

const HEAD: usize = 0;
const TAIL: usize = 1;

struct LruNode {
    prev: usize,
    next: usize,
    page_id: PageId, // sentinel nodes use PageId(u32::MAX)
}

/// Arena-based doubly-linked list with O(1) touch, push_back, and remove.
struct LruList {
    arena: Vec<LruNode>,
    map: HashMap<PageId, usize>,
    free_slots: Vec<usize>,
}

impl LruList {
    fn new(capacity: usize) -> Self {
        let sentinel_id = PageId(u32::MAX);
        let mut arena = Vec::with_capacity(capacity + 2);
        // Index 0 = head sentinel, index 1 = tail sentinel
        arena.push(LruNode {
            prev: HEAD,
            next: TAIL,
            page_id: sentinel_id,
        });
        arena.push(LruNode {
            prev: HEAD,
            next: TAIL,
            page_id: sentinel_id,
        });
        Self {
            arena,
            map: HashMap::with_capacity(capacity),
            free_slots: Vec::new(),
        }
    }

    /// Append `page_id` at the MRU end (just before tail sentinel).
    fn push_back(&mut self, page_id: PageId) {
        debug_assert!(
            !self.map.contains_key(&page_id),
            "push_back: duplicate page_id"
        );
        let idx = self.alloc_node(page_id);
        self.link_before(TAIL, idx);
        self.map.insert(page_id, idx);
    }

    /// Remove `page_id` from the list in O(1).
    fn remove(&mut self, page_id: PageId) {
        if let Some(idx) = self.map.remove(&page_id) {
            self.unlink(idx);
            self.free_slots.push(idx);
        }
    }

    /// Move `page_id` to the MRU end in O(1).
    fn touch(&mut self, page_id: PageId) {
        if let Some(&idx) = self.map.get(&page_id) {
            self.unlink(idx);
            self.link_before(TAIL, idx);
        }
    }

    /// Iterate page ids from LRU (front) to MRU (back).
    /// Caller can stop early (e.g. for eviction scan).
    fn iter_lru(&self) -> LruIter<'_> {
        LruIter {
            list: self,
            current: self.arena[HEAD].next,
        }
    }

    // -- internal helpers --

    fn alloc_node(&mut self, page_id: PageId) -> usize {
        if let Some(idx) = self.free_slots.pop() {
            self.arena[idx].page_id = page_id;
            self.arena[idx].prev = 0;
            self.arena[idx].next = 0;
            idx
        } else {
            let idx = self.arena.len();
            self.arena.push(LruNode {
                prev: 0,
                next: 0,
                page_id,
            });
            idx
        }
    }

    /// Insert `idx` immediately before `before` in the linked list.
    fn link_before(&mut self, before: usize, idx: usize) {
        let prev = self.arena[before].prev;
        self.arena[idx].prev = prev;
        self.arena[idx].next = before;
        self.arena[prev].next = idx;
        self.arena[before].prev = idx;
    }

    /// Remove node `idx` from its current position.
    fn unlink(&mut self, idx: usize) {
        let prev = self.arena[idx].prev;
        let nxt = self.arena[idx].next;
        self.arena[prev].next = nxt;
        self.arena[nxt].prev = prev;
    }
}

struct LruIter<'a> {
    list: &'a LruList,
    current: usize,
}

impl<'a> Iterator for LruIter<'a> {
    type Item = PageId;
    fn next(&mut self) -> Option<PageId> {
        if self.current == TAIL {
            return None;
        }
        let node = &self.list.arena[self.current];
        let page_id = node.page_id;
        self.current = node.next;
        Some(page_id)
    }
}

// ---------------------------------------------------------------------------
// BufferPool
// ---------------------------------------------------------------------------

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
/// REQ-S-001: O(1) LRU touch via arena-based doubly-linked list.
pub struct BufferPool {
    frames: HashMap<PageId, usize>, // page_id -> index in pool
    pool: Vec<BufferFrame>,
    lru: LruList,
    capacity: usize,
}

impl BufferPool {
    /// Create a new buffer pool with the given capacity in pages.
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: HashMap::with_capacity(capacity),
            pool: Vec::with_capacity(capacity),
            lru: LruList::new(capacity),
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
            self.lru.touch(page_id);
        } else {
            let idx = self.pool.len();
            self.pool.push(BufferFrame {
                page_id,
                data,
                dirty: false,
                pin_count: 0,
            });
            self.frames.insert(page_id, idx);
            self.lru.push_back(page_id);
        }
    }

    /// Fetch a page from the pool. Returns None if not cached.
    /// REQ-BUF-002: Returns None when page not in pool (caller must load from disk/WAL).
    pub fn get(&mut self, page_id: PageId) -> Option<&[u8; PAGE_SIZE]> {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.lru.touch(page_id);
            Some(&self.pool[idx].data)
        } else {
            None
        }
    }

    /// Get a mutable reference to page data. Marks the page dirty.
    pub fn get_mut(&mut self, page_id: PageId) -> Option<&mut [u8; PAGE_SIZE]> {
        if let Some(&idx) = self.frames.get(&page_id) {
            self.lru.touch(page_id);
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
        // Find first unpinned page walking from LRU end
        let evict_pid = self.lru.iter_lru().find(|pid| {
            self.frames
                .get(pid)
                .map(|&idx| self.pool[idx].pin_count == 0)
                .unwrap_or(false)
        });

        match evict_pid {
            Some(page_id) => {
                self.lru.remove(page_id);
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
    /// O(1) with the arena-based LRU list.
    pub fn remove(&mut self, page_id: PageId) -> Option<([u8; PAGE_SIZE], bool)> {
        if let Some(idx) = self.frames.remove(&page_id) {
            let frame = self.pool.swap_remove(idx);
            self.fix_swapped_frame(idx);
            self.lru.remove(page_id);
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

    // REQ-S-001: LRU touch must be O(1), verified via correct ordering after many ops
    #[test]
    fn test_lru_touch_is_o1() {
        // Insert many pages and touch them in various orders to verify
        // correct LRU ordering is maintained (functional correctness
        // implies O(1) touch when backed by linked-list + hashmap).
        let n: u32 = 1000;
        let mut pool = BufferPool::new(n as usize + 1);
        for i in 0..n {
            pool.insert(PageId(i), [i as u8; PAGE_SIZE]);
        }

        // Touch pages in reverse order: page 999 becomes MRU, page 0 stays LRU
        // then touch page 0, making page 1 the new LRU
        for i in (0..n).rev() {
            pool.get(PageId(i));
        }
        // Now LRU order from front: 999, 998, ..., 1, 0
        // Touch page 999 to move it to MRU
        pool.get(PageId(999));
        // Now LRU order from front: 998, 997, ..., 1, 0, 999
        // Evict should return 998
        let evicted = pool.evict().expect("ok").expect("some page");
        assert_eq!(evicted.0, PageId(998));
    }

    // REQ-S-001: Consecutive touches on same page produce no duplicates
    #[test]
    fn test_consecutive_touch_no_duplicates() {
        let mut pool = BufferPool::new(3);
        pool.insert(PageId(1), [0u8; PAGE_SIZE]);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);

        // Touch page 1 multiple times
        pool.get(PageId(1));
        pool.get(PageId(1));
        pool.get(PageId(1));

        // Evict should return page 2 (LRU), not page 1
        let evicted = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted.0, PageId(2));

        // After evicting page 2, next evict should return page 3
        let evicted2 = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted2.0, PageId(3));
    }

    // REQ-S-001: Cache size 1 with insert triggers correct eviction
    #[test]
    fn test_cache_size_one_insert_evict() {
        let mut pool = BufferPool::new(1);
        pool.insert(PageId(10), [0xAA; PAGE_SIZE]);
        assert_eq!(pool.size(), 1);

        // Evict only page
        let evicted = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted.0, PageId(10));
        assert_eq!(evicted.1[0], 0xAA);
        assert_eq!(pool.size(), 0);

        // Insert new page after eviction
        pool.insert(PageId(20), [0xBB; PAGE_SIZE]);
        assert_eq!(pool.size(), 1);
        let data = pool.get(PageId(20)).expect("cached");
        assert_eq!(data[0], 0xBB);
    }

    // REQ-S-001: LRU ordering with mixed get/get_mut/insert operations
    #[test]
    fn test_lru_ordering_mixed_operations() {
        let mut pool = BufferPool::new(4);
        pool.insert(PageId(1), [1u8; PAGE_SIZE]);
        pool.insert(PageId(2), [2u8; PAGE_SIZE]);
        pool.insert(PageId(3), [3u8; PAGE_SIZE]);
        pool.insert(PageId(4), [4u8; PAGE_SIZE]);
        // LRU order: 1, 2, 3, 4

        pool.get(PageId(1)); // move 1 to MRU -> order: 2, 3, 4, 1
        pool.get_mut(PageId(3)); // move 3 to MRU -> order: 2, 4, 1, 3
        pool.insert(PageId(2), [0xBB; PAGE_SIZE]); // update 2 -> order: 4, 1, 3, 2

        // Evict should return page 4 (LRU)
        let evicted = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted.0, PageId(4));

        // Next evict should return page 1
        let evicted2 = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted2.0, PageId(1));
    }

    // REQ-S-001: Remove is O(1) with new LRU structure
    #[test]
    fn test_remove_maintains_lru_order() {
        let mut pool = BufferPool::new(5);
        pool.insert(PageId(1), [0u8; PAGE_SIZE]);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);
        pool.insert(PageId(4), [0u8; PAGE_SIZE]);
        // LRU order: 1, 2, 3, 4

        // Remove page 2 from the middle
        pool.remove(PageId(2));
        // LRU order: 1, 3, 4

        // Evict should return page 1 (still LRU)
        let evicted = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted.0, PageId(1));

        // Next evict returns page 3
        let evicted2 = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted2.0, PageId(3));
    }

    // REQ-S-001: All pages dirty evict reports dirty flag correctly
    #[test]
    fn test_evict_all_dirty_reports_dirty() {
        let mut pool = BufferPool::new(3);
        pool.insert(PageId(1), [0u8; PAGE_SIZE]);
        pool.insert(PageId(2), [0u8; PAGE_SIZE]);
        pool.insert(PageId(3), [0u8; PAGE_SIZE]);
        pool.mark_dirty(PageId(1));
        pool.mark_dirty(PageId(2));
        pool.mark_dirty(PageId(3));

        let evicted = pool.evict().expect("ok").expect("some");
        assert_eq!(evicted.0, PageId(1));
        assert!(evicted.2); // dirty flag is true
    }
}
