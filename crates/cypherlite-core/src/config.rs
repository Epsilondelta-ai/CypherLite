// DatabaseConfig

use std::path::PathBuf;

/// Sync mode for WAL operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncMode {
    /// Full fsync after every write (maximum durability).
    Full,
    /// Normal sync (OS decides when to flush).
    Normal,
}

/// Configuration for opening or creating a CypherLite database.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Path to the main database file (.cyl).
    pub path: PathBuf,
    /// Page size in bytes (always 4096).
    pub page_size: u32,
    /// Number of pages in the buffer pool cache.
    pub cache_capacity: usize,
    /// WAL synchronization mode.
    pub wal_sync_mode: SyncMode,
    /// Enable automatic _created_at / _updated_at timestamp tracking (default: true).
    pub temporal_tracking_enabled: bool,
    /// Enable version storage for pre-update snapshots (default: true when temporal tracking enabled).
    pub version_storage_enabled: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("database.cyl"),
            page_size: 4096,
            cache_capacity: 256,
            wal_sync_mode: SyncMode::Full,
            temporal_tracking_enabled: true,
            version_storage_enabled: true,
        }
    }
}

impl DatabaseConfig {
    /// Returns the path to the WAL file associated with this database.
    pub fn wal_path(&self) -> PathBuf {
        self.path.with_extension("cyl-wal")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-PAGE-001: Always read/write in 4KB fixed pages
    #[test]
    fn test_default_page_size_is_4096() {
        let config = DatabaseConfig::default();
        assert_eq!(config.page_size, 4096);
    }

    // REQ-BUF-001: Default 256 pages (1MB)
    #[test]
    fn test_default_cache_capacity_is_256() {
        let config = DatabaseConfig::default();
        assert_eq!(config.cache_capacity, 256);
    }

    // REQ-TX-007: Default to Full sync for durability
    #[test]
    fn test_default_sync_mode_is_full() {
        let config = DatabaseConfig::default();
        assert_eq!(config.wal_sync_mode, SyncMode::Full);
    }

    #[test]
    fn test_default_path() {
        let config = DatabaseConfig::default();
        assert_eq!(config.path, PathBuf::from("database.cyl"));
    }

    // REQ-WAL-001: WAL file path derivation
    #[test]
    fn test_wal_path_derivation() {
        let config = DatabaseConfig {
            path: PathBuf::from("/tmp/test.cyl"),
            ..Default::default()
        };
        assert_eq!(config.wal_path(), PathBuf::from("/tmp/test.cyl-wal"));
    }

    // REQ-BUF-007: User-configurable capacity
    #[test]
    fn test_custom_cache_capacity() {
        let config = DatabaseConfig {
            cache_capacity: 1024,
            ..Default::default()
        };
        assert_eq!(config.cache_capacity, 1024);
    }

    // V-004: Temporal tracking enabled by default
    #[test]
    fn test_default_temporal_tracking_enabled() {
        let config = DatabaseConfig::default();
        assert!(config.temporal_tracking_enabled);
    }

    // V-004: Temporal tracking can be disabled
    #[test]
    fn test_temporal_tracking_can_be_disabled() {
        let config = DatabaseConfig {
            temporal_tracking_enabled: false,
            ..Default::default()
        };
        assert!(!config.temporal_tracking_enabled);
    }

    // W-005: Version storage enabled by default
    #[test]
    fn test_default_version_storage_enabled() {
        let config = DatabaseConfig::default();
        assert!(config.version_storage_enabled);
    }

    // W-005: Version storage can be disabled
    #[test]
    fn test_version_storage_can_be_disabled() {
        let config = DatabaseConfig {
            version_storage_enabled: false,
            ..Default::default()
        };
        assert!(!config.version_storage_enabled);
    }

    #[test]
    fn test_config_clone() {
        let config = DatabaseConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.page_size, config.page_size);
        assert_eq!(cloned.cache_capacity, config.cache_capacity);
    }
}
