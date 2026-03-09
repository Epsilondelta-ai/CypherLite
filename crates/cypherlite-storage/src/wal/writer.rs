// WalWriter: WAL frame write, fsync, commit marker

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use cypherlite_core::{PageId, Result, SyncMode};

use super::{WalFrame, WalHeader, WAL_FRAME_SIZE, WAL_HEADER_SIZE};
use crate::page::PAGE_SIZE;

/// Writes WAL frames to the WAL file.
///
/// REQ-WAL-001: Always write to WAL before main file.
/// REQ-WAL-002: Include all required fields and fsync.
pub struct WalWriter {
    file: File,
    header: WalHeader,
    path: PathBuf,
    /// Frames written in the current (uncommitted) transaction.
    uncommitted_frames: Vec<WalFrame>,
    sync_mode: SyncMode,
}

impl WalWriter {
    /// Create a new WAL file.
    pub fn create(path: &PathBuf, salt: u64, sync_mode: SyncMode) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        let header = WalHeader::new(salt);
        file.write_all(&header.to_bytes())?;
        if matches!(sync_mode, SyncMode::Full) {
            file.sync_all()?;
        }

        Ok(Self {
            file,
            header,
            path: path.clone(),
            uncommitted_frames: Vec::new(),
            sync_mode,
        })
    }

    /// Open an existing WAL file.
    pub fn open(path: &PathBuf, sync_mode: SyncMode) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;

        let mut hdr_buf = [0u8; WAL_HEADER_SIZE];
        use std::io::Read;
        file.read_exact(&mut hdr_buf)?;
        let header = WalHeader::from_bytes(&hdr_buf);

        Ok(Self {
            file,
            header,
            path: path.clone(),
            uncommitted_frames: Vec::new(),
            sync_mode,
        })
    }

    /// Write a frame to the WAL (uncommitted).
    /// REQ-WAL-002: Write frame with all required fields.
    pub fn write_frame(
        &mut self,
        page_id: PageId,
        db_size: u32,
        page_data: &[u8; PAGE_SIZE],
    ) -> Result<u64> {
        let frame_number = self.header.frame_count + self.uncommitted_frames.len() as u64 + 1;
        let frame = WalFrame::new(
            frame_number,
            page_id.0,
            db_size,
            self.header.salt,
            *page_data,
        );
        self.uncommitted_frames.push(frame);
        Ok(frame_number)
    }

    /// Commit all uncommitted frames: write to file + fsync.
    /// REQ-TX-002: fsync all uncommitted WAL frames on commit.
    /// REQ-TX-007: fsync committed transaction WAL frames for durability.
    pub fn commit(&mut self) -> Result<u64> {
        if self.uncommitted_frames.is_empty() {
            return Ok(self.header.frame_count);
        }

        // Seek to end of committed frames
        let write_offset = WAL_HEADER_SIZE as u64 + self.header.frame_count * WAL_FRAME_SIZE as u64;
        self.file.seek(SeekFrom::Start(write_offset))?;

        // Write all uncommitted frames
        for frame in &self.uncommitted_frames {
            self.file.write_all(&frame.to_bytes())?;
        }

        // Update header
        self.header.frame_count += self.uncommitted_frames.len() as u64;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&self.header.to_bytes())?;

        // REQ-WAL-002: fsync
        if matches!(self.sync_mode, SyncMode::Full) {
            self.file.sync_all()?;
        }

        self.uncommitted_frames.clear();

        Ok(self.header.frame_count)
    }

    /// Discard all uncommitted frames.
    /// REQ-TX-003: Discard uncommitted WAL frames on rollback.
    pub fn discard(&mut self) {
        self.uncommitted_frames.clear();
    }

    /// Returns the current committed frame count.
    pub fn frame_count(&self) -> u64 {
        self.header.frame_count
    }

    /// Returns uncommitted frame references for the WAL reader index.
    pub fn uncommitted_frames(&self) -> &[WalFrame] {
        &self.uncommitted_frames
    }

    /// Returns the WAL header.
    pub fn header(&self) -> &WalHeader {
        &self.header
    }

    /// Returns the WAL file path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Reset the WAL after checkpoint (truncate and rewrite header).
    /// REQ-WAL-004: Reset frame counter after checkpoint.
    pub fn reset(&mut self) -> Result<()> {
        self.header.frame_count = 0;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&self.header.to_bytes())?;
        self.file.set_len(WAL_HEADER_SIZE as u64)?;
        if matches!(self.sync_mode, SyncMode::Full) {
            self.file.sync_all()?;
        }
        Ok(())
    }

    /// Read a frame at a given frame number (1-indexed).
    pub fn read_frame(&mut self, frame_number: u64) -> Result<WalFrame> {
        let offset = WAL_HEADER_SIZE as u64 + (frame_number - 1) * WAL_FRAME_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; WAL_FRAME_SIZE];
        use std::io::Read;
        self.file.read_exact(&mut buf)?;
        WalFrame::from_bytes(&buf).ok_or_else(|| cypherlite_core::CypherLiteError::CorruptedPage {
            page_id: 0,
            reason: "Invalid WAL frame".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_writer(dir: &std::path::Path) -> WalWriter {
        let path = dir.join("test.cyl-wal");
        WalWriter::create(&path, 12345, SyncMode::Normal).expect("create")
    }

    // REQ-WAL-001: WAL file creation
    #[test]
    fn test_create_wal_file() {
        let dir = tempdir().expect("tempdir");
        let writer = test_writer(dir.path());
        assert_eq!(writer.frame_count(), 0);
        assert_eq!(writer.header().magic, super::super::WAL_MAGIC);
    }

    // REQ-WAL-002: Write frame with all required fields
    #[test]
    fn test_write_frame() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());
        let data = [0xAB; PAGE_SIZE];
        let frame_num = writer.write_frame(PageId(5), 100, &data).expect("write");
        assert_eq!(frame_num, 1);
        assert_eq!(writer.uncommitted_frames().len(), 1);
    }

    // REQ-TX-002: Commit flushes frames
    #[test]
    fn test_commit_frames() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());

        writer
            .write_frame(PageId(5), 100, &[0xAB; PAGE_SIZE])
            .expect("w1");
        writer
            .write_frame(PageId(6), 100, &[0xCD; PAGE_SIZE])
            .expect("w2");

        let count = writer.commit().expect("commit");
        assert_eq!(count, 2);
        assert_eq!(writer.frame_count(), 2);
        assert!(writer.uncommitted_frames().is_empty());
    }

    // REQ-TX-003: Discard uncommitted frames
    #[test]
    fn test_discard_uncommitted() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());
        writer
            .write_frame(PageId(5), 100, &[0xAB; PAGE_SIZE])
            .expect("w");
        assert_eq!(writer.uncommitted_frames().len(), 1);
        writer.discard();
        assert!(writer.uncommitted_frames().is_empty());
        assert_eq!(writer.frame_count(), 0);
    }

    // REQ-WAL-004: Reset after checkpoint
    #[test]
    fn test_reset_wal() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());
        writer
            .write_frame(PageId(5), 100, &[0xAB; PAGE_SIZE])
            .expect("w");
        writer.commit().expect("commit");
        assert_eq!(writer.frame_count(), 1);

        writer.reset().expect("reset");
        assert_eq!(writer.frame_count(), 0);
    }

    #[test]
    fn test_read_committed_frame() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());
        let data = [0xEF; PAGE_SIZE];
        writer.write_frame(PageId(10), 200, &data).expect("w");
        writer.commit().expect("commit");

        let frame = writer.read_frame(1).expect("read");
        assert_eq!(frame.page_number, 10);
        assert_eq!(frame.page_data[0], 0xEF);
        assert!(frame.verify_checksum());
    }

    #[test]
    fn test_open_existing_wal() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.cyl-wal");
        {
            let mut writer = WalWriter::create(&path, 42, SyncMode::Normal).expect("create");
            writer
                .write_frame(PageId(1), 10, &[0x11; PAGE_SIZE])
                .expect("w");
            writer.commit().expect("commit");
        }
        let writer = WalWriter::open(&path, SyncMode::Normal).expect("open");
        assert_eq!(writer.frame_count(), 1);
    }

    #[test]
    fn test_commit_empty_is_noop() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());
        let count = writer.commit().expect("commit");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_multiple_commits() {
        let dir = tempdir().expect("tempdir");
        let mut writer = test_writer(dir.path());

        writer
            .write_frame(PageId(1), 10, &[0x11; PAGE_SIZE])
            .expect("w");
        writer.commit().expect("c1");
        assert_eq!(writer.frame_count(), 1);

        writer
            .write_frame(PageId(2), 10, &[0x22; PAGE_SIZE])
            .expect("w");
        writer
            .write_frame(PageId(3), 10, &[0x33; PAGE_SIZE])
            .expect("w");
        writer.commit().expect("c2");
        assert_eq!(writer.frame_count(), 3);

        // Verify all frames readable
        let f1 = writer.read_frame(1).expect("r1");
        assert_eq!(f1.page_number, 1);
        let f3 = writer.read_frame(3).expect("r3");
        assert_eq!(f3.page_number, 3);
    }
}
