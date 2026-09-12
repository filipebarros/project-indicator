use crate::detection::caches::parsed_file::ParsedFileCache;
use crate::performance::FileSystemCache;
use std::sync::Arc;

/// Manages file system and parsed file caches for the detection engine.
///
/// Caches file existence checks, file metadata, and parsed file contents
/// to avoid redundant I/O operations during a single detection run.
pub struct FileSystemCacheManager {
    file_existence_cache: Arc<FileSystemCache>,
    parsed_file_cache: ParsedFileCache,
}

impl FileSystemCacheManager {
    pub fn new() -> Self {
        Self {
            file_existence_cache: Arc::new(FileSystemCache::new()),
            parsed_file_cache: ParsedFileCache::new(),
        }
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
}

impl Default for FileSystemCacheManager {
    fn default() -> Self {
        Self::new()
    }
}
