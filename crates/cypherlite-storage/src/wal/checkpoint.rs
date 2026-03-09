// Checkpoint: WAL -> main file copy, frame counter reset

use cypherlite_core::{CypherLiteError, PageId, Result};

use super::reader::WalReader;
use super::writer::WalWriter;
use crate::page::page_manager::PageManager;

/// Checkpoint: copies committed WAL frames to the main database file.
///
/// REQ-WAL-004: Copy committed WAL frames to main file, reset frame counter.
/// REQ-WAL-005: Continue using WAL index for reads during checkpoint.
// @MX:ANCHOR: [AUTO] Critical data-integrity path: WAL -> main file flush.
// @MX:REASON: Called by StorageEngine::checkpoint() (lib.rs) and must preserve
//   write-ahead guarantee; incorrect ordering causes data loss or corruption.
// @MX:SPEC: SPEC-DB-001 REQ-WAL-004, REQ-WAL-005, REQ-WAL-007
pub struct Checkpoint;

impl Checkpoint {
    /// Run a checkpoint: copy all committed WAL frames to the main file.
    pub fn run(
        page_manager: &mut PageManager,
        wal_writer: &mut WalWriter,
        wal_reader: &mut WalReader,
    ) -> Result<u64> {
        let frame_count = wal_writer.frame_count();
        if frame_count == 0 {
            return Ok(0);
        }

        // Read each committed frame and write to main file
        for i in 1..=frame_count {
            let frame = wal_writer.read_frame(i)?;

            // REQ-WAL-007: Verify checksum before applying
            if !frame.verify_checksum() {
                return Err(CypherLiteError::ChecksumMismatch {
                    expected: frame.compute_checksum(),
                    found: frame.checksum,
                });
            }

            let page_id = PageId(frame.page_number);
            page_manager.write_page(page_id, &frame.page_data)?;
        }

        // Sync main file
        page_manager.sync()?;

        // Reset WAL
        wal_writer.reset()?;

        // Clear WAL reader index
        wal_reader.clear();

        Ok(frame_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::page_manager::PageManager;
    use crate::page::PAGE_SIZE;
    use crate::wal::reader::WalReader;
    use crate::wal::writer::WalWriter;
    use cypherlite_core::{DatabaseConfig, SyncMode};
    use tempfile::tempdir;

    fn setup(dir: &std::path::Path) -> (PageManager, WalWriter, WalReader) {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            ..Default::default()
        };
        let pm = PageManager::create_database(&config).expect("create");
        let wal_path = config.wal_path();
        let ww = WalWriter::create(&wal_path, 12345, SyncMode::Normal).expect("wal");
        let wr = WalReader::new();
        (pm, ww, wr)
    }

    // REQ-WAL-004: Checkpoint copies frames to main file
    #[test]
    fn test_checkpoint_copies_frames() {
        let dir = tempdir().expect("tempdir");
        let (mut pm, mut ww, mut wr) = setup(dir.path());

        // Allocate a page and write via WAL
        let page_id = pm.allocate_page().expect("alloc");
        let data = [0xAB; PAGE_SIZE];
        ww.write_frame(page_id, 10, &data).expect("write");
        ww.commit().expect("commit");

        // Before checkpoint, main file has zeros
        let before = pm.read_page(page_id).expect("read");
        assert_eq!(before[0], 0);

        // Run checkpoint
        let count = Checkpoint::run(&mut pm, &mut ww, &mut wr).expect("checkpoint");
        assert_eq!(count, 1);

        // After checkpoint, main file has the data
        let after = pm.read_page(page_id).expect("read");
        assert_eq!(after[0], 0xAB);
    }

    // REQ-WAL-004: Reset frame counter after checkpoint
    #[test]
    fn test_checkpoint_resets_wal() {
        let dir = tempdir().expect("tempdir");
        let (mut pm, mut ww, mut wr) = setup(dir.path());

        let page_id = pm.allocate_page().expect("alloc");
        ww.write_frame(page_id, 10, &[0x11; PAGE_SIZE]).expect("w");
        ww.commit().expect("commit");

        Checkpoint::run(&mut pm, &mut ww, &mut wr).expect("checkpoint");
        assert_eq!(ww.frame_count(), 0);
    }

    #[test]
    fn test_checkpoint_clears_reader_index() {
        let dir = tempdir().expect("tempdir");
        let (mut pm, mut ww, mut wr) = setup(dir.path());

        let page_id = pm.allocate_page().expect("alloc");
        ww.write_frame(page_id, 10, &[0x11; PAGE_SIZE]).expect("w");
        ww.commit().expect("commit");

        // Index the frame in reader
        let frame = ww.read_frame(1).expect("read");
        wr.index_frame(frame);
        assert_eq!(wr.page_count(), 1);

        Checkpoint::run(&mut pm, &mut ww, &mut wr).expect("checkpoint");
        assert_eq!(wr.page_count(), 0);
    }

    #[test]
    fn test_checkpoint_empty_wal_is_noop() {
        let dir = tempdir().expect("tempdir");
        let (mut pm, mut ww, mut wr) = setup(dir.path());
        let count = Checkpoint::run(&mut pm, &mut ww, &mut wr).expect("checkpoint");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_checkpoint_multiple_frames() {
        let dir = tempdir().expect("tempdir");
        let (mut pm, mut ww, mut wr) = setup(dir.path());

        let p1 = pm.allocate_page().expect("alloc");
        let p2 = pm.allocate_page().expect("alloc");

        ww.write_frame(p1, 10, &[0x11; PAGE_SIZE]).expect("w");
        ww.write_frame(p2, 10, &[0x22; PAGE_SIZE]).expect("w");
        ww.commit().expect("commit");

        let count = Checkpoint::run(&mut pm, &mut ww, &mut wr).expect("checkpoint");
        assert_eq!(count, 2);

        assert_eq!(pm.read_page(p1).expect("r")[0], 0x11);
        assert_eq!(pm.read_page(p2).expect("r")[0], 0x22);
    }
}
