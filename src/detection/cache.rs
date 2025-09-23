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
#[cfg(test)]
use std::sync::Arc;
use std::sync::Mutex;
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
    stats: Mutex<CacheStats>,
    /// Additional dynamically supplied relevant paths (filenames or relative paths)
    dynamic_relevant: DashMap<PathBuf, ()>,
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
            stats: Mutex::new(CacheStats::default()),
            dynamic_relevant: DashMap::new(),
        }
    }

    /// Get a cached result if it exists and is still valid
    pub fn get(&self, path: &Path) -> Result<Option<DetectionResult>> {
        let path_buf = path.to_path_buf();

        // Find entry and determine what to do with it
        let mut found_key: Option<CacheKey> = None;
        let mut result: Option<DetectionResult> = None;
        let mut should_remove = false;

        // Look for any entry with this path and do fast TTL check
        for entry_ref in self.cache.iter() {
            let (key, entry) = entry_ref.pair();
            if key.path == path_buf {
                found_key = Some(key.clone());

                // Fast TTL check first (no filesystem operations)
                if let Ok(age) = entry.created_at.elapsed() {
                    if age > Duration::from_secs(self.config.ttl_seconds) {
                        // Entry is too old
                        should_remove = true;
                        break;
                    }
                }

                // TTL is still valid, now do expensive file validation
                let current_key = self.calculate_cache_key(path)?;
                if key.file_hash == current_key.file_hash {
                    // Files haven't changed, cache hit
                    result = Some(entry.result.clone());
                    break;
                } else {
                    // Files changed, entry is stale
                    should_remove = true;
                    break;
                }
            }
        }

        // Handle the result
        if let Some(key) = found_key {
            if should_remove {
                self.cache.remove(&key);
                if let Ok(mut stats) = self.stats.lock() {
                    stats.invalidations = stats.invalidations.saturating_add(1);
                    stats.misses = stats.misses.saturating_add(1);
                }
                return Ok(None);
            } else if let Some(res) = result {
                if let Ok(mut stats) = self.stats.lock() {
                    stats.hits = stats.hits.saturating_add(1);
                }
                return Ok(Some(res));
            }
        }

        // No entry found for this path
        if let Ok(mut stats) = self.stats.lock() {
            stats.misses = stats.misses.saturating_add(1);
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

        // Check only critical config directories that directly affect detection
        // Removed .vscode, .idea, node_modules/.bin as they rarely invalidate detection results
        // and cause significant filesystem overhead in cache operations

        // Include dynamic relevant paths (relative to base path)
        for key in self.dynamic_relevant.iter() {
            let rel = key.key();
            let p = path.join(rel);
            if let Ok(metadata) = fs::metadata(&p) {
                if let Ok(modified) = metadata.modified() {
                    file_times.insert(p, modified);
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
        if to_remove > 0 {
            if let Ok(mut stats) = self.stats.lock() {
                stats.invalidations = stats.invalidations.saturating_add(to_remove as u64);
            }
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        if let Ok(stats) = self.stats.lock() {
            CacheStats {
                hits: stats.hits,
                misses: stats.misses,
                invalidations: stats.invalidations,
                entries: self.cache.len(),
            }
        } else {
            CacheStats {
                hits: 0,
                misses: 0,
                invalidations: 0,
                entries: self.cache.len(),
            }
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

        if removed > 0 {
            if let Ok(mut stats) = self.stats.lock() {
                stats.invalidations = stats.invalidations.saturating_add(removed as u64);
            }
        }

        removed
    }

    /// Add a dynamically relevant file or directory (relative path) to monitor
    pub fn add_dynamic_relevant<P: Into<PathBuf>>(&self, relative: P) {
        let p: PathBuf = relative.into();
        self.dynamic_relevant.insert(p, ());
    }

    /// Clear all dynamically relevant paths
    pub fn clear_dynamic_relevant(&self) {
        self.dynamic_relevant.clear();
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

        DetectionResult::new(Some(Arc::new(language)), vec![], 0.9)
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
