/// LRU buffer pool for caching database pages in memory.
pub mod buffer_pool;
/// Page-level I/O and database file management.
pub mod page_manager;

/// Page size constant: 4096 bytes.
pub const PAGE_SIZE: usize = 4096;

/// Magic bytes for CypherLite database files: "CYLT" (0x43594C54).
pub const MAGIC: u32 = 0x4359_4C54;

/// Current database format version.
pub const FORMAT_VERSION: u32 = 2;

/// Header page is always at page 0.
pub const HEADER_PAGE_ID: u32 = 0;

/// Free Space Map is always at page 1.
pub const FSM_PAGE_ID: u32 = 1;

/// First data page index.
pub const FIRST_DATA_PAGE: u32 = 2;

/// Maximum pages trackable by a single FSM page (4096 * 8 bits).
pub const FSM_MAX_PAGES: u32 = PAGE_SIZE as u32 * 8;

/// Types of pages in the database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageType {
    /// Database file header page (page 0).
    Header = 0,
    /// Free space map bitmap page.
    FreeSpaceMap = 1,
    /// Interior node of a B-tree index.
    BTreeInterior = 2,
    /// Leaf node of a B-tree index.
    BTreeLeaf = 3,
    /// Overflow page for large records.
    Overflow = 4,
    /// General data page.
    Data = 5,
}

/// 32-byte header at the start of each data page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHeader {
    /// Discriminant indicating the page type.
    pub page_type: u8,
    /// Bit flags for page-level metadata.
    pub flags: u8,
    /// Byte offset where free space begins.
    pub free_start: u16,
    /// Byte offset where free space ends.
    pub free_end: u16,
    /// Page ID of the overflow continuation page (0 = none).
    pub overflow_page: u32,
    /// Number of items stored in this page.
    pub item_count: u16,
    /// Reserved bytes for future use (pads header to 32 bytes).
    pub _reserved: [u8; 20],
}

impl PageHeader {
    /// Size of the page header in bytes.
    pub const SIZE: usize = 32;

    /// Creates a new page header with the given type.
    pub fn new(page_type: PageType) -> Self {
        Self {
            page_type: page_type as u8,
            flags: 0,
            free_start: Self::SIZE as u16,
            free_end: PAGE_SIZE as u16,
            overflow_page: 0,
            item_count: 0,
            _reserved: [0u8; 20],
        }
    }

    /// Serialize page header into the first 32 bytes of a buffer.
    pub fn write_to(&self, buf: &mut [u8]) {
        debug_assert!(buf.len() >= Self::SIZE);
        buf[0] = self.page_type;
        buf[1] = self.flags;
        buf[2..4].copy_from_slice(&self.free_start.to_le_bytes());
        buf[4..6].copy_from_slice(&self.free_end.to_le_bytes());
        buf[6..10].copy_from_slice(&self.overflow_page.to_le_bytes());
        buf[10..12].copy_from_slice(&self.item_count.to_le_bytes());
        buf[12..32].copy_from_slice(&self._reserved);
    }

    /// Deserialize page header from the first 32 bytes of a buffer.
    pub fn read_from(buf: &[u8]) -> Self {
        debug_assert!(buf.len() >= Self::SIZE);
        let mut reserved = [0u8; 20];
        reserved.copy_from_slice(&buf[12..32]);
        Self {
            page_type: buf[0],
            flags: buf[1],
            free_start: u16::from_le_bytes([buf[2], buf[3]]),
            free_end: u16::from_le_bytes([buf[4], buf[5]]),
            overflow_page: u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]),
            item_count: u16::from_le_bytes([buf[10], buf[11]]),
            _reserved: reserved,
        }
    }
}

