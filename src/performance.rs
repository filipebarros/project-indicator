use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Initial capacity for the cache map.
///
/// Typical projects touch 50-200 paths during a detection run; pre-allocating
/// avoids early rehashing without over-allocating.
const INITIAL_CACHE_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy)]
pub struct CachedMetadata {
    pub exists: bool,
    pub is_file: bool,
}

/// Per-run memoization of `fs::metadata` lookups.
///
/// The cache lives for a single detection run (one CLI invocation), so there
/// is no TTL and no eviction: the set of paths touched during one run is
/// naturally bounded by the scan, and the process exits when the run ends.
#[derive(Debug, Default)]
pub struct FileSystemCache {
    metadata_cache: Mutex<HashMap<PathBuf, CachedMetadata>>,
}

impl FileSystemCache {
    pub fn new() -> Self {
        Self {
            metadata_cache: Mutex::new(HashMap::with_capacity(INITIAL_CACHE_CAPACITY)),
        }
    }

    /// The lock is only ever held for a lookup or an insert, never across
    /// I/O; a poisoned lock still holds valid data, so recover it.
    fn map(&self) -> MutexGuard<'_, HashMap<PathBuf, CachedMetadata>> {
        self.metadata_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn get_metadata(&self, path: &Path) -> CachedMetadata {
        if let Some(cached) = self.map().get(path) {
            return *cached;
        }

        let metadata = match fs::metadata(path) {
            Ok(metadata) => CachedMetadata {
                exists: true,
                is_file: metadata.is_file(),
            },
            Err(_) => CachedMetadata {
                exists: false,
                is_file: false,
            },
        };

        self.map().insert(path.to_path_buf(), metadata);
        metadata
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.get_metadata(path).exists
    }

    pub fn is_file(&self, path: &Path) -> bool {
        self.get_metadata(path).is_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_is_memoized_for_the_run() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content")?;

        let first = cache.get_metadata(&file_path);
        assert!(first.exists && first.is_file);

        // Memoized: the run sees one consistent snapshot even if the file
        // disappears mid-run
        fs::remove_file(&file_path)?;
        assert!(cache.exists(&file_path));
        Ok(())
    }

    #[test]
    fn test_missing_path_is_cached_as_missing() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("later.txt");

        assert!(!cache.exists(&path));
        assert!(!cache.is_file(&path));

        // Negative results are memoized too
        fs::write(&path, "x")?;
        assert!(!cache.exists(&path));
        Ok(())
    }

    #[test]
    fn test_directory_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;

        assert!(cache.exists(temp_dir.path()));
        assert!(!cache.is_file(temp_dir.path()));
        Ok(())
    }

    #[test]
    fn test_concurrent_access() -> Result<(), Box<dyn std::error::Error>> {
        let cache = Arc::new(FileSystemCache::new());
        let temp_dir = TempDir::new()?;

        let mut paths = Vec::new();
        for i in 0..20 {
            let path = temp_dir.path().join(format!("file_{}.txt", i));
            fs::write(&path, "x")?;
            paths.push(path);
        }

        let mut handles = Vec::new();
        for _ in 0..4 {
            let cache = Arc::clone(&cache);
            let paths = paths.clone();
            handles.push(std::thread::spawn(move || {
                for path in &paths {
                    assert!(cache.exists(path));
                }
            }));
        }

        for handle in handles {
            handle
                .join()
                .map_err(|_| "concurrent access thread panicked")?;
        }
        Ok(())
    }
}
