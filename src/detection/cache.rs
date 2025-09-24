use crate::types::{CacheConfig, DetectionResult};
use crate::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub result: DetectionResult,
    pub created_at: SystemTime,
    pub file_hash: u64,
}

pub struct DetectionCache {
    cache: DashMap<CacheKey, CacheEntry>,
    config: CacheConfig,
    hits: AtomicU64,
    misses: AtomicU64,
    invalidations: AtomicU64,
    dynamic_relevant: DashMap<PathBuf, ()>,
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub entries: usize,
    pub hit_rate: f64,
}

impl DetectionCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: DashMap::new(),
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
            dynamic_relevant: DashMap::new(),
        }
    }

    pub fn get(&self, path: &Path) -> Result<Option<DetectionResult>> {
        let key = CacheKey {
            path: path.to_path_buf(),
        };

        if let Some(entry_ref) = self.cache.get(&key) {
            let entry = entry_ref.value();

            if let Ok(age) = entry.created_at.elapsed() {
                if age > Duration::from_secs(self.config.ttl_seconds) {
                    drop(entry_ref);
                    self.cache.remove(&key);
                    self.invalidations.fetch_add(1, Ordering::Relaxed);
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
            }

            let current_hash = self.calculate_file_hash(path)?;
            if entry.file_hash == current_hash {
                let result = entry.result.clone();
                drop(entry_ref);
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(result));
            } else {
                drop(entry_ref);
                self.cache.remove(&key);
                self.invalidations.fetch_add(1, Ordering::Relaxed);
                self.misses.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    pub fn put(&self, path: &Path, result: DetectionResult) -> Result<()> {
        let file_hash = self.calculate_file_hash(path)?;
        let key = CacheKey {
            path: path.to_path_buf(),
        };

        let entry = CacheEntry {
            result,
            created_at: SystemTime::now(),
            file_hash,
        };

        self.cache.insert(key, entry);

        if self.cache.len() > self.config.max_entries {
            self.evict_oldest_entries();
        }

        Ok(())
    }

    fn calculate_file_hash(&self, path: &Path) -> Result<u64> {
        let file_times = self.get_relevant_file_times(path)?;
        Ok(self.hash_file_times(&file_times))
    }

    fn get_relevant_file_times(&self, path: &Path) -> Result<HashMap<PathBuf, SystemTime>> {
        let mut file_times = HashMap::new();

        let key_files = [
            "package.json",
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "Cargo.toml",
            "Cargo.lock",
            "go.mod",
            "pyproject.toml",
            "poetry.lock",
            "requirements.txt",
            "tsconfig.json",
            "setup.py",
            "composer.json",
            "composer.lock",
            "Gemfile",
            "Gemfile.lock",
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

    fn hash_file_times(&self, file_times: &HashMap<PathBuf, SystemTime>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        let mut sorted_files: Vec<_> = file_times.iter().collect();
        sorted_files.sort_by_key(|(path, _)| *path);

        for (path, time) in sorted_files {
            path.hash(&mut hasher);
            if let Ok(duration) = time.duration_since(UNIX_EPOCH) {
                duration.as_secs().hash(&mut hasher);
                duration.subsec_nanos().hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    fn evict_oldest_entries(&self) {
        let target_size = (self.config.max_entries as f32 * 0.8) as usize;
        let current_size = self.cache.len();
        let to_remove = current_size.saturating_sub(target_size);

        if to_remove == 0 {
            return;
        }

        let mut candidates: Vec<(CacheKey, SystemTime)> = Vec::with_capacity(to_remove * 2);

        for entry_ref in self.cache.iter().take(to_remove * 2) {
            candidates.push((entry_ref.key().clone(), entry_ref.value().created_at));
        }

        candidates.sort_by_key(|(_, created_at)| *created_at);

        let removed_count = candidates
            .into_iter()
            .take(to_remove)
            .filter(|(key, _)| self.cache.remove(key).is_some())
            .count();

        if removed_count > 0 {
            self.invalidations
                .fetch_add(removed_count as u64, Ordering::Relaxed);
        }
    }

    pub fn clear(&self) {
        let removed = self.cache.len();
        self.cache.clear();
        if removed > 0 {
            self.invalidations
                .fetch_add(removed as u64, Ordering::Relaxed);
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

        CacheStats {
            hits,
            misses,
            invalidations: self.invalidations.load(Ordering::Relaxed),
            entries: self.cache.len(),
            hit_rate,
        }
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn invalidate_path(&self, path: &Path) -> usize {
        let key = CacheKey {
            path: path.to_path_buf(),
        };

        if self.cache.remove(&key).is_some() {
            self.invalidations.fetch_add(1, Ordering::Relaxed);
            1
        } else {
            0
        }
    }

    pub fn add_dynamic_relevant<P: Into<PathBuf>>(&self, relative: P) {
        let p: PathBuf = relative.into();
        self.dynamic_relevant.insert(p, ());
    }

    pub fn clear_dynamic_relevant(&self) {
        self.dynamic_relevant.clear();
    }
}

pub trait CachedDetection {
    fn detect_cached(&mut self, path: &Path, cache: &DetectionCache) -> Result<DetectionResult>;
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
            ttl_seconds: 300,
        };
        DetectionCache::new(config)
    }

    fn create_test_result() -> DetectionResult {
        let language = ProjectIndicator::new(
            "TypeScript".to_string(),
            vec!["tsconfig.json".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            1,
            vec![],
        );

        DetectionResult::new(Some(Arc::new(language)), vec![], 0.9)
    }

    #[test]
    fn test_cache_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        assert!(cache.get(temp_dir.path())?.is_none());

        cache.put(temp_dir.path(), result.clone())?;

        let cached = cache
            .get(temp_dir.path())?
            .ok_or("Failed to unwrap cached result")?;
        assert_eq!(
            cached
                .language
                .as_ref()
                .ok_or("Failed to get language reference")?
                .name,
            "TypeScript"
        );
        assert_eq!(cached.confidence, 0.9);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
        Ok(())
    }

    #[test]
    fn test_cache_file_modification_invalidation() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        fs::write(temp_dir.path().join("package.json"), "{}")?;

        cache.put(temp_dir.path(), result.clone())?;

        assert!(cache.get(temp_dir.path())?.is_some());

        thread::sleep(Duration::from_millis(10));
        fs::write(temp_dir.path().join("package.json"), "{\"name\": \"test\"}")?;

        assert!(cache.get(temp_dir.path())?.is_none());

        let stats = cache.stats();
        assert_eq!(stats.invalidations, 1);
        Ok(())
    }

    #[test]
    fn test_cache_new_file_invalidation() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;

        assert!(cache.get(temp_dir.path())?.is_some());

        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        assert!(cache.get(temp_dir.path())?.is_none());

        let stats = cache.stats();
        assert_eq!(stats.invalidations, 1);
        Ok(())
    }

    #[test]
    fn test_cache_ttl_expiration() -> Result<(), Box<dyn std::error::Error>> {
        let config = CacheConfig {
            enabled: true,
            max_entries: 100,
            ttl_seconds: 1,
        };
        let cache = DetectionCache::new(config);
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;

        assert!(cache.get(temp_dir.path())?.is_some());

        thread::sleep(Duration::from_secs(2));

        assert!(cache.get(temp_dir.path())?.is_none());

        let stats = cache.stats();
        assert!(stats.invalidations >= 1);
        Ok(())
    }

    #[test]
    fn test_cache_size_limit() -> Result<(), Box<dyn std::error::Error>> {
        let config = CacheConfig {
            enabled: true,
            max_entries: 2,
            ttl_seconds: 300,
        };
        let cache = DetectionCache::new(config);
        let result = create_test_result();

        let temp1 = TempDir::new()?;
        let temp2 = TempDir::new()?;
        let temp3 = TempDir::new()?;

        cache.put(temp1.path(), result.clone())?;
        cache.put(temp2.path(), result.clone())?;
        assert_eq!(cache.len(), 2);

        cache.put(temp3.path(), result.clone())?;

        assert!(cache.len() <= 2);

        let stats = cache.stats();
        assert!(stats.invalidations > 0);
        Ok(())
    }

    #[test]
    fn test_cache_invalidate_path() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;
        assert!(cache.get(temp_dir.path())?.is_some());

        let removed = cache.invalidate_path(temp_dir.path());
        assert_eq!(removed, 1);

        assert!(cache.get(temp_dir.path())?.is_none());

        let stats = cache.stats();
        assert_eq!(stats.invalidations, 1);
        Ok(())
    }

    #[test]
    fn test_cache_clear() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
        assert!(cache.get(temp_dir.path())?.is_none());

        let stats = cache.stats();
        assert_eq!(stats.invalidations, 1);
        Ok(())
    }

    #[test]
    fn test_cache_hit_rate() -> Result<(), Box<dyn std::error::Error>> {
        let cache = create_test_cache();
        let temp_dir = TempDir::new()?;
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;

        let _ = cache.get(temp_dir.path())?;
        let _ = cache.get(temp_dir.path())?;
        let _ = cache.get(temp_dir.path())?;

        let temp_other = TempDir::new()?;
        let _ = cache.get(temp_other.path())?;
        let _ = cache.get(temp_other.path())?;

        let stats = cache.stats();
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 2);
        assert_eq!((stats.hit_rate * 10.0).round() / 10.0, 60.0);
        Ok(())
    }

    #[test]
    fn test_cache_concurrent_access() -> Result<(), Box<dyn std::error::Error>> {
        let cache = Arc::new(create_test_cache());
        let temp_dir = Arc::new(TempDir::new()?);
        let result = create_test_result();

        cache.put(temp_dir.path(), result.clone())?;

        let mut handles = vec![];

        for _ in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let temp_clone = Arc::clone(&temp_dir);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = cache_clone.get(temp_clone.path());
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().map_err(|_| "Thread join failed")?;
        }

        let stats = cache.stats();
        assert_eq!(stats.hits, 1000);
        assert!(stats.hit_rate > 99.0);
        Ok(())
    }

    #[test]
    fn test_eviction_algorithm_efficiency() -> Result<(), Box<dyn std::error::Error>> {
        let config = CacheConfig {
            enabled: true,
            max_entries: 100,
            ttl_seconds: 300,
        };
        let cache = DetectionCache::new(config);
        let result = create_test_result();

        let mut temp_dirs = vec![];
        for _ in 0..150 {
            let temp = TempDir::new()?;
            cache.put(temp.path(), result.clone())?;
            temp_dirs.push(temp);
        }

        assert!(cache.len() <= 100);
        assert!(cache.len() >= 80);

        let stats = cache.stats();
        assert!(stats.invalidations >= 50);
        Ok(())
    }
}