/// Database file header stored at page 0.
#[derive(Debug, Clone)]
pub struct DatabaseHeader {
    /// Magic number identifying the file format.
    pub magic: u32,
    /// Database format version number.
    pub version: u32,
    /// Total number of pages in the database file.
    pub page_count: u32,
    /// Root page of the node B-tree index.
    pub root_node_page: u32,
    /// Root page of the edge B-tree index.
    pub root_edge_page: u32,
    /// Next available node ID.
    pub next_node_id: u64,
    /// Next available edge ID.
    pub next_edge_id: u64,
    /// Root page of the version store (0 = no version store allocated).
    /// Added in format version 2, stored at bytes 36-43.
    pub version_store_root_page: u64,
}

impl DatabaseHeader {
    /// Creates a new database header with default values.
    pub fn new() -> Self {
        Self {
            magic: MAGIC,
            version: FORMAT_VERSION,
            page_count: FIRST_DATA_PAGE,
            root_node_page: 0,
            root_edge_page: 0,
            next_node_id: 1,
            next_edge_id: 1,
            version_store_root_page: 0,
        }
    }

    /// Serialize the database header into a 4096-byte page.
    pub fn to_page(&self) -> [u8; PAGE_SIZE] {
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&self.magic.to_le_bytes());
        page[4..8].copy_from_slice(&self.version.to_le_bytes());
        page[8..12].copy_from_slice(&self.page_count.to_le_bytes());
        page[12..16].copy_from_slice(&self.root_node_page.to_le_bytes());
        page[16..20].copy_from_slice(&self.root_edge_page.to_le_bytes());
        page[20..28].copy_from_slice(&self.next_node_id.to_le_bytes());
        page[28..36].copy_from_slice(&self.next_edge_id.to_le_bytes());
        // W-004: version_store_root_page at bytes 36-43
        page[36..44].copy_from_slice(&self.version_store_root_page.to_le_bytes());
        page
    }

    /// Deserialize the database header from a 4096-byte page.
    /// Supports both v1 (without version_store_root_page) and v2 formats.
    pub fn from_page(page: &[u8; PAGE_SIZE]) -> Self {
        let version = u32::from_le_bytes([page[4], page[5], page[6], page[7]]);

        // W-004: Auto-migrate v1 headers (bytes 36-43 are zero = no version store)
        let version_store_root_page = if version >= 2 {
            u64::from_le_bytes([
                page[36], page[37], page[38], page[39], page[40], page[41], page[42], page[43],
            ])
        } else {
            0 // v1 headers have no version store field
        };

        Self {
            magic: u32::from_le_bytes([page[0], page[1], page[2], page[3]]),
            version,
            page_count: u32::from_le_bytes([page[8], page[9], page[10], page[11]]),
            root_node_page: u32::from_le_bytes([page[12], page[13], page[14], page[15]]),
            root_edge_page: u32::from_le_bytes([page[16], page[17], page[18], page[19]]),
            next_node_id: u64::from_le_bytes([
                page[20], page[21], page[22], page[23], page[24], page[25], page[26], page[27],
            ]),
            next_edge_id: u64::from_le_bytes([
                page[28], page[29], page[30], page[31], page[32], page[33], page[34], page[35],
            ]),
            version_store_root_page,
        }
    }
}

