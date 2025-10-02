use anyhow::Result;
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const INITIAL_CACHE_CAPACITY: usize = 128;

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

pub struct FileSystemCache {
    metadata_cache: Arc<DashMap<PathBuf, CachedMetadata>>,
    ttl_secs: u64,
    max_entries: usize,
    current_entries: Arc<AtomicUsize>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    eviction_lock: Arc<Mutex<()>>,
    // Lock contention metrics
    lock_contentions: Arc<AtomicU64>,
    evictions_performed: Arc<AtomicU64>,
}

// Manual Debug impl because Mutex doesn't implement Debug easily
impl std::fmt::Debug for FileSystemCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSystemCache")
            .field("metadata_cache", &self.metadata_cache)
            .field("ttl_secs", &self.ttl_secs)
            .field("max_entries", &self.max_entries)
            .field("current_entries", &self.current_entries)
            .field("hits", &self.hits)
            .field("misses", &self.misses)
            .field("lock_contentions", &self.lock_contentions)
            .field("evictions_performed", &self.evictions_performed)
            .finish()
    }
}

impl FileSystemCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            metadata_cache: Arc::new(DashMap::with_capacity(INITIAL_CACHE_CAPACITY)),
            ttl_secs,
            max_entries,
            current_entries: Arc::new(AtomicUsize::new(0)),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            eviction_lock: Arc::new(Mutex::new(())),
            lock_contentions: Arc::new(AtomicU64::new(0)),
            evictions_performed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn get_metadata(&self, path: &Path) -> Option<CachedMetadata> {
        // Check cache first - DashMap supports &Path lookups via Borrow trait
        // This avoids allocating a PathBuf for cache hits (the common case)
        if let Some(cached) = self.metadata_cache.get(path) {
            if cached.is_valid(self.ttl_secs) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(cached.clone());
            }
            // TTL expired - remove it
            drop(cached); // Release read lock before removing
            self.metadata_cache.remove(path);
            self.current_entries.fetch_sub(1, Ordering::Relaxed);
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        // Perform file system operation (expensive)
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

        // Insert into cache with eviction check
        // Use mutex to prevent concurrent insertions during eviction
        // Track lock contention by trying to acquire without blocking first
        let eviction_guard = match self.eviction_lock.try_lock() {
            Ok(guard) => Some(guard),
            Err(_) => {
                // Lock is held by another thread - track contention
                self.lock_contentions.fetch_add(1, Ordering::Relaxed);
                // Now block until we can acquire the lock
                match self.eviction_lock.lock() {
                    Ok(guard) => Some(guard),
                    Err(_) => {
                        // Lock is poisoned - return metadata without caching
                        log::warn!("Eviction lock poisoned, skipping cache insert");
                        return Some(metadata);
                    }
                }
            }
        };

        // Check size before inserting
        let current_size = self.current_entries.load(Ordering::Acquire);

        // If we're at the limit, evict first
        if current_size >= self.max_entries {
            self.evict_entries();
            self.evictions_performed.fetch_add(1, Ordering::Relaxed);
        }

        // Now insert - only allocate PathBuf here when we need to insert
        self.metadata_cache
            .insert(path.to_path_buf(), metadata.clone());
        self.current_entries.fetch_add(1, Ordering::Release);

        // Explicitly drop the guard to release the lock
        drop(eviction_guard);

        Some(metadata)
    }

    fn evict_entries(&self) {
        let target_size = (self.max_entries as f64 * 0.75) as usize;
        let current_size = self.current_entries.load(Ordering::Relaxed);

        if current_size <= target_size {
            return;
        }

        let to_remove = current_size.saturating_sub(target_size);

        // O(n log k) eviction: Use a max-heap to find the oldest entries
        // This avoids sorting all entries (O(n log n)) by maintaining only
        // the top k oldest entries, where k << n makes this effectively O(n)
        use std::collections::BinaryHeap;

        let mut oldest_entries: BinaryHeap<(u64, PathBuf)> = BinaryHeap::new();

        // Single pass through all entries - O(n log k)
        for entry in self.metadata_cache.iter() {
            let key = entry.key().clone();
            let cached_at = entry.value().cached_at;

            if oldest_entries.len() < to_remove {
                // Heap not full yet, just insert
                oldest_entries.push((cached_at, key));
            } else if let Some(&(newest_in_heap, _)) = oldest_entries.peek() {
                // If this entry is older (smaller timestamp) than the newest in heap, replace it
                if cached_at < newest_in_heap {
                    oldest_entries.pop();
                    oldest_entries.push((cached_at, key));
                }
            }
        }

        // Remove the collected oldest entries - O(k)
        let mut removed = 0;
        while let Some((_, key)) = oldest_entries.pop() {
            if self.metadata_cache.remove(&key).is_some() {
                removed += 1;
            }
        }

        self.current_entries.fetch_sub(removed, Ordering::Relaxed);
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
        // No need to allocate PathBuf - fs::read_to_string accepts &Path
        if let Some(metadata) = self.get_metadata(path) {
            if !metadata.exists || !metadata.is_file {
                return Err(anyhow::anyhow!("File does not exist or is not a file"));
            }
        }
        let content = fs::read_to_string(path)?;
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

    pub fn batch_exists(&self, paths: &[PathBuf]) -> Vec<(PathBuf, bool)> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| (path.to_owned(), self.exists(path)))
            .collect()
    }

    pub fn clear(&self) {
        self.metadata_cache.clear();
        self.current_entries.store(0, Ordering::Relaxed);
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
            metadata_entries: self.current_entries.load(Ordering::Relaxed),
            metadata_capacity: self.max_entries,
            hits,
            misses,
            hit_rate,
            lock_contentions: self.lock_contentions.load(Ordering::Relaxed),
            evictions_performed: self.evictions_performed.load(Ordering::Relaxed),
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
    pub lock_contentions: u64,
    pub evictions_performed: u64,
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

    #[test]
    fn test_cache_eviction() -> Result<(), Box<dyn std::error::Error>> {
        // Create a cache with small capacity to test eviction
        let cache = FileSystemCache::new(300, 10);
        let temp_dir = TempDir::new()?;

        // Create test files
        for i in 0..15 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;

            // Access the file to populate cache
            cache.exists(&file);

            // Small delay to ensure distinct timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let stats = cache.stats();

        // Cache should have evicted some entries to stay under limit
        assert!(
            stats.metadata_entries <= 10,
            "Cache has {} entries, expected <= 10",
            stats.metadata_entries
        );

        // Should have triggered eviction (more misses than capacity)
        assert!(
            stats.misses >= 10,
            "Expected at least 10 misses, got {}",
            stats.misses
        );

        Ok(())
    }

    #[test]
    fn test_cache_eviction_oldest_first() -> Result<(), Box<dyn std::error::Error>> {
        // Test that eviction removes oldest entries
        let cache = FileSystemCache::new(1, 5); // 1 second TTL, 5 entry limit
        let temp_dir = TempDir::new()?;

        // Create and cache 10 files
        let mut files = Vec::new();
        for i in 0..10 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;
            files.push(file.clone());

            cache.exists(&file);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let stats = cache.stats();

        // Should have evicted to 75% of max (3-4 entries)
        assert!(
            stats.metadata_entries <= 5,
            "Cache size {} exceeds limit 5",
            stats.metadata_entries
        );

        Ok(())
    }

    #[test]
    fn test_concurrent_cache_access() -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(FileSystemCache::new(300, 1000));
        let temp_dir = Arc::new(TempDir::new()?);

        // Create test files
        for i in 0..50 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;
        }

        // Spawn multiple threads accessing cache concurrently
        let mut handles = vec![];
        for thread_id in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let temp_dir_clone = Arc::clone(&temp_dir);

            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let file_idx = (thread_id * 10 + i) % 50;
                    let file = temp_dir_clone.path().join(format!("file{}.txt", file_idx));

                    // Mix of operations
                    cache_clone.exists(&file);
                    cache_clone.is_file(&file);
                    cache_clone.is_dir(&file);
                }
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle
                .join()
                .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))?;
        }

        let stats = cache.stats();

        // Verify cache is still functional and has reasonable stats
        assert!(stats.metadata_entries > 0, "Cache should have entries");
        assert!(
            stats.hits + stats.misses > 0,
            "Cache should have been accessed"
        );

        Ok(())
    }

    #[test]
    fn test_concurrent_cache_eviction_under_pressure() -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use std::thread;

        // Small cache to force frequent eviction
        let cache = Arc::new(FileSystemCache::new(1, 50));
        let temp_dir = Arc::new(TempDir::new()?);

        // Create many test files
        for i in 0..200 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;
        }

        let mut handles = vec![];

        // Create heavy write pressure from multiple threads
        for thread_id in 0..8 {
            let cache_clone = Arc::clone(&cache);
            let temp_dir_clone = Arc::clone(&temp_dir);

            let handle = thread::spawn(move || {
                for i in 0..500 {
                    let file_idx = (thread_id * 100 + i) % 200;
                    let file = temp_dir_clone.path().join(format!("file{}.txt", file_idx));
                    cache_clone.exists(&file);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle
                .join()
                .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))?;
        }

        let stats = cache.stats();

        // Cache should maintain its size limit despite heavy concurrent access
        assert!(
            stats.metadata_entries <= 50,
            "Cache size {} exceeds limit 50 under concurrent pressure",
            stats.metadata_entries
        );

        // Should have processed many requests
        assert!(
            stats.hits + stats.misses >= 3000,
            "Expected at least 3000 total accesses"
        );

        Ok(())
    }

    #[test]
    fn test_concurrent_mixed_operations() -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(FileSystemCache::new(300, 500));
        let temp_dir = Arc::new(TempDir::new()?);

        // Create test files
        for i in 0..100 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;
        }

        let mut handles = vec![];

        // Mix of readers and writers
        for thread_id in 0..12 {
            let cache_clone = Arc::clone(&cache);
            let temp_dir_clone = Arc::clone(&temp_dir);

            let handle = thread::spawn(move || {
                for i in 0..200 {
                    let file_idx = (thread_id * 20 + i) % 100;
                    let file = temp_dir_clone.path().join(format!("file{}.txt", file_idx));

                    match thread_id % 3 {
                        0 => {
                            // Read operations
                            cache_clone.exists(&file);
                        }
                        1 => {
                            // Metadata operations
                            cache_clone.is_file(&file);
                            cache_clone.is_dir(&file);
                        }
                        _ => {
                            // Batch operations
                            let paths = vec![file.clone()];
                            cache_clone.batch_exists(&paths);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle
                .join()
                .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))?;
        }

        let stats = cache.stats();

        // Verify no data races or panics occurred
        assert!(stats.metadata_entries <= 500, "Cache size within limits");
        assert!(
            stats.hits + stats.misses > 0,
            "Operations completed successfully"
        );

        Ok(())
    }

    #[test]
    fn test_lock_contention_metrics() -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use std::thread;

        // Small cache to force contention
        let cache = Arc::new(FileSystemCache::new(1, 20));
        let temp_dir = Arc::new(TempDir::new()?);

        // Create test files
        for i in 0..100 {
            let file = temp_dir.path().join(format!("file{}.txt", i));
            std::fs::write(&file, format!("content{}", i))?;
        }

        let mut handles = vec![];

        // Many threads competing for the lock
        for thread_id in 0..16 {
            let cache_clone = Arc::clone(&cache);
            let temp_dir_clone = Arc::clone(&temp_dir);

            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let file_idx = (thread_id * 10 + i) % 100;
                    let file = temp_dir_clone.path().join(format!("file{}.txt", file_idx));
                    cache_clone.exists(&file);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle
                .join()
                .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))?;
        }

        let stats = cache.stats();

        // With heavy concurrent access and frequent eviction, we should see:
        // 1. Lock contentions (threads waiting for lock)
        // 2. Evictions performed
        println!("Lock contentions: {}", stats.lock_contentions);
        println!("Evictions performed: {}", stats.evictions_performed);
        println!("Cache entries: {}", stats.metadata_entries);
        println!("Hits: {}, Misses: {}", stats.hits, stats.misses);

        // Verify metrics were captured
        assert!(
            stats.lock_contentions > 0,
            "Expected lock contention with 16 threads"
        );
        assert!(
            stats.evictions_performed > 0,
            "Expected evictions with small cache"
        );
        assert!(stats.metadata_entries <= 20, "Cache size within limit");

        Ok(())
    }
}
