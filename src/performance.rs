use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Initial capacity for the DashMap cache.
///
/// Typical projects touch 50-200 paths during a detection run; pre-allocating
/// avoids early rehashing without over-allocating.
const INITIAL_CACHE_CAPACITY: usize = 128;

#[derive(Debug, Clone)]
pub struct CachedMetadata {
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
}

impl CachedMetadata {
    pub fn new(exists: bool, is_file: bool, is_dir: bool) -> Self {
        Self {
            exists,
            is_file,
            is_dir,
        }
    }
}

/// Per-run memoization of `fs::metadata` lookups.
///
/// The cache lives for a single detection run (one CLI invocation), so there
/// is no TTL and no eviction: the set of paths touched during one run is
/// naturally bounded by the scan, and the process exits when the run ends.
#[derive(Debug, Default)]
pub struct FileSystemCache {
    metadata_cache: DashMap<PathBuf, CachedMetadata>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl FileSystemCache {
    pub fn new() -> Self {
        Self {
            metadata_cache: DashMap::with_capacity(INITIAL_CACHE_CAPACITY),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn get_metadata(&self, path: &Path) -> Option<CachedMetadata> {
        // DashMap supports &Path lookups via the Borrow trait, avoiding a
        // PathBuf allocation for cache hits (the common case)
        if let Some(cached) = self.metadata_cache.get(path) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Some(cached.clone());
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        let metadata = match fs::metadata(path) {
            Ok(metadata) => CachedMetadata::new(true, metadata.is_file(), metadata.is_dir()),
            Err(_) => CachedMetadata::new(false, false, false),
        };

        self.metadata_cache
            .insert(path.to_path_buf(), metadata.clone());

        Some(metadata)
    }

    pub fn exists(&self, path: &Path) -> bool {
        self.get_metadata(path).map(|m| m.exists).unwrap_or(false)
    }

    pub fn is_file(&self, path: &Path) -> bool {
        self.get_metadata(path).map(|m| m.is_file).unwrap_or(false)
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        self.get_metadata(path).map(|m| m.is_dir).unwrap_or(false)
    }

    pub fn clear(&self) {
        self.metadata_cache.clear();
    }

    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        CacheStats {
            metadata_entries: self.metadata_cache.len(),
            hits,
            misses,
            hit_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub metadata_entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_caching_hit_and_miss() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content")?;

        let first = cache
            .get_metadata(&file_path)
            .ok_or("expected metadata on first lookup")?;
        assert!(first.exists);
        assert!(first.is_file);
        assert!(!first.is_dir);

        let second = cache
            .get_metadata(&file_path)
            .ok_or("expected metadata on second lookup")?;
        assert!(second.exists);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.metadata_entries, 1);
        Ok(())
    }

    #[test]
    fn test_nonexistent_path_is_cached_as_missing() {
        let cache = FileSystemCache::new();
        let path = Path::new("/nonexistent/path/to/file.txt");

        assert!(!cache.exists(path));
        assert!(!cache.is_file(path));
        assert!(!cache.is_dir(path));

        let stats = cache.stats();
        // Only the first lookup should miss; subsequent lookups hit the
        // cached negative entry
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 2);
    }

    #[test]
    fn test_directory_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;

        assert!(cache.exists(temp_dir.path()));
        assert!(cache.is_dir(temp_dir.path()));
        assert!(!cache.is_file(temp_dir.path()));
        Ok(())
    }

    #[test]
    fn test_clear_empties_cache() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content")?;

        cache.exists(&file_path);
        assert_eq!(cache.stats().metadata_entries, 1);

        cache.clear();
        assert_eq!(cache.stats().metadata_entries, 0);
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

        assert_eq!(cache.stats().metadata_entries, 20);
        Ok(())
    }
}
