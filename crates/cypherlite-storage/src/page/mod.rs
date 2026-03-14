/// LRU buffer pool for caching database pages in memory.
pub mod buffer_pool;
/// Page-level I/O and database file management.
pub mod page_manager;

/// Page size constant: 4096 bytes.
pub const PAGE_SIZE: usize = 4096;

/// Magic bytes for CypherLite database files: "CYLT" (0x43594C54).
pub const MAGIC: u32 = 0x4359_4C54;

/// Current database format version.
#[cfg(feature = "hypergraph")]
pub const FORMAT_VERSION: u32 = 5;
/// Current database format version.
#[cfg(all(feature = "subgraph", not(feature = "hypergraph")))]
pub const FORMAT_VERSION: u32 = 4;
/// Current database format version.
#[cfg(not(feature = "subgraph"))]
pub const FORMAT_VERSION: u32 = 3;

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
    /// Bit flags for enabled database features.
    /// Added in format version 3, stored at bytes 44-47.
    /// Bit 0: temporal-core, Bit 1: temporal-edge, Bit 2: subgraph.
    pub feature_flags: u32,
    /// Root page of the subgraph store (0 = no subgraph store allocated).
    /// Added in format version 4, stored at bytes 48-55.
    #[cfg(feature = "subgraph")]
    pub subgraph_root_page: u64,
    /// Next available subgraph ID.
    /// Added in format version 4, stored at bytes 56-63.
    #[cfg(feature = "subgraph")]
    pub next_subgraph_id: u64,
    /// Root page of the hyperedge store (0 = no hyperedge store allocated).
    /// Added in format version 5, stored at bytes 64-71.
    #[cfg(feature = "hypergraph")]
    pub hyperedge_root_page: u64,
    /// Next available hyperedge ID.
    /// Added in format version 5, stored at bytes 72-79.
    #[cfg(feature = "hypergraph")]
    pub next_hyperedge_id: u64,
}

impl DatabaseHeader {
    /// Feature flag bit: temporal-core is enabled.
    pub const FLAG_TEMPORAL_CORE: u32 = 1 << 0;
    /// Feature flag bit: temporal-edge is enabled.
    pub const FLAG_TEMPORAL_EDGE: u32 = 1 << 1;
    /// Feature flag bit: subgraph is enabled.
    #[cfg(feature = "subgraph")]
    pub const FLAG_SUBGRAPH: u32 = 1 << 2;
    /// Feature flag bit: hypergraph is enabled.
    #[cfg(feature = "hypergraph")]
    pub const FLAG_HYPERGRAPH: u32 = 1 << 3;

