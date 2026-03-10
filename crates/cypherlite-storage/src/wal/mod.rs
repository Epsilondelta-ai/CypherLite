/// WAL checkpoint: copies committed frames back to the main database file.
pub mod checkpoint;
/// WAL reader for scanning committed frames.
pub mod reader;
/// Crash recovery by replaying WAL frames.
pub mod recovery;
/// WAL writer for appending frames and committing transactions.
pub mod writer;

use crate::page::PAGE_SIZE;

/// WAL magic bytes: "WAL1" (0x57414C31).
pub const WAL_MAGIC: u32 = 0x5741_4C31;

/// WAL format version.
pub const WAL_FORMAT_VERSION: u32 = 1;

/// Size of the WAL header in bytes.
pub const WAL_HEADER_SIZE: usize = 32;

/// Size of a single WAL frame (header fields + page data).
/// 8 (frame_number) + 4 (page_number) + 4 (db_size) + 8 (salt) + 8 (checksum) + 4096 (page_data) = 4128
pub const WAL_FRAME_SIZE: usize = 32 + PAGE_SIZE;

/// WAL file header.
#[derive(Debug, Clone)]
pub struct WalHeader {
    /// Magic number identifying the WAL file.
    pub magic: u32,
    /// WAL format version number.
    pub format_version: u32,
    /// Number of committed frames in the WAL.
    pub frame_count: u64,
    /// Random salt for checksum computation.
    pub salt: u64,
    /// Checksum of the header fields.
    pub header_checksum: u64,
}

impl WalHeader {
    /// Creates a new WAL header with the given salt.
    pub fn new(salt: u64) -> Self {
        let mut hdr = Self {
            magic: WAL_MAGIC,
            format_version: WAL_FORMAT_VERSION,
            frame_count: 0,
            salt,
            header_checksum: 0,
        };
        hdr.header_checksum = hdr.compute_checksum();
        hdr
    }

    /// Serialize the WAL header to bytes.
    pub fn to_bytes(&self) -> [u8; WAL_HEADER_SIZE] {
        let mut buf = [0u8; WAL_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..8].copy_from_slice(&self.format_version.to_le_bytes());
        buf[8..16].copy_from_slice(&self.frame_count.to_le_bytes());
        buf[16..24].copy_from_slice(&self.salt.to_le_bytes());
        buf[24..32].copy_from_slice(&self.header_checksum.to_le_bytes());
        buf
    }

    /// Deserialize the WAL header from bytes.
    pub fn from_bytes(buf: &[u8; WAL_HEADER_SIZE]) -> Self {
        Self {
            magic: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            format_version: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            frame_count: u64::from_le_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
            salt: u64::from_le_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]),
            header_checksum: u64::from_le_bytes([
                buf[24], buf[25], buf[26], buf[27], buf[28], buf[29], buf[30], buf[31],
            ]),
        }
    }

    fn compute_checksum(&self) -> u64 {
        let mut sum: u64 = 0;
        sum = sum.wrapping_add(self.magic as u64);
        sum = sum.wrapping_add(self.format_version as u64);
        sum = sum.wrapping_add(self.frame_count);
        sum = sum.wrapping_add(self.salt);
        sum
    }
}

/// A single WAL frame: metadata + page data.
#[derive(Debug, Clone)]
pub struct WalFrame {
    /// Sequential frame number within the WAL.
    pub frame_number: u64,
    /// Database page number this frame overwrites.
    pub page_number: u32,
    /// Database size (in pages) at the time of this frame.
    pub db_size: u32,
    /// Salt value copied from the WAL header.
    pub salt: u64,
    /// Integrity checksum over all frame fields and page data.
    pub checksum: u64,
    /// Full page data (4096 bytes).
    pub page_data: [u8; PAGE_SIZE],
}

impl WalFrame {
    /// Create a new WAL frame with computed checksum.
    pub fn new(
        frame_number: u64,
        page_number: u32,
        db_size: u32,
        salt: u64,
        page_data: [u8; PAGE_SIZE],
    ) -> Self {
        let mut frame = Self {
            frame_number,
            page_number,
            db_size,
            salt,
            checksum: 0,
            page_data,
        };
        frame.checksum = frame.compute_checksum();
        frame
    }

    /// Compute checksum over all frame fields except checksum itself.
    // @MX:NOTE: [AUTO] Checksum is wrapping-add of frame_number, page_number, db_size,
    //   salt, and all page_data bytes as u64 chunks. Changing this algorithm
    //   breaks compatibility with existing WAL files (format version must be bumped).
    // @MX:SPEC: SPEC-DB-001 REQ-WAL-007
    pub fn compute_checksum(&self) -> u64 {
        let mut sum: u64 = 0;
        sum = sum.wrapping_add(self.frame_number);
        sum = sum.wrapping_add(self.page_number as u64);
        sum = sum.wrapping_add(self.db_size as u64);
        sum = sum.wrapping_add(self.salt);
        for chunk in self.page_data.chunks(8) {
            let mut bytes = [0u8; 8];
            bytes[..chunk.len()].copy_from_slice(chunk);
            sum = sum.wrapping_add(u64::from_le_bytes(bytes));
        }
        sum
    }

