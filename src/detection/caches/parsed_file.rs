use crate::constants::*;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const MAX_FILE_SIZE: u64 = 1_048_576;
const MAX_CACHE_SIZE: usize = 52_428_800;
const MAX_ENTRIES: usize = 1000;
const INITIAL_CAPACITY: usize = 64; // Pre-allocate for common project sizes

#[derive(Debug, Clone)]
struct CacheEntry {
    content: Option<Arc<String>>,
    last_accessed: u64,
    size: usize,
}

/// Cached parsed value with last access time
#[derive(Debug, Clone)]
struct ParsedValueEntry<T> {
    value: Option<Arc<T>>,
    last_accessed: u64,
}

#[derive(Debug)]
pub struct ParsedFileCache {
    content_cache: Mutex<HashMap<PathBuf, CacheEntry>>,
    json_cache: Mutex<HashMap<PathBuf, ParsedValueEntry<serde_json::Value>>>,
    toml_cache: Mutex<HashMap<PathBuf, ParsedValueEntry<toml::Value>>>,
    parse_failures: Mutex<HashSet<PathBuf>>,
    total_size: Arc<AtomicUsize>,
    access_counter: Arc<AtomicU64>,
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    evictions: Arc<AtomicUsize>,
    parse_hits: Arc<AtomicUsize>,
    parse_misses: Arc<AtomicUsize>,
    parse_failures_count: Arc<AtomicUsize>,
    max_cache_size: usize,
    max_entries: usize,
}

impl Default for ParsedFileCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsedFileCache {
    pub fn new() -> Self {
        Self {
            content_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            json_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            toml_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            parse_failures: Mutex::new(HashSet::with_capacity(16)),
            total_size: Arc::new(AtomicUsize::new(0)),
            access_counter: Arc::new(AtomicU64::new(0)),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            evictions: Arc::new(AtomicUsize::new(0)),
            parse_hits: Arc::new(AtomicUsize::new(0)),
            parse_misses: Arc::new(AtomicUsize::new(0)),
            parse_failures_count: Arc::new(AtomicUsize::new(0)),
            max_cache_size: MAX_CACHE_SIZE,
            max_entries: MAX_ENTRIES,
        }
    }