    /// Creates a new database header with default values.
    /// Feature flags are set based on compiled Cargo features.
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
            feature_flags: Self::compiled_feature_flags(),
            #[cfg(feature = "subgraph")]
            subgraph_root_page: 0,
            #[cfg(feature = "subgraph")]
            next_subgraph_id: 1,
            #[cfg(feature = "hypergraph")]
            hyperedge_root_page: 0,
            #[cfg(feature = "hypergraph")]
            next_hyperedge_id: 1,
        }
    }

    /// Returns feature flags based on the current compilation features.
    pub fn compiled_feature_flags() -> u32 {
        let mut flags = 0u32;
        // temporal-core is the default feature, always on when compiled with defaults
        flags |= Self::FLAG_TEMPORAL_CORE;
        #[cfg(feature = "temporal-edge")]
        {
            flags |= Self::FLAG_TEMPORAL_EDGE;
        }
        #[cfg(feature = "subgraph")]
        {
            flags |= Self::FLAG_SUBGRAPH;
        }
        #[cfg(feature = "hypergraph")]
        {
            flags |= Self::FLAG_HYPERGRAPH;
        }
        flags
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
        // AA-T2: feature_flags at bytes 44-47
        page[44..48].copy_from_slice(&self.feature_flags.to_le_bytes());
        // GG-003: subgraph_root_page at bytes 48-55, next_subgraph_id at bytes 56-63
        #[cfg(feature = "subgraph")]
        {
            page[48..56].copy_from_slice(&self.subgraph_root_page.to_le_bytes());
            page[56..64].copy_from_slice(&self.next_subgraph_id.to_le_bytes());
        }
        // HH-005: hyperedge_root_page at bytes 64-71, next_hyperedge_id at bytes 72-79
        #[cfg(feature = "hypergraph")]
        {
            page[64..72].copy_from_slice(&self.hyperedge_root_page.to_le_bytes());
            page[72..80].copy_from_slice(&self.next_hyperedge_id.to_le_bytes());
        }
        page
    }

    /// Deserialize the database header from a 4096-byte page.
    /// Supports v1, v2, and v3 formats with auto-migration.
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

        // AA-T2: feature_flags at bytes 44-47 (v3+)
        let feature_flags = if version >= 3 {
            u32::from_le_bytes([page[44], page[45], page[46], page[47]])
        } else {
            // Auto-migrate: v1/v2 databases default to temporal-core only
            Self::FLAG_TEMPORAL_CORE
        };

        // GG-003: subgraph fields at bytes 48-55, 56-63 (v4+)
        #[cfg(feature = "subgraph")]
        let subgraph_root_page = if version >= 4 {
            u64::from_le_bytes([
                page[48], page[49], page[50], page[51], page[52], page[53], page[54], page[55],
            ])
        } else {
            0 // Auto-migrate: pre-v4 databases have no subgraph store
        };

        #[cfg(feature = "subgraph")]
        let next_subgraph_id = if version >= 4 {
            u64::from_le_bytes([
                page[56], page[57], page[58], page[59], page[60], page[61], page[62], page[63],
            ])
        } else {
            0 // Auto-migrate: pre-v4 databases have no subgraph IDs
        };

        // HH-005: hyperedge fields at bytes 64-71, 72-79 (v5+)
        #[cfg(feature = "hypergraph")]
        let hyperedge_root_page = if version >= 5 {
            u64::from_le_bytes([
                page[64], page[65], page[66], page[67], page[68], page[69], page[70], page[71],
            ])
        } else {
            0 // Auto-migrate: pre-v5 databases have no hyperedge store
        };

        #[cfg(feature = "hypergraph")]
        let next_hyperedge_id = if version >= 5 {
            u64::from_le_bytes([
                page[72], page[73], page[74], page[75], page[76], page[77], page[78], page[79],
            ])
        } else {
            0 // Auto-migrate: pre-v5 databases have no hyperedge IDs
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
            feature_flags,
            #[cfg(feature = "subgraph")]
            subgraph_root_page,
            #[cfg(feature = "subgraph")]
            next_subgraph_id,
            #[cfg(feature = "hypergraph")]
            hyperedge_root_page,
            #[cfg(feature = "hypergraph")]
            next_hyperedge_id,
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
    #[allow(clippy::needless_update)]
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
            feature_flags: DatabaseHeader::FLAG_TEMPORAL_CORE,
            ..DatabaseHeader::new()
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
        assert_eq!(decoded.feature_flags, DatabaseHeader::FLAG_TEMPORAL_CORE);
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

    // AA-T2: FORMAT_VERSION is now 3 (without subgraph feature)
    #[cfg(not(feature = "subgraph"))]
    #[test]
    fn test_format_version_is_3() {
        assert_eq!(FORMAT_VERSION, 3);
    }

    // AA-T2: feature_flags field in DatabaseHeader
    #[test]
    fn test_database_header_feature_flags_roundtrip() {
        let hdr = DatabaseHeader {
            feature_flags: DatabaseHeader::FLAG_TEMPORAL_CORE | DatabaseHeader::FLAG_TEMPORAL_EDGE,
            ..DatabaseHeader::new()
        };
        let page = hdr.to_page();
        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(
            decoded.feature_flags,
            DatabaseHeader::FLAG_TEMPORAL_CORE | DatabaseHeader::FLAG_TEMPORAL_EDGE
        );
    }

    // AA-T3: v2 headers auto-migrate with feature_flags = FLAG_TEMPORAL_CORE
    #[test]
    fn test_database_header_v2_migration_feature_flags() {
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        page[4..8].copy_from_slice(&2u32.to_le_bytes()); // version 2
        page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
        page[20..28].copy_from_slice(&1u64.to_le_bytes());
        page[28..36].copy_from_slice(&1u64.to_le_bytes());
        // bytes 44-47 are zeros (no feature_flags in v2)

        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.version, 2);
        // Should auto-migrate to temporal-core
        assert_eq!(decoded.feature_flags, DatabaseHeader::FLAG_TEMPORAL_CORE);
    }

    // AA-T2: compiled_feature_flags returns at least temporal-core
    #[test]
    fn test_compiled_feature_flags() {
        let flags = DatabaseHeader::compiled_feature_flags();
        assert!(flags & DatabaseHeader::FLAG_TEMPORAL_CORE != 0);
    }

    // AA-T2: new() sets feature_flags from compiled features
    #[test]
    fn test_database_header_new_sets_feature_flags() {
        let hdr = DatabaseHeader::new();
        assert!(hdr.feature_flags & DatabaseHeader::FLAG_TEMPORAL_CORE != 0);
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

    // ======================================================================
    // GG-003: DatabaseHeader v4 with subgraph fields
    // ======================================================================

    #[cfg(feature = "subgraph")]
    mod subgraph_header_tests {
        use super::*;

        // GG-003: FORMAT_VERSION bumped to 4 when subgraph feature is compiled
        #[cfg(not(feature = "hypergraph"))]
        #[test]
        fn test_format_version_is_4() {
            assert_eq!(FORMAT_VERSION, 4);
        }

        // GG-003: FLAG_SUBGRAPH is bit 2 (0x04)
        #[test]
        fn test_flag_subgraph_constant() {
            assert_eq!(DatabaseHeader::FLAG_SUBGRAPH, 0x04);
        }

        // GG-003: New header fields have correct defaults
        #[test]
        fn test_database_header_new_subgraph_fields() {
            let hdr = DatabaseHeader::new();
            assert_eq!(hdr.subgraph_root_page, 0);
            assert_eq!(hdr.next_subgraph_id, 1);
            // Should include FLAG_SUBGRAPH in compiled flags
            assert!(hdr.feature_flags & DatabaseHeader::FLAG_SUBGRAPH != 0);
        }

        // GG-003: subgraph_root_page roundtrip
        #[test]
        fn test_subgraph_root_page_roundtrip() {
            let hdr = DatabaseHeader {
                subgraph_root_page: 42,
                ..DatabaseHeader::new()
            };
            let page = hdr.to_page();
            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.subgraph_root_page, 42);
        }

        // GG-003: next_subgraph_id roundtrip
        #[test]
        fn test_next_subgraph_id_roundtrip() {
            let hdr = DatabaseHeader {
                next_subgraph_id: 999,
                ..DatabaseHeader::new()
            };
            let page = hdr.to_page();
            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.next_subgraph_id, 999);
        }

        // GG-003: Full header roundtrip with all fields
        #[test]
        #[allow(clippy::needless_update)]
        fn test_database_header_v4_full_roundtrip() {
            let hdr = DatabaseHeader {
                magic: MAGIC,
                version: FORMAT_VERSION,
                page_count: 100,
                root_node_page: 5,
                root_edge_page: 10,
                next_node_id: 42,
                next_edge_id: 99,
                version_store_root_page: 7,
                feature_flags: DatabaseHeader::FLAG_TEMPORAL_CORE
                    | DatabaseHeader::FLAG_TEMPORAL_EDGE
                    | DatabaseHeader::FLAG_SUBGRAPH,
                subgraph_root_page: 15,
                next_subgraph_id: 200,
                ..DatabaseHeader::new()
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
            assert_eq!(decoded.version_store_root_page, 7);
            assert_eq!(decoded.subgraph_root_page, 15);
            assert_eq!(decoded.next_subgraph_id, 200);
            assert_eq!(
                decoded.feature_flags,
                DatabaseHeader::FLAG_TEMPORAL_CORE
                    | DatabaseHeader::FLAG_TEMPORAL_EDGE
                    | DatabaseHeader::FLAG_SUBGRAPH
            );
        }

        // GG-003: v3->v4 auto-migration (subgraph fields default to 0)
        #[test]
        fn test_database_header_v3_migration() {
            let mut page = [0u8; PAGE_SIZE];
            page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
            page[4..8].copy_from_slice(&3u32.to_le_bytes()); // version 3
            page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
            page[20..28].copy_from_slice(&1u64.to_le_bytes());
            page[28..36].copy_from_slice(&1u64.to_le_bytes());
            // v3 feature_flags at bytes 44-47
            let flags = DatabaseHeader::FLAG_TEMPORAL_CORE | DatabaseHeader::FLAG_TEMPORAL_EDGE;
            page[44..48].copy_from_slice(&flags.to_le_bytes());
            // bytes 48-55, 56-63 are zeros (no subgraph fields in v3)

            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.version, 3);
            assert_eq!(decoded.subgraph_root_page, 0); // auto-migrated
            assert_eq!(decoded.next_subgraph_id, 0); // auto-migrated
        }

        // GG-003: compiled_feature_flags includes FLAG_SUBGRAPH
        #[test]
        fn test_compiled_feature_flags_includes_subgraph() {
            let flags = DatabaseHeader::compiled_feature_flags();
            assert!(flags & DatabaseHeader::FLAG_SUBGRAPH != 0);
        }
    }

    // ======================================================================
    // HH-005: DatabaseHeader v5 with hyperedge fields
    // ======================================================================

    #[cfg(feature = "hypergraph")]
    mod hypergraph_header_tests {
        use super::*;

        // HH-005: FORMAT_VERSION bumped to 5 when hypergraph feature is compiled
        #[test]
        fn test_format_version_is_5_with_hypergraph() {
            assert_eq!(FORMAT_VERSION, 5);
        }

        // HH-005: FLAG_HYPERGRAPH is bit 3 (0x08)
        #[test]
        fn test_flag_hypergraph_constant() {
            assert_eq!(DatabaseHeader::FLAG_HYPERGRAPH, 0x08);
        }

        // HH-005: New header fields have correct defaults
        #[test]
        fn test_database_header_new_hypergraph_fields() {
            let hdr = DatabaseHeader::new();
            assert_eq!(hdr.hyperedge_root_page, 0);
            assert_eq!(hdr.next_hyperedge_id, 1);
            // Should include FLAG_HYPERGRAPH in compiled flags
            assert!(hdr.feature_flags & DatabaseHeader::FLAG_HYPERGRAPH != 0);
        }

        // HH-005: hyperedge_root_page roundtrip
        #[test]
        fn test_hyperedge_root_page_roundtrip() {
            let hdr = DatabaseHeader {
                hyperedge_root_page: 77,
                ..DatabaseHeader::new()
            };
            let page = hdr.to_page();
            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.hyperedge_root_page, 77);
        }

        // HH-005: next_hyperedge_id roundtrip
        #[test]
        fn test_next_hyperedge_id_roundtrip() {
            let hdr = DatabaseHeader {
                next_hyperedge_id: 500,
                ..DatabaseHeader::new()
            };
            let page = hdr.to_page();
            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.next_hyperedge_id, 500);
        }

        // HH-005: Full header roundtrip with all v5 fields
        #[test]
        fn test_database_header_v5_full_roundtrip() {
            let hdr = DatabaseHeader {
                magic: MAGIC,
                version: FORMAT_VERSION,
                page_count: 100,
                root_node_page: 5,
                root_edge_page: 10,
                next_node_id: 42,
                next_edge_id: 99,
                version_store_root_page: 7,
                feature_flags: DatabaseHeader::FLAG_TEMPORAL_CORE
                    | DatabaseHeader::FLAG_TEMPORAL_EDGE
                    | DatabaseHeader::FLAG_SUBGRAPH
                    | DatabaseHeader::FLAG_HYPERGRAPH,
                subgraph_root_page: 15,
                next_subgraph_id: 200,
                hyperedge_root_page: 25,
                next_hyperedge_id: 300,
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
            assert_eq!(decoded.version_store_root_page, 7);
            assert_eq!(decoded.subgraph_root_page, 15);
            assert_eq!(decoded.next_subgraph_id, 200);
            assert_eq!(decoded.hyperedge_root_page, 25);
            assert_eq!(decoded.next_hyperedge_id, 300);
            assert_eq!(
                decoded.feature_flags,
                DatabaseHeader::FLAG_TEMPORAL_CORE
                    | DatabaseHeader::FLAG_TEMPORAL_EDGE
                    | DatabaseHeader::FLAG_SUBGRAPH
                    | DatabaseHeader::FLAG_HYPERGRAPH
            );
        }

        // HH-005: v4->v5 auto-migration (hyperedge fields default to 0)
        #[test]
        fn test_database_header_v4_to_v5_migration() {
            let mut page = [0u8; PAGE_SIZE];
            page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
            page[4..8].copy_from_slice(&4u32.to_le_bytes()); // version 4
            page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
            page[20..28].copy_from_slice(&1u64.to_le_bytes());
            page[28..36].copy_from_slice(&1u64.to_le_bytes());
            // v4 feature_flags at bytes 44-47
            let flags = DatabaseHeader::FLAG_TEMPORAL_CORE
                | DatabaseHeader::FLAG_TEMPORAL_EDGE
                | DatabaseHeader::FLAG_SUBGRAPH;
            page[44..48].copy_from_slice(&flags.to_le_bytes());
            // v4 subgraph fields
            page[48..56].copy_from_slice(&10u64.to_le_bytes()); // subgraph_root_page
            page[56..64].copy_from_slice(&5u64.to_le_bytes()); // next_subgraph_id
                                                               // bytes 64-79 are zeros (no hyperedge fields in v4)

            let decoded = DatabaseHeader::from_page(&page);
            assert_eq!(decoded.version, 4);
            assert_eq!(decoded.subgraph_root_page, 10);
            assert_eq!(decoded.next_subgraph_id, 5);
            assert_eq!(decoded.hyperedge_root_page, 0); // auto-migrated
            assert_eq!(decoded.next_hyperedge_id, 0); // auto-migrated
        }

        // HH-005: compiled_feature_flags includes FLAG_HYPERGRAPH
        #[test]
        fn test_compiled_feature_flags_includes_hypergraph() {
            let flags = DatabaseHeader::compiled_feature_flags();
            assert!(flags & DatabaseHeader::FLAG_HYPERGRAPH != 0);
        }
    }
}
