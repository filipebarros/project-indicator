use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct CachedMetadata {
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub cached_at: u64,
}

impl CachedMetadata {
    pub fn new(exists: bool, is_file: bool, is_dir: bool, size: u64, modified: u64) -> Self {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();

        Self {
            exists,
            is_file,
            is_dir,
            size,
            modified,
            cached_at,
        }
    }

    pub fn is_valid(&self, ttl_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_secs();
        (now - self.cached_at) < ttl_secs
    }
}

#[derive(Debug)]
pub struct FileSystemCache {
    metadata_cache: Arc<Mutex<HashMap<PathBuf, CachedMetadata>>>,
    ttl_secs: u64,
    max_entries: usize,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl FileSystemCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            metadata_cache: Arc::new(Mutex::new(HashMap::new())),
            ttl_secs,
            max_entries,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn get_metadata(&self, path: &Path) -> Option<CachedMetadata> {
        let path_buf = path.to_path_buf();
        if let Ok(mut cache) = self.metadata_cache.lock() {
            if let Some(cached) = cache.get(&path_buf) {
                if cached.is_valid(self.ttl_secs) {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(cached.clone());
                }
                cache.remove(&path_buf);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                CachedMetadata::new(
                    true,
                    metadata.is_file(),
                    metadata.is_dir(),
                    metadata.len(),
                    modified,
                )
            }
            Err(_) => CachedMetadata::new(false, false, false, 0, 0),
        };

        // Insert into cache
        if let Ok(mut cache) = self.metadata_cache.lock() {
            if cache.len() >= self.max_entries {
                self.evict_oldest(&mut cache);
            }
            cache.insert(path_buf, metadata.clone());
        }

        Some(metadata)
    }

    fn evict_oldest(&self, cache: &mut HashMap<PathBuf, CachedMetadata>) {
        let mut oldest_key: Option<PathBuf> = None;
        let mut oldest_time = u64::MAX;

        for (key, value) in cache.iter() {
            if value.cached_at < oldest_time {
                oldest_time = value.cached_at;
                oldest_key = Some(key.clone());
            }
        }

        if let Some(key) = oldest_key {
            cache.remove(&key);
        }
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

    pub fn get_content(&self, path: &Path) -> Result<String> {
        let path_buf = path.to_path_buf();
        if let Some(metadata) = self.get_metadata(path) {
            if !metadata.exists || !metadata.is_file {
                return Err(anyhow::anyhow!("File does not exist or is not a file"));
            }
        }
        let content = fs::read_to_string(&path_buf)?;
        Ok(content)
    }

    pub fn batch_read_files(&self, paths: &[PathBuf]) -> Vec<(PathBuf, Result<String>)> {
        paths
            .iter()
            .map(|path| {
                let result = fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e));
                (path.clone(), result)
            })
            .collect()
    }

    pub fn batch_get_metadata(&self, paths: &[PathBuf]) -> Vec<(PathBuf, Option<CachedMetadata>)> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| (path.clone(), self.get_metadata(path)))
            .collect()
    }

    pub fn batch_exists(&self, paths: &[PathBuf]) -> Vec<(PathBuf, bool)> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| (path.to_owned(), self.exists(path)))
            .collect()
    }

    pub fn filter_existing_files(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .filter(|path| self.is_file(path))
            .cloned()
            .collect()
    }

    pub fn batch_get_sizes(&self, paths: &[PathBuf]) -> Vec<(PathBuf, u64)> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .filter_map(|path| {
                self.get_metadata(path).and_then(|m| {
                    if m.exists && m.is_file {
                        Some((path.clone(), m.size))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.metadata_cache.lock() {
            cache.clear();
        }
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

        let metadata_entries = if let Ok(cache) = self.metadata_cache.lock() {
            cache.len()
        } else {
            0
        };

        CacheStats {
            metadata_entries,
            metadata_capacity: self.max_entries,
            hits,
            misses,
            hit_rate,
        }
    }
}

impl Default for FileSystemCache {
    fn default() -> Self {
        Self::new(300, 10000)
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub metadata_entries: usize,
    pub metadata_capacity: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_filesystem_cache_basic() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content")?;

        assert!(cache.exists(&test_file));
        assert!(cache.is_file(&test_file));
        assert!(!cache.is_dir(&test_file));

        Ok(())
    }

    #[test]
    fn test_filesystem_cache_nonexistent() {
        let cache = FileSystemCache::default();
        let nonexistent = PathBuf::from("/nonexistent/path/to/file.txt");

        assert!(!cache.exists(&nonexistent));
        assert!(!cache.is_file(&nonexistent));
        assert!(!cache.is_dir(&nonexistent));
    }

    #[test]
    fn test_filesystem_cache_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "content1")?;
        std::fs::write(&file2, "content2")?;

        let paths = vec![file1.clone(), file2.clone()];
        let results = cache.batch_exists(&paths);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, exists)| *exists));

        Ok(())
    }

    #[test]
    fn test_filesystem_cache_stats() {
        let cache = FileSystemCache::new(300, 100);
        let stats = cache.stats();
        assert_eq!(stats.metadata_capacity, 100);
    }

    #[test]
    fn test_content_read() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "content")?;

        let content = cache.get_content(&test_file)?;
        assert_eq!(content, "content");

        Ok(())
    }

    #[test]
    fn test_batch_read_files() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "content1")?;
        std::fs::write(&file2, "content2")?;

        let paths = vec![file1.clone(), file2.clone()];
        let results = cache.batch_read_files(&paths);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, result)| result.is_ok()));

        Ok(())
    }
}