    /// Verify frame checksum integrity.
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.compute_checksum()
    }

    /// Serialize frame to bytes (32 bytes header + 4096 bytes page data = 4128 total).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(WAL_FRAME_SIZE);
        buf.extend_from_slice(&self.frame_number.to_le_bytes());
        buf.extend_from_slice(&self.page_number.to_le_bytes());
        buf.extend_from_slice(&self.db_size.to_le_bytes());
        buf.extend_from_slice(&self.salt.to_le_bytes());
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        buf.extend_from_slice(&self.page_data);
        buf
    }

    /// Deserialize frame from bytes.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < WAL_FRAME_SIZE {
            return None;
        }
        let frame_number = u64::from_le_bytes(buf[0..8].try_into().ok()?);
        let page_number = u32::from_le_bytes(buf[8..12].try_into().ok()?);
        let db_size = u32::from_le_bytes(buf[12..16].try_into().ok()?);
        let salt = u64::from_le_bytes(buf[16..24].try_into().ok()?);
        let checksum = u64::from_le_bytes(buf[24..32].try_into().ok()?);
        let mut page_data = [0u8; PAGE_SIZE];
        page_data.copy_from_slice(&buf[32..32 + PAGE_SIZE]);
        Some(Self {
            frame_number,
            page_number,
            db_size,
            salt,
            checksum,
            page_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-WAL-006: WalHeader has magic, checksum, salt, frame_count
    #[test]
    fn test_wal_header_new() {
        let hdr = WalHeader::new(12345);
        assert_eq!(hdr.magic, WAL_MAGIC);
        assert_eq!(hdr.format_version, WAL_FORMAT_VERSION);
        assert_eq!(hdr.frame_count, 0);
        assert_eq!(hdr.salt, 12345);
        assert_ne!(hdr.header_checksum, 0);
    }

    #[test]
    fn test_wal_header_roundtrip() {
        let hdr = WalHeader::new(99999);
        let bytes = hdr.to_bytes();
        let decoded = WalHeader::from_bytes(&bytes);
        assert_eq!(decoded.magic, hdr.magic);
        assert_eq!(decoded.salt, hdr.salt);
        assert_eq!(decoded.header_checksum, hdr.header_checksum);
    }

    // REQ-WAL-002: Frame includes frame_number, page_number, db_size, salt, checksum, page_data
    #[test]
    fn test_wal_frame_creation() {
        let data = [0xAB; PAGE_SIZE];
        let frame = WalFrame::new(1, 5, 100, 12345, data);
        assert_eq!(frame.frame_number, 1);
        assert_eq!(frame.page_number, 5);
        assert_eq!(frame.db_size, 100);
        assert_ne!(frame.checksum, 0);
    }

    // REQ-WAL-007: Checksum verification
    #[test]
    fn test_wal_frame_checksum_verification() {
        let data = [0xCD; PAGE_SIZE];
        let frame = WalFrame::new(1, 5, 100, 12345, data);
        assert!(frame.verify_checksum());
    }

    // REQ-WAL-007: Corrupted frame fails checksum
    #[test]
    fn test_wal_frame_corrupted_checksum() {
        let data = [0xCD; PAGE_SIZE];
        let mut frame = WalFrame::new(1, 5, 100, 12345, data);
        frame.page_data[0] = 0xFF; // corrupt
        assert!(!frame.verify_checksum());
    }

    #[test]
    fn test_wal_frame_roundtrip() {
        let data = [0xEF; PAGE_SIZE];
        let frame = WalFrame::new(42, 10, 200, 54321, data);
        let bytes = frame.to_bytes();
        let decoded = WalFrame::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded.frame_number, 42);
        assert_eq!(decoded.page_number, 10);
        assert_eq!(decoded.checksum, frame.checksum);
        assert!(decoded.verify_checksum());
    }

    #[test]
    fn test_wal_frame_size() {
        assert_eq!(WAL_FRAME_SIZE, 32 + 4096);
    }

    #[test]
    fn test_wal_header_size() {
        assert_eq!(WAL_HEADER_SIZE, 32);
    }

    #[test]
    fn test_wal_magic_bytes() {
        let bytes = WAL_MAGIC.to_be_bytes();
        assert_eq!(&bytes, b"WAL1");
    }

    #[test]
    fn test_wal_frame_from_bytes_too_short() {
        let buf = [0u8; 10];
        assert!(WalFrame::from_bytes(&buf).is_none());
    }
}