    pub fn with_limits(max_cache_size: usize, max_entries: usize) -> Self {
        Self {
            content_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            json_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            toml_cache: Mutex::new(HashMap::with_capacity(INITIAL_CAPACITY)),
            parse_failures: Mutex::new(HashSet::with_capacity(16)),
            total_size: Arc::new(AtomicUsize::new(0)),
            access_counter: Arc::new(AtomicU64::new(0)),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            evictions: Arc::new(AtomicUsize::new(0)),
            parse_hits: Arc::new(AtomicUsize::new(0)),
            parse_misses: Arc::new(AtomicUsize::new(0)),
            parse_failures_count: Arc::new(AtomicUsize::new(0)),
            max_cache_size,
            max_entries,
        }
    }

    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        let file_path_ref = file_path.as_ref();
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);

        // Check cache first (short-lived read lock)
        // HashMap supports lookups via Borrow trait, avoiding PathBuf allocation on cache hits
        match self.content_cache.lock() {
            Ok(mut cache) => {
                if let Some(entry) = cache.get_mut(file_path_ref) {
                    // Cache hit - update access time and return
                    entry.last_accessed = access_time;
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(entry.content.clone());
                }
                // Cache miss - drop lock before expensive I/O
            }
            Err(e) => {
                log::warn!(
                    "ParsedFileCache lock poisoned during read for {:?}: {}",
                    file_path_ref,
                    e
                );
                // Continue with file read as fallback
            }
        }

        // Record cache miss
        self.misses.fetch_add(1, Ordering::Relaxed);

        // Check if file exists (no lock held)
        if !file_path_ref.exists() {
            // Single lock acquisition to insert None
            // Only allocate PathBuf when inserting into cache
            match self.content_cache.lock() {
                Ok(mut cache) => {
                    let entry = CacheEntry {
                        content: None,
                        last_accessed: access_time,
                        size: 0,
                    };
                    cache.insert(file_path_ref.to_path_buf(), entry);
                }
                Err(e) => {
                    log::warn!(
                        "ParsedFileCache lock poisoned during None insert for {:?}: {}",
                        file_path_ref,
                        e
                    );
                }
            }
            return Ok(None);
        }

        // Read metadata and file content (no lock held - expensive I/O)
        let metadata = fs::metadata(file_path_ref)
            .with_context(|| format!("Failed to get metadata for: {}", file_path_ref.display()))?;

        if metadata.len() > MAX_FILE_SIZE {
            let content = fs::read_to_string(file_path_ref).with_context(|| {
                format!("Failed to read large file: {}", file_path_ref.display())
            })?;
            // Don't cache large files
            return Ok(Some(Arc::new(content)));
        }

        let content = fs::read_to_string(file_path_ref)
            .with_context(|| format!("Failed to read file: {}", file_path_ref.display()))?;

        let content_size = content.len();
        let result = Some(Arc::new(content));

        // Single lock acquisition for final insert
        match self.content_cache.lock() {
            Ok(mut cache) => {
                // Check if we need to evict based on entry count or size
                if cache.len() >= self.max_entries
                    || self.total_size.load(Ordering::Relaxed) + content_size > self.max_cache_size
                {
                    self.evict_entries_lru(&mut cache, content_size);
                }

                let entry = CacheEntry {
                    content: result.clone(),
                    last_accessed: access_time,
                    size: content_size,
                };
                // Only allocate PathBuf when inserting into cache
                cache.insert(file_path_ref.to_path_buf(), entry);
                self.total_size.fetch_add(content_size, Ordering::Relaxed);
            }
            Err(e) => {
                log::warn!(
                    "ParsedFileCache lock poisoned during content insert for {:?}: {}",
                    file_path_ref,
                    e
                );
                // Still return the content we read
            }
        }

        Ok(result)
    }

    fn evict_entries_lru(&self, cache: &mut HashMap<PathBuf, CacheEntry>, needed_size: usize) {
        let current_entries = cache.len();
        let current_size = self.total_size.load(Ordering::Relaxed);

        // Determine how much to evict
        let target_entries = (self.max_entries as f64 * 0.75) as usize;
        let target_size = (self.max_cache_size as f64 * 0.75) as usize;

        // Sort entries by last_accessed (LRU first)
        let mut entries: Vec<(PathBuf, u64, usize)> = cache
            .iter()
            .map(|(k, v)| (k.clone(), v.last_accessed, v.size))
            .collect();
        entries.sort_by_key(|(_, accessed, _)| *accessed);

        let mut removed_size = 0;
        let mut removed_count = 0;

        // Evict entries until we meet both targets
        for (key, _, _size) in entries {
            if current_entries - removed_count <= target_entries
                && current_size - removed_size + needed_size <= target_size
            {
                break;
            }

            if let Some(entry) = cache.remove(&key) {
                removed_size += entry.size;
                removed_count += 1;
                self.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }

        if removed_size > 0 {
            self.total_size
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(removed_size))
                })
                .ok();
        }
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<toml::Value>> {
        let file_path_ref = file_path.as_ref();
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);

        // Check parsed value cache first
        // HashMap supports lookups via Borrow trait, avoiding PathBuf allocation on cache hits
        if let Ok(mut cache) = self.toml_cache.lock() {
            if let Some(entry) = cache.get_mut(file_path_ref) {
                entry.last_accessed = access_time;
                self.parse_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(entry.value.as_ref().map(|v| (**v).clone()));
            }
        }

        // Check if we've failed to parse this before
        if let Ok(failures) = self.parse_failures.lock() {
            if failures.contains(file_path_ref) {
                self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        }

        self.parse_misses.fetch_add(1, Ordering::Relaxed);

        // Get file content (may hit content cache)
        let content = match self.get_file_content(file_path_ref)? {
            Some(c) => c,
            None => {
                // File doesn't exist - cache None result
                if let Ok(mut cache) = self.toml_cache.lock() {
                    cache.insert(
                        file_path_ref.to_path_buf(),
                        ParsedValueEntry {
                            value: None,
                            last_accessed: access_time,
                        },
                    );
                }
                return Ok(None);
            }
        };

        // Parse TOML
        match toml::from_str(&content) {
            Ok(value) => {
                let arc_value: Arc<toml::Value> = Arc::new(value);
                if let Ok(mut cache) = self.toml_cache.lock() {
                    cache.insert(
                        file_path_ref.to_path_buf(),
                        ParsedValueEntry {
                            value: Some(arc_value.clone()),
                            last_accessed: access_time,
                        },
                    );
                }
                Ok(Some((*arc_value).clone()))
            }
            Err(e) => {
                // Cache parse failure to avoid retrying
                if let Ok(mut failures) = self.parse_failures.lock() {
                    failures.insert(file_path_ref.to_path_buf());
                }
                self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                Err(e).with_context(|| {
                    format!("Failed to parse TOML file: {}", file_path_ref.display())
                })
            }
        }
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let file_path_ref = file_path.as_ref();
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);

        // Check parsed value cache first
        // HashMap supports lookups via Borrow trait, avoiding PathBuf allocation on cache hits
        if let Ok(mut cache) = self.json_cache.lock() {
            if let Some(entry) = cache.get_mut(file_path_ref) {
                entry.last_accessed = access_time;
                self.parse_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(entry.value.as_ref().map(|v| (**v).clone()));
            }
        }

        // Check if we've failed to parse this before
        if let Ok(failures) = self.parse_failures.lock() {
            if failures.contains(file_path_ref) {
                self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                return Ok(None);
            }
        }

        self.parse_misses.fetch_add(1, Ordering::Relaxed);

        // Get file content (may hit content cache)
        let content = match self.get_file_content(file_path_ref)? {
            Some(c) => c,
            None => {
                // File doesn't exist - cache None result
                if let Ok(mut cache) = self.json_cache.lock() {
                    cache.insert(
                        file_path_ref.to_path_buf(),
                        ParsedValueEntry {
                            value: None,
                            last_accessed: access_time,
                        },
                    );
                }
                return Ok(None);
            }
        };

        // Parse JSON
        match serde_json::from_str(&content) {
            Ok(value) => {
                let arc_value: Arc<serde_json::Value> = Arc::new(value);
                if let Ok(mut cache) = self.json_cache.lock() {
                    cache.insert(
                        file_path_ref.to_path_buf(),
                        ParsedValueEntry {
                            value: Some(arc_value.clone()),
                            last_accessed: access_time,
                        },
                    );
                }
                Ok(Some((*arc_value).clone()))
            }
            Err(e) => {
                // Cache parse failure to avoid retrying
                if let Ok(mut failures) = self.parse_failures.lock() {
                    failures.insert(file_path_ref.to_path_buf());
                }
                self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                Err(e).with_context(|| {
                    format!("Failed to parse JSON file: {}", file_path_ref.display())
                })
            }
        }
    }

    pub fn get_cargo_toml<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<toml::Value>> {
        let cargo_path = dir_path.as_ref().join("Cargo.toml");
        self.get_toml_value(cargo_path)
    }

    pub fn get_package_json<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let package_path = dir_path.as_ref().join("package.json");
        self.get_json_value(package_path)
    }

    pub fn get_pyproject_toml<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<toml::Value>> {
        let pyproject_path = dir_path.as_ref().join("pyproject.toml");
        self.get_toml_value(pyproject_path)
    }

    pub fn get_composer_json<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let composer_path = dir_path.as_ref().join("composer.json");
        self.get_json_value(composer_path)
    }

    pub fn get_go_mod<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let go_mod_path = dir_path.as_ref().join("go.mod");
        self.get_file_content(go_mod_path)
    }

    pub fn get_config_file<P: AsRef<Path>>(
        &self,
        dir_path: P,
        filename: &str,
    ) -> Result<Option<Arc<String>>> {
        let file_path = dir_path.as_ref().join(filename);
        self.get_file_content(file_path)
    }

    pub fn get_package_lock_json<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let package_lock_path = dir_path.as_ref().join(PACKAGE_LOCK_JSON);
        self.get_json_value(package_lock_path)
    }

    pub fn get_yarn_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let yarn_lock_path = dir_path.as_ref().join(YARN_LOCK);
        self.get_file_content(yarn_lock_path)
    }

    pub fn get_pnpm_lock_yaml<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let pnpm_lock_path = dir_path.as_ref().join(PNPM_LOCK_YAML);
        self.get_file_content(pnpm_lock_path)
    }

    pub fn get_composer_lock<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let composer_lock_path = dir_path.as_ref().join(COMPOSER_LOCK);
        self.get_json_value(composer_lock_path)
    }

    pub fn get_gemfile_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let gemfile_lock_path = dir_path.as_ref().join(GEMFILE_LOCK);
        self.get_file_content(gemfile_lock_path)
    }

    pub fn get_poetry_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let poetry_lock_path = dir_path.as_ref().join(POETRY_LOCK);
        self.get_file_content(poetry_lock_path)
    }

    pub fn get_cargo_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let cargo_lock_path = dir_path.as_ref().join(CARGO_LOCK);
        self.get_file_content(cargo_lock_path)
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.content_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.json_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.toml_cache.lock() {
            cache.clear();
        }
        if let Ok(mut failures) = self.parse_failures.lock() {
            failures.clear();
        }
        self.total_size.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.parse_hits.store(0, Ordering::Relaxed);
        self.parse_misses.store(0, Ordering::Relaxed);
        self.parse_failures_count.store(0, Ordering::Relaxed);
        self.access_counter.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> CacheStats {
        let content_entries = self.content_cache.lock().map(|c| c.len()).unwrap_or(0);
        let json_entries = self.json_cache.lock().map(|c| c.len()).unwrap_or(0);
        let toml_entries = self.toml_cache.lock().map(|c| c.len()).unwrap_or(0);
        let parse_failure_entries = self.parse_failures.lock().map(|c| c.len()).unwrap_or(0);

        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let hit_rate = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64
        } else {
            0.0
        };

        let parse_hits = self.parse_hits.load(Ordering::Relaxed);
        let parse_misses = self.parse_misses.load(Ordering::Relaxed);
        let parse_hit_rate = if parse_hits + parse_misses > 0 {
            parse_hits as f64 / (parse_hits + parse_misses) as f64
        } else {
            0.0
        };

        CacheStats {
            content_entries,
            json_entries,
            toml_entries,
            parse_failure_entries,
            total_size_bytes: self.total_size.load(Ordering::Relaxed),
            max_size_bytes: self.max_cache_size,
            max_entries: self.max_entries,
            hits,
            misses,
            evictions: self.evictions.load(Ordering::Relaxed),
            hit_rate,
            parse_hits,
            parse_misses,
            parse_failures: self.parse_failures_count.load(Ordering::Relaxed),
            parse_hit_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub content_entries: usize,
    pub json_entries: usize,
    pub toml_entries: usize,
    pub parse_failure_entries: usize,
    pub total_size_bytes: usize,
    pub max_size_bytes: usize,
    pub max_entries: usize,
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub hit_rate: f64,
    pub parse_hits: usize,
    pub parse_misses: usize,
    pub parse_failures: usize,
    pub parse_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cargo_toml_cache() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        let cargo_content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
tokio = "1.0"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content)?;

        let result1 = cache.get_cargo_toml(temp_dir.path())?;
        assert!(result1.is_some());

        let result2 = cache.get_cargo_toml(temp_dir.path())?;
        assert!(result2.is_some());

        let toml1 = result1.ok_or("Failed to get first toml result")?;
        let toml2 = result2.ok_or("Failed to get second toml result")?;
        assert_eq!(
            toml1["package"]["name"]
                .as_str()
                .ok_or("Failed to get package name from first toml")?,
            toml2["package"]["name"]
                .as_str()
                .ok_or("Failed to get package name from second toml")?
        );
        assert_eq!(
            toml1["package"]["name"]
                .as_str()
                .ok_or("Failed to get package name")?,
            "test"
        );
        Ok(())
    }

    #[test]
    fn test_cache_stats() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;
        fs::write(temp_dir.path().join("package.json"), "{\"name\": \"test\"}")?;

        cache.get_cargo_toml(temp_dir.path())?;
        cache.get_package_json(temp_dir.path())?;

        let stats = cache.stats();
        assert_eq!(stats.content_entries, 2); // Both files read
        assert_eq!(stats.toml_entries, 1); // Cargo.toml parsed
        assert_eq!(stats.json_entries, 1); // package.json parsed
        assert!(stats.total_size_bytes > 0);
        Ok(())
    }

    #[test]
    fn test_generic_file_access() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        fs::write(temp_dir.path().join("test.json"), "{\"key\": \"value\"}")?;
        let json_value = cache.get_json_value(temp_dir.path().join("test.json"))?;
        assert!(json_value.is_some());
        assert_eq!(
            json_value.ok_or("Failed to get json value")?["key"]
                .as_str()
                .ok_or("Failed to get key from json")?,
            "value"
        );

        fs::write(temp_dir.path().join("test.toml"), "key = \"value\"")?;
        let toml_value = cache.get_toml_value(temp_dir.path().join("test.toml"))?;
        assert!(toml_value.is_some());
        assert_eq!(
            toml_value.ok_or("Failed to get toml value")?["key"]
                .as_str()
                .ok_or("Failed to get key from toml")?,
            "value"
        );

        fs::write(temp_dir.path().join("test.txt"), "hello world")?;
        let text_content = cache.get_file_content(temp_dir.path().join("test.txt"))?;
        assert!(text_content.is_some());
        assert_eq!(
            *text_content.ok_or("Failed to get text content")?,
            "hello world"
        );

        let stats = cache.stats();
        assert_eq!(stats.content_entries, 3); // All 3 files
        assert_eq!(stats.json_entries, 1); // test.json parsed
        assert_eq!(stats.toml_entries, 1); // test.toml parsed
        Ok(())
    }

    #[test]
    fn test_arc_sharing() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        fs::write(temp_dir.path().join("test.txt"), "shared content")?;

        let content1 = cache.get_file_content(temp_dir.path().join("test.txt"))?;
        let content2 = cache.get_file_content(temp_dir.path().join("test.txt"))?;

        assert!(content1.is_some());
        assert!(content2.is_some());

        let arc1 = content1.ok_or("Expected content1 to be Some")?;
        let arc2 = content2.ok_or("Expected content2 to be Some")?;

        assert_eq!(*arc1, *arc2);
        assert!(Arc::ptr_eq(&arc1, &arc2));

        Ok(())
    }
}
