// PageManager: file format, page allocation, Free Space Map

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use cypherlite_core::{CypherLiteError, DatabaseConfig, PageId, Result};

use super::{DatabaseHeader, FORMAT_VERSION, FSM_PAGE_ID, MAGIC, PAGE_SIZE};

/// Manages the on-disk database file: page allocation, FSM, and raw I/O.
pub struct PageManager {
    file: File,
    header: DatabaseHeader,
    path: PathBuf,
}

impl PageManager {
    /// Create a new database file with initial header and FSM pages.
    pub fn create_database(config: &DatabaseConfig) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&config.path)?;

        let header = DatabaseHeader::new();
        let header_page = header.to_page();
        file.write_all(&header_page)?;

        // Write empty FSM page (page 1) - all bits 0 = all pages free
        // But mark pages 0 and 1 as used (header + FSM)
        let mut fsm_page = [0u8; PAGE_SIZE];
        // Mark page 0 (header) and page 1 (FSM) as used
        fsm_page[0] = 0b0000_0011; // bits 0 and 1 set
        file.write_all(&fsm_page)?;

        file.sync_all()?;

        Ok(Self {
            file,
            header,
            path: config.path.clone(),
        })
    }

    /// Open an existing database file, validating magic and version.
    pub fn open_database(config: &DatabaseConfig) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)?;

        let mut header_buf = [0u8; PAGE_SIZE];
        file.read_exact(&mut header_buf)?;

        let header = DatabaseHeader::from_page(&header_buf);

        // REQ-PAGE-007: Validate magic number
        if header.magic != MAGIC {
            return Err(CypherLiteError::InvalidMagicNumber);
        }

        // REQ-PAGE-008: Validate version (accept older versions for auto-migration)
        if header.version == 0 || header.version > FORMAT_VERSION {
            return Err(CypherLiteError::UnsupportedVersion {
                found: header.version,
                supported: FORMAT_VERSION,
            });
        }

        // Auto-migrate old headers to current FORMAT_VERSION
        let mut header = header;
        if header.version < FORMAT_VERSION {
            // W-004: v1->v2 migration
            if header.version < 2 {
                header.version_store_root_page = 0;
            }
            // AA-T3: v2->v3 migration: feature_flags defaults to temporal-core
            // (from_page already sets this for version < 3)
            // GG-003: v3->v4 migration: subgraph fields default to 0
            // (from_page already sets subgraph_root_page=0, next_subgraph_id=0 for version < 4)
            header.version = FORMAT_VERSION;
        }

        // AA-T4: Feature compatibility check
        let compiled = super::DatabaseHeader::compiled_feature_flags();
        let db_flags = header.feature_flags;
        // If the database requires features we don't have compiled in, reject
        if (db_flags & !compiled) != 0 {
            return Err(CypherLiteError::FeatureIncompatible {
                db_flags,
                compiled_flags: compiled,
            });
        }

        Ok(Self {
            file,
            header,
            path: config.path.clone(),
        })
    }

    /// Allocate a new page by finding the first free bit in the FSM.
    /// REQ-PAGE-004: Find first free bit in FSM.
    pub fn allocate_page(&mut self) -> Result<PageId> {
        let mut fsm_buf = [0u8; PAGE_SIZE];
        self.read_page_raw(FSM_PAGE_ID, &mut fsm_buf)?;

        // Scan bytes for first byte with a free bit
        for (byte_idx, byte_val) in fsm_buf.iter().enumerate() {
            if *byte_val != 0xFF {
                // Found a byte with at least one free bit
                for bit in 0..8u32 {
                    if (*byte_val & (1 << bit)) == 0 {
                        let page_id = (byte_idx as u32) * 8 + bit;
                        // Mark as used
                        fsm_buf[byte_idx] |= 1 << bit;
                        self.write_page_raw(FSM_PAGE_ID, &fsm_buf)?;

                        // Update page count if needed
                        if page_id >= self.header.page_count {
                            self.header.page_count = page_id + 1;
                            self.flush_header()?;
                        }

                        // Ensure the file is large enough
                        let required_size = (page_id as u64 + 1) * PAGE_SIZE as u64;
                        let current_size = self.file.seek(SeekFrom::End(0))?;
                        if current_size < required_size {
                            // Extend file with zero page
                            self.file
                                .seek(SeekFrom::Start(page_id as u64 * PAGE_SIZE as u64))?;
                            self.file.write_all(&[0u8; PAGE_SIZE])?;
                        }

                        return Ok(PageId(page_id));
                    }
                }
            }
        }

        Err(CypherLiteError::OutOfSpace)
    }

    /// Deallocate a page by clearing its bit in the FSM.
    /// REQ-PAGE-005: Clear bit in FSM when page freed.
    pub fn deallocate_page(&mut self, page_id: PageId) -> Result<()> {
        let mut fsm_buf = [0u8; PAGE_SIZE];
        self.read_page_raw(FSM_PAGE_ID, &mut fsm_buf)?;

        let byte_idx = page_id.0 as usize / 8;
        let bit_idx = page_id.0 % 8;
        fsm_buf[byte_idx] &= !(1 << bit_idx);

        self.write_page_raw(FSM_PAGE_ID, &fsm_buf)?;
        Ok(())
    }

    /// Read a raw 4KB page from the database file.
    pub fn read_page(&mut self, page_id: PageId) -> Result<[u8; PAGE_SIZE]> {
        let mut buf = [0u8; PAGE_SIZE];
        self.read_page_raw(page_id.0, &mut buf)?;
        Ok(buf)
    }

    /// Write a raw 4KB page to the database file.
    pub fn write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<()> {
        self.write_page_raw(page_id.0, data)
    }

    /// Flush the database header (page 0) to disk.
    pub fn flush_header(&mut self) -> Result<()> {
        let page = self.header.to_page();
        self.write_page_raw(0, &page)?;
        Ok(())
    }

    /// Returns a reference to the current database header.
    pub fn header(&self) -> &DatabaseHeader {
        &self.header
    }

    /// Returns a mutable reference to the current database header.
    pub fn header_mut(&mut self) -> &mut DatabaseHeader {
        &mut self.header
    }

    /// Returns the database file path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Sync all data to disk.
    pub fn sync(&mut self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }

    // Internal: read raw page by numeric ID
    fn read_page_raw(&mut self, page_id: u32, buf: &mut [u8; PAGE_SIZE]) -> Result<()> {
        let offset = page_id as u64 * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buf)?;
        Ok(())
    }

    // Internal: write raw page by numeric ID
    fn write_page_raw(&mut self, page_id: u32, data: &[u8; PAGE_SIZE]) -> Result<()> {
        let offset = page_id as u64 * PAGE_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::{DatabaseHeader, FIRST_DATA_PAGE};
    use tempfile::tempdir;

    fn test_config(dir: &std::path::Path) -> DatabaseConfig {
        DatabaseConfig {
            path: dir.join("test.cyl"),
            ..Default::default()
        }
    }

    // REQ-PAGE-002: Create database with header page at page 0
    #[test]
    fn test_create_database_writes_header() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let pm = PageManager::create_database(&config).expect("create");
        assert_eq!(pm.header().magic, MAGIC);
        assert_eq!(pm.header().version, FORMAT_VERSION);
        assert_eq!(pm.header().page_count, FIRST_DATA_PAGE);
    }

    // REQ-PAGE-002: File contains header and FSM pages
    #[test]
    fn test_create_database_file_size() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let _pm = PageManager::create_database(&config).expect("create");
        let file_size = std::fs::metadata(&config.path).expect("metadata").len();
        // Should contain at least header page + FSM page = 2 * 4096
        assert_eq!(file_size, 2 * PAGE_SIZE as u64);
    }

    // REQ-PAGE-007: Opening file with wrong magic -> InvalidMagicNumber
    #[test]
    fn test_open_invalid_magic_number() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());

        // Write garbage header
        let mut file = File::create(&config.path).expect("create");
        let mut bad_page = [0u8; PAGE_SIZE];
        bad_page[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        file.write_all(&bad_page).expect("write");
        file.write_all(&[0u8; PAGE_SIZE]).expect("write fsm");
        drop(file);

        let result = PageManager::open_database(&config);
        assert!(matches!(result, Err(CypherLiteError::InvalidMagicNumber)));
    }

    // REQ-PAGE-008: Opening file with unsupported version -> UnsupportedVersion
    #[test]
    fn test_open_unsupported_version() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());

        let mut hdr = DatabaseHeader::new();
        hdr.version = 99;
        let mut file = File::create(&config.path).expect("create");
        file.write_all(&hdr.to_page()).expect("write");
        file.write_all(&[0u8; PAGE_SIZE]).expect("write fsm");
        drop(file);

        let result = PageManager::open_database(&config);
        assert!(matches!(
            result,
            Err(CypherLiteError::UnsupportedVersion {
                found: 99,
                supported
            }) if supported == FORMAT_VERSION
        ));
    }

    // REQ-PAGE-007 + REQ-PAGE-002: Open valid database succeeds
    #[test]
    fn test_open_valid_database() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        drop(PageManager::create_database(&config).expect("create"));
        let pm = PageManager::open_database(&config).expect("open");
        assert_eq!(pm.header().magic, MAGIC);
    }

    // REQ-PAGE-004: Allocate page finds first free bit in FSM
    #[test]
    fn test_allocate_page_returns_first_free() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let mut pm = PageManager::create_database(&config).expect("create");

        // Pages 0 and 1 are used. First free should be page 2.
        let page = pm.allocate_page().expect("alloc");
        assert_eq!(page, PageId(2));

        let page2 = pm.allocate_page().expect("alloc");
        assert_eq!(page2, PageId(3));
    }

    // REQ-PAGE-005: Deallocate page clears FSM bit
    #[test]
    fn test_deallocate_and_reallocate() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let mut pm = PageManager::create_database(&config).expect("create");

        let p1 = pm.allocate_page().expect("alloc");
        let p2 = pm.allocate_page().expect("alloc");
        assert_eq!(p1, PageId(2));
        assert_eq!(p2, PageId(3));

        // Free p1
        pm.deallocate_page(p1).expect("dealloc");

        // Next allocation should reuse p1's slot (page 2)
        let p3 = pm.allocate_page().expect("alloc");
        assert_eq!(p3, PageId(2));
    }

    // REQ-PAGE-001: Read/write in 4KB fixed pages
    #[test]
    fn test_read_write_page_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let mut pm = PageManager::create_database(&config).expect("create");

        let page_id = pm.allocate_page().expect("alloc");

        let mut data = [0u8; PAGE_SIZE];
        data[0] = 0xAB;
        data[4095] = 0xCD;
        pm.write_page(page_id, &data).expect("write");

        let read_back = pm.read_page(page_id).expect("read");
        assert_eq!(read_back[0], 0xAB);
        assert_eq!(read_back[4095], 0xCD);
    }

    // Multiple allocations
    #[test]
    fn test_allocate_multiple_pages() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let mut pm = PageManager::create_database(&config).expect("create");

        let mut pages = vec![];
        for i in 0..10 {
            let p = pm.allocate_page().expect("alloc");
            assert_eq!(p, PageId(FIRST_DATA_PAGE + i));
            pages.push(p);
        }
        assert_eq!(pages.len(), 10);
    }

    // REQ-PAGE-003: FSM bitmap marks header and FSM as used
    #[test]
    fn test_fsm_initial_state() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let mut pm = PageManager::create_database(&config).expect("create");

        let fsm = pm.read_page(PageId(FSM_PAGE_ID)).expect("read fsm");
        // Bits 0 and 1 should be set (header + FSM)
        assert_eq!(fsm[0] & 0b11, 0b11);
        // Bit 2 should be clear (first data page is free)
        assert_eq!(fsm[0] & 0b100, 0);
    }

    #[test]
    fn test_flush_header_persists() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        {
            let mut pm = PageManager::create_database(&config).expect("create");
            pm.header_mut().next_node_id = 42;
            pm.flush_header().expect("flush");
        }
        let pm = PageManager::open_database(&config).expect("open");
        assert_eq!(pm.header().next_node_id, 42);
    }

    // AA-T3: Open v2 database auto-migrates to v3
    #[test]
    fn test_open_v2_database_migrates_to_v3() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());

        // Write a v2 header manually (raw bytes, not via DatabaseHeader)
        let mut file = File::create(&config.path).expect("create");
        // Manually write v2 format (no feature_flags at bytes 44-47)
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        page[4..8].copy_from_slice(&2u32.to_le_bytes());
        page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
        page[20..28].copy_from_slice(&1u64.to_le_bytes());
        page[28..36].copy_from_slice(&1u64.to_le_bytes());
        file.write_all(&page).expect("write header");
        file.write_all(&[0u8; PAGE_SIZE]).expect("write fsm");
        drop(file);

        let pm = PageManager::open_database(&config).expect("open v2");
        assert_eq!(pm.header().version, FORMAT_VERSION);
        // Auto-migrated feature_flags should have temporal-core
        assert!(pm.header().feature_flags & DatabaseHeader::FLAG_TEMPORAL_CORE != 0);
    }

    // AA-T4: Database with feature flags not compiled in is rejected
    #[test]
    fn test_open_database_with_unsupported_features() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());

        // Write a v3 header with a feature flag we don't have compiled
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        page[4..8].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
        page[20..28].copy_from_slice(&1u64.to_le_bytes());
        page[28..36].copy_from_slice(&1u64.to_le_bytes());
        // Set a high bit that no current compilation supports
        let bogus_flags = 0x8000_0000u32;
        page[44..48].copy_from_slice(&bogus_flags.to_le_bytes());

        let mut file = File::create(&config.path).expect("create");
        file.write_all(&page).expect("write header");
        file.write_all(&[0u8; PAGE_SIZE]).expect("write fsm");
        drop(file);

        let result = PageManager::open_database(&config);
        assert!(matches!(result, Err(CypherLiteError::FeatureIncompatible { .. })));
    }

    // AA-T3: New database gets current FORMAT_VERSION with feature flags
    #[test]
    fn test_new_database_has_current_format_version_header() {
        let dir = tempdir().expect("tempdir");
        let config = test_config(dir.path());
        let pm = PageManager::create_database(&config).expect("create");
        assert_eq!(pm.header().version, FORMAT_VERSION);
        assert!(pm.header().feature_flags & DatabaseHeader::FLAG_TEMPORAL_CORE != 0);
    }
}
