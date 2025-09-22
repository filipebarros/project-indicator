//! Caching system for detection results
//!
//! This module provides memory-based caching of detection results with file modification
//! time tracking to invalidate stale cache entries.

use crate::types::{CacheConfig, DetectionResult};
use crate::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Cache key consisting of path and relevant file modification times
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    /// The project path being detected
    pub path: PathBuf,
    /// Hash of critical file modification times
    pub file_hash: u64,
}

/// Cached detection result with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The cached detection result
    pub result: DetectionResult,
    /// When this entry was created
    pub created_at: SystemTime,
    /// File modification times that were used to create the hash
    pub file_times: HashMap<PathBuf, SystemTime>,
}

/// High-performance in-memory cache for detection results
pub struct DetectionCache {
    /// The actual cache storage
    cache: DashMap<CacheKey, CacheEntry>,
    /// Cache configuration
    config: CacheConfig,
    /// Statistics
    stats: CacheStats,
}

/// Cache performance statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub entries: usize,
}

impl DetectionCache {
    /// Create a new cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: DashMap::new(),
            config,
            stats: CacheStats::default(),
        }
    }

    /// Get a cached result if it exists and is still valid
    pub fn get(&self, path: &Path) -> Result<Option<DetectionResult>> {
        // Calculate current file hash
        let current_key = self.calculate_cache_key(path)?;

        if let Some(entry) = self.cache.get(&current_key) {
            // Check if entry is still valid (TTL check)
            if self.is_entry_valid(&entry, &current_key)? {
                // Note: In a real implementation, we'd use atomic counters for stats

                return Ok(Some(entry.result.clone()));
            } else {
                // Entry is stale, remove it
                drop(entry); // Release the read lock
                self.cache.remove(&current_key);
            }
        }

        Ok(None)
    }

    /// Store a detection result in the cache
    pub fn put(&self, path: &Path, result: DetectionResult) -> Result<()> {
        let cache_key = self.calculate_cache_key(path)?;
        let file_times = self.get_relevant_file_times(path)?;

        let entry = CacheEntry {
            result,
            created_at: SystemTime::now(),
            file_times,
        };

        self.cache.insert(cache_key, entry);

        // Enforce size limits
        if self.cache.len() > self.config.max_entries {
            self.evict_oldest_entries();
        }

        Ok(())
    }

    /// Calculate a cache key for the given path
    fn calculate_cache_key(&self, path: &Path) -> Result<CacheKey> {
        let file_times = self.get_relevant_file_times(path)?;
        let file_hash = self.hash_file_times(&file_times);

        Ok(CacheKey {
            path: path.to_path_buf(),
            file_hash,
        })
    }

    /// Get modification times for files relevant to detection
    fn get_relevant_file_times(&self, path: &Path) -> Result<HashMap<PathBuf, SystemTime>> {
        let mut file_times = HashMap::new();

        // Key files that affect detection results
        let key_files = [
            "package.json",
            "Cargo.toml",
            "go.mod",
            "pyproject.toml",
            "requirements.txt",
            "tsconfig.json",
            "setup.py",
            "composer.json",
            "pom.xml",
            "build.gradle",
        ];

        for file_name in &key_files {
            let file_path = path.join(file_name);
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    file_times.insert(file_path, modified);
                }
            }
        }

        // Also check for common config directories that might affect detection
        let config_dirs = [
            ".vscode",
            ".idea",
            "node_modules/.bin", // Check if dependencies changed
        ];

        for dir_name in &config_dirs {
            let dir_path = path.join(dir_name);
            if let Ok(metadata) = fs::metadata(&dir_path) {
                if let Ok(modified) = metadata.modified() {
                    file_times.insert(dir_path, modified);
                }
            }
        }

        Ok(file_times)
    }

    /// Create a hash from file modification times
    fn hash_file_times(&self, file_times: &HashMap<PathBuf, SystemTime>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Sort paths for consistent hashing
        let mut sorted_files: Vec<_> = file_times.iter().collect();
        sorted_files.sort_by_key(|(path, _)| *path);

        for (path, time) in sorted_files {
            path.hash(&mut hasher);
            // Convert SystemTime to a comparable format
            if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
                duration.as_secs().hash(&mut hasher);
                duration.subsec_nanos().hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Check if a cache entry is still valid
    fn is_entry_valid(&self, entry: &CacheEntry, current_key: &CacheKey) -> Result<bool> {
        // Check TTL
        if let Ok(age) = entry.created_at.elapsed() {
            if age > Duration::from_secs(self.config.ttl_seconds) {
                return Ok(false);
            }
        }

        // Check if any tracked files have been modified
        let current_file_times = self.get_relevant_file_times(&current_key.path)?;

        // If the number of files changed, cache is invalid
        if current_file_times.len() != entry.file_times.len() {
            return Ok(false);
        }

        // Check each file's modification time
        for (path, current_time) in &current_file_times {
            if let Some(cached_time) = entry.file_times.get(path) {
                if current_time != cached_time {
                    return Ok(false);
                }
            } else {
                // New file appeared
                return Ok(false);
            }
        }

        // Check for deleted files
        for path in entry.file_times.keys() {
            if !current_file_times.contains_key(path) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Remove oldest cache entries when cache is full
    fn evict_oldest_entries(&self) {
        let target_size = (self.config.max_entries as f32 * 0.8) as usize;
        let mut entries_to_remove = Vec::new();

        // Collect entries with their creation times
        for entry in self.cache.iter() {
            entries_to_remove.push((entry.key().clone(), entry.value().created_at));
        }

        // Sort by creation time (oldest first)
        entries_to_remove.sort_by_key(|(_, created_at)| *created_at);

        // Remove oldest entries
        let to_remove = self.cache.len().saturating_sub(target_size);
        for (key, _) in entries_to_remove.into_iter().take(to_remove) {
            self.cache.remove(&key);
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.stats.hits,
            misses: self.stats.misses,
            invalidations: self.stats.invalidations,
            entries: self.cache.len(),
        }
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Remove entries for a specific path (useful when files are known to have changed)
    pub fn invalidate_path(&self, path: &Path) -> usize {
        let mut removed = 0;
        let path_buf = path.to_path_buf();

        // Find all cache entries for this path
        let keys_to_remove: Vec<_> = self
            .cache
            .iter()
            .filter(|entry| entry.key().path == path_buf)
            .map(|entry| entry.key().clone())
            .collect();

        // Remove them
        for key in keys_to_remove {
            if self.cache.remove(&key).is_some() {
                removed += 1;
            }
        }

        removed
    }
}

/// Cache-aware detection trait
pub trait CachedDetection {
    /// Detect with caching support
    fn detect_cached(&self, path: &Path, cache: &DetectionCache) -> Result<DetectionResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProjectIndicator;
    use std::fs;
    use std::thread;
    use tempfile::TempDir;

    fn create_test_cache() -> DetectionCache {
        let config = CacheConfig {
            enabled: true,
            max_entries: 100,
            ttl_seconds: 300, // 5 minutes
        };
        DetectionCache::new(config)
    }

    fn create_test_result() -> DetectionResult {
        let language = ProjectIndicator {
            name: "TypeScript".to_string(),
            files: vec!["tsconfig.json".to_string()],
            color: "#3178C6".to_string(),
            icon: "󰛦".to_string(),
            priority: 1,
            frameworks: vec![],
        };

        DetectionResult::new(Some(language), vec![], 0.9)
    }

    #[test]
    fn test_cache_basic_operations() {
        let cache = create_test_cache();
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Initially empty
        assert!(cache.get(temp_dir.path()).unwrap().is_none());

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();

        // Should retrieve the same result
        let cached = cache.get(temp_dir.path()).unwrap().unwrap();
        assert_eq!(cached.language.as_ref().unwrap().name, "TypeScript");
        assert_eq!(cached.confidence, 0.9);
    }

    #[test]
    fn test_cache_file_modification_invalidation() {
        let cache = create_test_cache();
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Create initial file
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();

        // Should get cached result
        assert!(cache.get(temp_dir.path()).unwrap().is_some());

        // Wait a bit and modify file
        thread::sleep(Duration::from_millis(10));
        fs::write(temp_dir.path().join("package.json"), "{\"name\": \"test\"}").unwrap();

        // Cache should be invalidated
        assert!(cache.get(temp_dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_cache_new_file_invalidation() {
        let cache = create_test_cache();
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();

        // Should get cached result
        assert!(cache.get(temp_dir.path()).unwrap().is_some());

        // Add new relevant file
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        // Cache should be invalidated due to new file
        assert!(cache.get(temp_dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_cache_ttl_expiration() {
        let config = CacheConfig {
            enabled: true,
            max_entries: 100,
            ttl_seconds: 1, // 1 second TTL
        };
        let cache = DetectionCache::new(config);
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();

        // Should get cached result immediately
        assert!(cache.get(temp_dir.path()).unwrap().is_some());

        // Wait for TTL to expire
        thread::sleep(Duration::from_secs(2));

        // Cache should be expired
        assert!(cache.get(temp_dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_cache_size_limit() {
        let config = CacheConfig {
            enabled: true,
            max_entries: 2, // Small limit
            ttl_seconds: 300,
        };
        let cache = DetectionCache::new(config);
        let result = create_test_result();

        // Create multiple temp directories
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();
        let temp3 = TempDir::new().unwrap();

        // Fill cache beyond limit
        cache.put(temp1.path(), result.clone()).unwrap();
        cache.put(temp2.path(), result.clone()).unwrap();
        assert_eq!(cache.len(), 2);

        cache.put(temp3.path(), result.clone()).unwrap();

        // Cache should evict oldest entries
        assert!(cache.len() <= 2);
    }

    #[test]
    fn test_cache_invalidate_path() {
        let cache = create_test_cache();
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();
        assert!(cache.get(temp_dir.path()).unwrap().is_some());

        // Invalidate specific path
        let removed = cache.invalidate_path(temp_dir.path());
        assert_eq!(removed, 1);

        // Should no longer be cached
        assert!(cache.get(temp_dir.path()).unwrap().is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = create_test_cache();
        let temp_dir = TempDir::new().unwrap();
        let result = create_test_result();

        // Store result
        cache.put(temp_dir.path(), result.clone()).unwrap();
        assert!(!cache.is_empty());

        // Clear cache
        cache.clear();
        assert!(cache.is_empty());
        assert!(cache.get(temp_dir.path()).unwrap().is_none());
    }
}
