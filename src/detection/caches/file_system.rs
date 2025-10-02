use crate::detection::caches::parsed_file::ParsedFileCache;
use crate::performance::{CacheStats, FileSystemCache};
use std::path::Path;
use std::sync::Arc;

/// Manages file system and parsed file caches for the detection engine.
///
/// Caches file existence checks, file metadata, and parsed file contents
/// to avoid redundant I/O operations during a single detection run.
///
/// Note: This is separate from `DetectionCache` which caches complete
/// detection results across CLI invocations.
pub struct FileSystemCacheManager {
    file_existence_cache: Arc<FileSystemCache>,
    parsed_file_cache: ParsedFileCache,
}

impl FileSystemCacheManager {
    /// Creates a new FileSystemCacheManager with default settings.
    ///
    /// Default settings:
    /// - TTL: 300 seconds (5 minutes)
    /// - Max entries: 10,000
    pub fn new() -> Self {
        Self {
            file_existence_cache: Arc::new(FileSystemCache::new(300, 10000)),
            parsed_file_cache: ParsedFileCache::new(),
        }
    }

    /// Creates a new FileSystemCacheManager with custom settings.
    pub fn with_config(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            file_existence_cache: Arc::new(FileSystemCache::new(ttl_secs, max_entries)),
            parsed_file_cache: ParsedFileCache::new(),
        }
    }

    /// Checks if a file exists using the cache.
    pub fn file_exists(&self, path: &Path) -> bool {
        self.file_existence_cache.exists(path)
    }

    /// Gets a shared reference to the file existence cache.
    ///
    /// Returns an Arc clone (cheap reference count increment) allowing
    /// the cache to be shared across multiple components.
    pub fn file_existence_cache(&self) -> Arc<FileSystemCache> {
        Arc::clone(&self.file_existence_cache)
    }

    /// Gets a reference to the parsed file cache.
    pub fn parsed_file_cache(&self) -> &ParsedFileCache {
        &self.parsed_file_cache
    }

    /// Clears all caches.
    pub fn clear_all(&mut self) {
        self.file_existence_cache.clear();
        self.parsed_file_cache.clear();
    }

    /// Gets cache statistics for the file existence cache.
    pub fn stats(&self) -> CacheStats {
        self.file_existence_cache.stats()
    }

    /// Gets the number of cached file entries.
    pub fn cached_file_count(&self) -> usize {
        self.stats().metadata_entries
    }

    /// Gets the cache hit rate as a percentage.
    pub fn hit_rate(&self) -> f64 {
        self.stats().hit_rate
    }
}

impl Default for FileSystemCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cache_manager_creation() {
        let manager = FileSystemCacheManager::new();
        let stats = manager.stats();

        assert_eq!(stats.metadata_capacity, 10000);
        assert_eq!(stats.metadata_entries, 0);
    }

    #[test]
    fn test_cache_manager_with_config() {
        let manager = FileSystemCacheManager::with_config(600, 5000);
        let stats = manager.stats();

        assert_eq!(stats.metadata_capacity, 5000);
    }

    #[test]
    fn test_file_exists_caching() -> Result<(), Box<dyn std::error::Error>> {
        let manager = FileSystemCacheManager::new();
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "content")?;

        // First call - cache miss
        assert!(manager.file_exists(&test_file));

        // Second call - cache hit
        assert!(manager.file_exists(&test_file));

        let stats = manager.stats();
        assert!(stats.hits > 0);

        Ok(())
    }

    #[test]
    fn test_clear_all_caches() -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = FileSystemCacheManager::new();
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.txt");

        fs::write(&test_file, "content")?;

        // Populate cache
        manager.file_exists(&test_file);

        let stats_before = manager.stats();
        assert!(
            stats_before.metadata_entries > 0 || stats_before.hits > 0 || stats_before.misses > 0
        );

        // Clear caches
        manager.clear_all();

        let stats_after = manager.stats();
        assert_eq!(stats_after.metadata_entries, 0);

        Ok(())
    }

    #[test]
    fn test_cache_statistics() {
        let manager = FileSystemCacheManager::new();

        // Initially empty
        assert_eq!(manager.cached_file_count(), 0);
        assert_eq!(manager.hit_rate(), 0.0);
    }

    #[test]
    fn test_nonexistent_file() {
        let manager = FileSystemCacheManager::new();
        let nonexistent = Path::new("/nonexistent/path/to/file.txt");

        assert!(!manager.file_exists(nonexistent));
    }
}
