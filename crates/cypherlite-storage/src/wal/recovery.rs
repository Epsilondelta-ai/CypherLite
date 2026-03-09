// Recovery: crash recovery via WAL replay

use std::path::PathBuf;

use cypherlite_core::{PageId, Result, SyncMode};

use super::reader::WalReader;
use super::writer::WalWriter;
use crate::page::page_manager::PageManager;

/// Crash recovery: scan WAL, replay committed frames, discard uncommitted.
///
/// REQ-TX-008: When database opened after abnormal termination,
/// scan WAL, replay committed frames, discard uncommitted.
// @MX:NOTE: [AUTO] Recovery resets the WAL after replay (wal_writer.reset()).
//   Frames with bad checksums are silently skipped (not errors); partial writes
//   at the tail truncate the replay sequence. StorageEngine::open() always runs
//   recovery before constructing the WalWriter.
// @MX:SPEC: SPEC-DB-001 REQ-TX-008, REQ-WAL-007
pub struct Recovery;

impl Recovery {
    /// Recover the database by replaying committed WAL frames.
    ///
    /// Returns the number of frames replayed and a populated WalReader.
    pub fn recover(page_manager: &mut PageManager, wal_path: &PathBuf) -> Result<(u64, WalReader)> {
        // Try to open existing WAL
        let mut wal_writer = match WalWriter::open(wal_path, SyncMode::Normal) {
            Ok(w) => w,
            Err(_) => {
                // No WAL file, nothing to recover
                return Ok((0, WalReader::new()));
            }
        };

        let frame_count = wal_writer.frame_count();
        if frame_count == 0 {
            return Ok((0, WalReader::new()));
        }

        let mut wal_reader = WalReader::new();
        let mut replayed = 0u64;

        // Read and validate each committed frame
        for i in 1..=frame_count {
            match wal_writer.read_frame(i) {
                Ok(frame) => {
                    if frame.verify_checksum() {
                        // Write to main file
                        let page_id = PageId(frame.page_number);
                        page_manager.write_page(page_id, &frame.page_data)?;
                        wal_reader.index_frame(frame);
                        replayed += 1;
                    }
                    // REQ-WAL-007: Skip frames with checksum mismatch
                }
                Err(_) => {
                    // Partial write / truncated frame - stop here
                    break;
                }
            }
        }

        // Sync main file after replay
        if replayed > 0 {
            page_manager.sync()?;
        }

        // Reset WAL after recovery
        wal_writer.reset()?;
        wal_reader.clear();

        Ok((replayed, wal_reader))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::PAGE_SIZE;
    use cypherlite_core::DatabaseConfig;
    use tempfile::tempdir;

    // REQ-TX-008: Recovery replays committed frames
    #[test]
    fn test_recovery_replays_committed_frames() {
        let dir = tempdir().expect("tempdir");
        let config = DatabaseConfig {
            path: dir.path().join("test.cyl"),
            ..Default::default()
        };
        let mut pm = PageManager::create_database(&config).expect("create");
        let wal_path = config.wal_path();

        // Write frames to WAL and commit
        {
            let mut ww = WalWriter::create(&wal_path, 42, SyncMode::Normal).expect("wal");
            let page_id = pm.allocate_page().expect("alloc");
            ww.write_frame(page_id, 10, &[0xAB; PAGE_SIZE]).expect("w");
            ww.commit().expect("commit");
        }

        // Simulate crash: main file does NOT have the data
        let page_id = PageId(2);
        let before = pm.read_page(page_id).expect("read");
        assert_eq!(before[0], 0); // not yet applied

        // Recovery
        let (replayed, _reader) = Recovery::recover(&mut pm, &wal_path).expect("recover");
        assert_eq!(replayed, 1);

        // After recovery, main file has the data
        let after = pm.read_page(page_id).expect("read");
        assert_eq!(after[0], 0xAB);
    }

    #[test]
    fn test_recovery_no_wal_file() {
        let dir = tempdir().expect("tempdir");
        let config = DatabaseConfig {
            path: dir.path().join("test.cyl"),
            ..Default::default()
        };
        let mut pm = PageManager::create_database(&config).expect("create");
        let wal_path = dir.path().join("nonexistent.cyl-wal");

        let (replayed, _) = Recovery::recover(&mut pm, &wal_path).expect("recover");
        assert_eq!(replayed, 0);
    }

    #[test]
    fn test_recovery_empty_wal() {
        let dir = tempdir().expect("tempdir");
        let config = DatabaseConfig {
            path: dir.path().join("test.cyl"),
            ..Default::default()
        };
        let mut pm = PageManager::create_database(&config).expect("create");
        let wal_path = config.wal_path();

        // Create empty WAL
        let _ww = WalWriter::create(&wal_path, 42, SyncMode::Normal).expect("wal");

        let (replayed, _) = Recovery::recover(&mut pm, &wal_path).expect("recover");
        assert_eq!(replayed, 0);
    }

    // REQ-WAL-007: Skip frames with bad checksum
    #[test]
    fn test_recovery_skips_corrupted_frames() {
        let dir = tempdir().expect("tempdir");
        let config = DatabaseConfig {
            path: dir.path().join("test.cyl"),
            ..Default::default()
        };
        let mut pm = PageManager::create_database(&config).expect("create");
        let wal_path = config.wal_path();

        // Write a good frame and a corrupted frame
        {
            let mut ww = WalWriter::create(&wal_path, 42, SyncMode::Normal).expect("wal");
            let p1 = pm.allocate_page().expect("alloc");
            let p2 = pm.allocate_page().expect("alloc");
            ww.write_frame(p1, 10, &[0x11; PAGE_SIZE]).expect("w1");
            ww.write_frame(p2, 10, &[0x22; PAGE_SIZE]).expect("w2");
            ww.commit().expect("commit");

            // Corrupt the second frame by overwriting part of it
            use std::fs::OpenOptions;
            use std::io::{Seek, SeekFrom, Write};
            let mut f = OpenOptions::new()
                .write(true)
                .open(&wal_path)
                .expect("open");
            // Second frame starts at header(32) + frame1(4128)
            let offset = 32 + 4128 + 32; // skip to page_data of frame 2
            f.seek(SeekFrom::Start(offset as u64)).expect("seek");
            f.write_all(&[0xFF; 10]).expect("corrupt");
        }

        let (replayed, _) = Recovery::recover(&mut pm, &wal_path).expect("recover");
        // First frame good, second frame has bad checksum
        assert_eq!(replayed, 1);

        // First page should be recovered
        assert_eq!(pm.read_page(PageId(2)).expect("r")[0], 0x11);
    }
}