impl Default for DatabaseHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-PAGE-001: 4KB page size
    #[test]
    fn test_page_size_is_4096() {
        assert_eq!(PAGE_SIZE, 4096);
    }

    // REQ-PAGE-002: Magic is CYLT (0x43594C54)
    #[test]
    fn test_magic_bytes() {
        assert_eq!(MAGIC, 0x4359_4C54);
        let bytes = MAGIC.to_be_bytes();
        assert_eq!(&bytes, b"CYLT");
    }

    // REQ-PAGE-002: Header at page 0
    #[test]
    fn test_header_page_id_is_zero() {
        assert_eq!(HEADER_PAGE_ID, 0);
    }

    // REQ-PAGE-003: FSM at page 1
    #[test]
    fn test_fsm_page_id_is_one() {
        assert_eq!(FSM_PAGE_ID, 1);
    }

    // REQ-PAGE-006: PageHeader is 32 bytes
    #[test]
    fn test_page_header_size_is_32() {
        assert_eq!(PageHeader::SIZE, 32);
    }

    #[test]
    fn test_page_header_new() {
        let hdr = PageHeader::new(PageType::BTreeLeaf);
        assert_eq!(hdr.page_type, PageType::BTreeLeaf as u8);
        assert_eq!(hdr.free_start, 32);
        assert_eq!(hdr.free_end, 4096);
        assert_eq!(hdr.item_count, 0);
    }

    #[test]
    fn test_page_header_roundtrip() {
        let hdr = PageHeader::new(PageType::BTreeInterior);
        let mut buf = [0u8; PAGE_SIZE];
        hdr.write_to(&mut buf);
        let decoded = PageHeader::read_from(&buf);
        assert_eq!(hdr, decoded);
    }

    // REQ-PAGE-002: DatabaseHeader contains magic, version, root pointer, page count
    #[test]
    fn test_database_header_new() {
        let hdr = DatabaseHeader::new();
        assert_eq!(hdr.magic, MAGIC);
        assert_eq!(hdr.version, FORMAT_VERSION);
        assert_eq!(hdr.page_count, FIRST_DATA_PAGE);
        assert_eq!(hdr.next_node_id, 1);
        assert_eq!(hdr.next_edge_id, 1);
        assert_eq!(hdr.version_store_root_page, 0);
    }

    #[test]
    fn test_database_header_roundtrip() {
        let hdr = DatabaseHeader {
            magic: MAGIC,
            version: FORMAT_VERSION,
            page_count: 100,
            root_node_page: 5,
            root_edge_page: 10,
            next_node_id: 42,
            next_edge_id: 99,
            version_store_root_page: 0,
        };
        let page = hdr.to_page();
        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.magic, MAGIC);
        assert_eq!(decoded.version, FORMAT_VERSION);
        assert_eq!(decoded.page_count, 100);
        assert_eq!(decoded.root_node_page, 5);
        assert_eq!(decoded.root_edge_page, 10);
        assert_eq!(decoded.next_node_id, 42);
        assert_eq!(decoded.next_edge_id, 99);
        assert_eq!(decoded.version_store_root_page, 0);
    }

    // W-004: DatabaseHeader with version_store_root_page
    #[test]
    fn test_database_header_version_store_root_page() {
        let hdr = DatabaseHeader {
            version_store_root_page: 42,
            ..DatabaseHeader::new()
        };
        let page = hdr.to_page();
        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.version_store_root_page, 42);
    }

    // W-004: Auto-migrate v1 header (version_store_root_page = 0)
    #[test]
    fn test_database_header_v1_migration() {
        // Create a v1 header (no version_store_root_page field, bytes 36-43 are zeros)
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        page[4..8].copy_from_slice(&1u32.to_le_bytes()); // version 1
        page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
        page[20..28].copy_from_slice(&1u64.to_le_bytes());
        page[28..36].copy_from_slice(&1u64.to_le_bytes());
        // bytes 36-43 are zeros (no version store)

        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.version, 1); // preserved as-is in from_page
        assert_eq!(decoded.version_store_root_page, 0); // auto-migrated to 0
    }

    // W-004: FORMAT_VERSION is now 2
    #[test]
    fn test_format_version_is_2() {
        assert_eq!(FORMAT_VERSION, 2);
    }

    // REQ-PAGE-003: FSM bitmap can track 32768 pages
    #[test]
    fn test_fsm_max_pages() {
        assert_eq!(FSM_MAX_PAGES, 32768);
    }

    #[test]
    fn test_page_type_variants() {
        assert_eq!(PageType::Header as u8, 0);
        assert_eq!(PageType::FreeSpaceMap as u8, 1);
        assert_eq!(PageType::BTreeInterior as u8, 2);
        assert_eq!(PageType::BTreeLeaf as u8, 3);
        assert_eq!(PageType::Overflow as u8, 4);
        assert_eq!(PageType::Data as u8, 5);
    }
}
