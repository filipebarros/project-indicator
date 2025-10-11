use anyhow::{Context, Result};
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

const MAX_FILE_SIZE: u64 = 1_048_576;
const MAX_CACHE_SIZE: usize = 52_428_800;
const MAX_ENTRIES: usize = 1000;

// Adaptive cache capacities based on project size estimation
const SMALL_PROJECT_CAPACITY: usize = 32; // < 50 files
const MEDIUM_PROJECT_CAPACITY: usize = 128; // 50-500 files
const LARGE_PROJECT_CAPACITY: usize = 256; // > 500 files

// Thresholds matching scanner configuration
const SMALL_PROJECT_THRESHOLD: usize = 50;
const EXTREME_SIZE_THRESHOLD: usize = 500;

/// Type identifier for cached values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedValueType {
    Json,
    Toml,
}

/// Trait for types that can be parsed and cached
trait ParsedType: Sized + Clone {
    /// Parse from string content
    fn parse(content: &str) -> Result<Self>;

    /// Estimate memory size of the parsed value
    fn estimate_size(value: &Self) -> usize;

    /// Convert to CachedValue
    fn to_cached_value(value: Arc<Self>, size: usize) -> CachedValue;

    /// Extract from CachedValue if it's the right type
    fn from_cached_value(cached: &CachedValue) -> Option<&Arc<Self>>;

    /// Get the type identifier
    fn value_type() -> CachedValueType;
}

/// Implement ParsedType for serde_json::Value
impl ParsedType for serde_json::Value {
    fn parse(content: &str) -> Result<Self> {
        serde_json::from_str(content).with_context(|| "Failed to parse JSON")
    }

    fn estimate_size(value: &Self) -> usize {
        match serde_json::to_string(value) {
            Ok(s) => s.len(),
            Err(e) => {
                log::warn!("Failed to estimate JSON size for caching: {}", e);
                // Use conservative default of 1KB when estimation fails
                1024
            }
        }
    }

    fn to_cached_value(value: Arc<Self>, size: usize) -> CachedValue {
        CachedValue::ParsedJson { value, size }
    }

    fn from_cached_value(cached: &CachedValue) -> Option<&Arc<Self>> {
        match cached {
            CachedValue::ParsedJson { value, .. } => Some(value),
            _ => None,
        }
    }

    fn value_type() -> CachedValueType {
        CachedValueType::Json
    }
}

/// Implement ParsedType for toml::Value
impl ParsedType for toml::Value {
    fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).with_context(|| "Failed to parse TOML")
    }

    fn estimate_size(value: &Self) -> usize {
        match toml::to_string(value) {
            Ok(s) => s.len(),
            Err(e) => {
                log::warn!("Failed to estimate TOML size for caching: {}", e);
                // Use conservative default of 1KB when estimation fails
                1024
            }
        }
    }

    fn to_cached_value(value: Arc<Self>, size: usize) -> CachedValue {
        CachedValue::ParsedToml { value, size }
    }

    fn from_cached_value(cached: &CachedValue) -> Option<&Arc<Self>> {
        match cached {
            CachedValue::ParsedToml { value, .. } => Some(value),
            _ => None,
        }
    }

    fn value_type() -> CachedValueType {
        CachedValueType::Toml
    }
}

/// Represents different types of cached file content
#[derive(Debug, Clone)]
enum CachedValue {
    /// Raw file content as string
    RawContent { content: Arc<String>, size: usize },
    /// Parsed JSON value with estimated memory size
    ParsedJson {
        value: Arc<serde_json::Value>,
        size: usize,
    },
    /// Parsed TOML value with estimated memory size
    ParsedToml {
        value: Arc<toml::Value>,
        size: usize,
    },
}

impl CachedValue {
    /// Returns the memory size of this cached value
    fn size(&self) -> usize {
        match self {
            CachedValue::RawContent { size, .. } => *size,
            CachedValue::ParsedJson { size, .. } => *size,
            CachedValue::ParsedToml { size, .. } => *size,
        }
    }
}

/// Unified cache entry storing any type of cached file data
#[derive(Debug, Clone)]
struct UnifiedCacheEntry {
    value: Option<CachedValue>,
    last_accessed: u64,
}

#[derive(Debug)]
pub struct ParsedFileCache {
    /// Unified cache storing all types of file content (raw, JSON, TOML)
    cache: DashMap<PathBuf, UnifiedCacheEntry>,
    /// Track files that failed to parse to avoid retrying
    parse_failures: DashMap<PathBuf, ()>,
    /// Total memory used by all cached values
    total_size: Arc<AtomicUsize>,
    /// Monotonic counter for LRU tracking
    access_counter: Arc<AtomicU64>,
    /// Type-specific entry counts for O(1) stats
    content_count: Arc<AtomicUsize>,
    json_count: Arc<AtomicUsize>,
    toml_count: Arc<AtomicUsize>,
    /// Cache hit/miss metrics
    /// - `hits/misses`: Primary cache lookups (includes all entry types)
    /// - `parse_hits/parse_misses`: Parsed value availability (upgrades count as misses)
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    evictions: Arc<AtomicUsize>,
    parse_hits: Arc<AtomicUsize>,
    parse_misses: Arc<AtomicUsize>,
    parse_failures_count: Arc<AtomicUsize>,
    /// Configuration
    max_cache_size: usize,
    max_entries: usize,
}

impl Default for ParsedFileCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsedFileCache {
    /// Determine optimal initial capacity based on estimated project size
    fn determine_capacity(estimated_size: Option<usize>) -> usize {
        match estimated_size {
            Some(size) if size < SMALL_PROJECT_THRESHOLD => SMALL_PROJECT_CAPACITY,
            Some(size) if size > EXTREME_SIZE_THRESHOLD => LARGE_PROJECT_CAPACITY,
            _ => MEDIUM_PROJECT_CAPACITY,
        }
    }

    pub fn new() -> Self {
        let capacity = Self::determine_capacity(None);
        Self {
            cache: DashMap::with_capacity(capacity),
            parse_failures: DashMap::with_capacity(16),
            total_size: Arc::new(AtomicUsize::new(0)),
            access_counter: Arc::new(AtomicU64::new(0)),
            content_count: Arc::new(AtomicUsize::new(0)),
            json_count: Arc::new(AtomicUsize::new(0)),
            toml_count: Arc::new(AtomicUsize::new(0)),
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

    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        let file_path_ref = file_path.as_ref();
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);

        // Check cache first - DashMap provides concurrent access without explicit locking
        // DashMap supports lookups via Borrow trait, avoiding PathBuf allocation on cache hits
        if let Some(mut entry) = self.cache.get_mut(file_path_ref) {
            // Update access time
            entry.last_accessed = access_time;

            // Check if we have the right type cached
            match &entry.value {
                Some(CachedValue::RawContent { content, .. }) => {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(content.clone()));
                }
                None => {
                    // File doesn't exist, cached as None
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
                _ => {
                    // Wrong type cached, fall through to reload
                    // This shouldn't normally happen but handle gracefully
                }
            }
        }

        // Record cache miss
        self.misses.fetch_add(1, Ordering::Relaxed);

        // Check if file exists
        if !file_path_ref.exists() {
            // Insert None into cache (only allocate PathBuf when inserting)
            let entry = UnifiedCacheEntry {
                value: None,
                last_accessed: access_time,
            };
            self.cache.insert(file_path_ref.to_path_buf(), entry);
            return Ok(None);
        }

        // Read metadata and file content (expensive I/O done outside any locks)
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
        let arc_content = Arc::new(content);

        // Check if we need to evict based on entry count or size
        if self.cache.len() >= self.max_entries
            || self.total_size.load(Ordering::Relaxed) + content_size > self.max_cache_size
        {
            self.evict_entries_lru(content_size);
        }

        let entry = UnifiedCacheEntry {
            value: Some(CachedValue::RawContent {
                content: arc_content.clone(),
                size: content_size,
            }),
            last_accessed: access_time,
        };

        // Only allocate PathBuf when inserting into cache
        self.cache.insert(file_path_ref.to_path_buf(), entry);
        self.total_size.fetch_add(content_size, Ordering::Relaxed);
        self.content_count.fetch_add(1, Ordering::Relaxed);

        Ok(Some(arc_content))
    }

    fn evict_entries_lru(&self, needed_size: usize) {
        let current_entries = self.cache.len();
        let current_size = self.total_size.load(Ordering::Relaxed);

        // Determine how much to evict (bring down to 75% capacity)
        let target_entries = (self.max_entries as f64 * 0.75) as usize;
        let target_size = (self.max_cache_size as f64 * 0.75) as usize;

        // Calculate how many entries to remove
        let entries_to_remove = current_entries.saturating_sub(target_entries);
        let size_to_remove = current_size.saturating_sub(target_size).max(needed_size);

        if entries_to_remove == 0 && size_to_remove == 0 {
            return;
        }

        // O(n log k) eviction: Use a max-heap to find the oldest entries
        // This avoids sorting all entries (O(n log n)) by maintaining only
        // the top k oldest entries, where k << n makes this effectively O(n)
        use std::collections::BinaryHeap;

        // Max-heap stores (last_accessed, path) - we want oldest (smallest timestamp) entries
        let mut oldest_entries: BinaryHeap<(u64, PathBuf)> = BinaryHeap::new();

        // Single pass through all entries - O(n log k)
        for entry_ref in self.cache.iter() {
            let key = entry_ref.key().clone();
            let last_accessed = entry_ref.value().last_accessed;

            if oldest_entries.len() < entries_to_remove {
                // Heap not full yet, just insert
                oldest_entries.push((last_accessed, key));
            } else if let Some(&(newest_in_heap, _)) = oldest_entries.peek() {
                // If this entry is older (smaller timestamp) than the newest in heap, replace it
                if last_accessed < newest_in_heap {
                    oldest_entries.pop();
                    oldest_entries.push((last_accessed, key));
                }
            }
        }

        // Now remove the oldest entries
        let mut removed_size = 0;
        let mut removed_count = 0;

        while let Some((_, key)) = oldest_entries.pop() {
            // Check if we've met our targets
            if removed_count >= entries_to_remove && removed_size >= size_to_remove {
                break;
            }

            // Remove from unified cache and track eviction
            if let Some((_, entry)) = self.cache.remove(&key) {
                let entry_size = entry.value.as_ref().map(|v| v.size()).unwrap_or(0);
                removed_size += entry_size;
                removed_count += 1;
                self.evictions.fetch_add(1, Ordering::Relaxed);

                // Decrement type-specific counter
                match &entry.value {
                    Some(CachedValue::RawContent { .. }) | None => {
                        self.content_count.fetch_sub(1, Ordering::Relaxed);
                    }
                    Some(CachedValue::ParsedJson { .. }) => {
                        self.json_count.fetch_sub(1, Ordering::Relaxed);
                    }
                    Some(CachedValue::ParsedToml { .. }) => {
                        self.toml_count.fetch_sub(1, Ordering::Relaxed);
                    }
                }
            }
        }

        // Update total size atomically
        if removed_size > 0 {
            self.total_size
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    Some(current.saturating_sub(removed_size))
                })
                .ok();
        }
    }

    /// Generic method to get a parsed value from the cache
    /// Handles progressive enhancement: None → RawContent → ParsedType
    fn get_parsed_value<T: ParsedType>(&self, file_path: &Path) -> Result<Option<T>> {
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);

        // Check unified cache first for parsed value
        if let Some(mut entry) = self.cache.get_mut(file_path) {
            entry.last_accessed = access_time;

            match &entry.value {
                Some(cached) if T::from_cached_value(cached).is_some() => {
                    // Perfect match - already parsed as the right type
                    self.parse_hits.fetch_add(1, Ordering::Relaxed);
                    let value = T::from_cached_value(cached)
                        .ok_or_else(|| anyhow::anyhow!("Failed to extract value from cache"))?;
                    return Ok(Some((**value).clone()));
                }
                None => {
                    // File doesn't exist, cached as None
                    self.parse_hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
                Some(CachedValue::RawContent { content, .. }) => {
                    // We have raw content, parse it now
                    let content_arc = content.clone();
                    let old_size = content.len();
                    drop(entry); // Release the lock before parsing

                    match T::parse(&content_arc) {
                        Ok(value) => {
                            let size = T::estimate_size(&value);
                            let arc_value = Arc::new(value);

                            // Check if we need to evict before upgrading cache entry
                            if self.cache.len() >= self.max_entries
                                || self.total_size.load(Ordering::Relaxed) + size
                                    > self.max_cache_size
                            {
                                self.evict_entries_lru(size);
                            }

                            // Replace raw content with parsed value
                            let parsed_entry = UnifiedCacheEntry {
                                value: Some(T::to_cached_value(arc_value.clone(), size)),
                                last_accessed: access_time,
                            };

                            // Update size tracking (remove raw content size, add parsed size)
                            self.total_size.fetch_sub(old_size, Ordering::Relaxed);
                            self.total_size.fetch_add(size, Ordering::Relaxed);

                            // Update type counters
                            self.content_count.fetch_sub(1, Ordering::Relaxed);
                            self.increment_type_counter::<T>();

                            self.cache.insert(file_path.to_path_buf(), parsed_entry);
                            self.parse_misses.fetch_add(1, Ordering::Relaxed);
                            return Ok(Some((*arc_value).clone()));
                        }
                        Err(e) => {
                            // Cache parse failure to avoid retrying
                            self.parse_failures.insert(file_path.to_path_buf(), ());
                            self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                            return Err(e).with_context(|| {
                                format!("Failed to parse file: {}", file_path.display())
                            });
                        }
                    }
                }
                _ => {
                    // Wrong type cached, fall through to reload
                }
            }
        }

        // Check if we've failed to parse this before
        if self.parse_failures.contains_key(file_path) {
            self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }

        self.parse_misses.fetch_add(1, Ordering::Relaxed);

        // Get file content (will cache as raw content)
        let content = match self.get_file_content(file_path)? {
            Some(c) => c,
            None => {
                return Ok(None);
            }
        };

        // Parse value
        match T::parse(&content) {
            Ok(value) => {
                let size = T::estimate_size(&value);
                let arc_value = Arc::new(value);

                // Check if we need to evict before caching
                if self.cache.len() >= self.max_entries
                    || self.total_size.load(Ordering::Relaxed) + size > self.max_cache_size
                {
                    self.evict_entries_lru(size);
                }

                // Update cache with parsed value (replacing raw content)
                let parsed_entry = UnifiedCacheEntry {
                    value: Some(T::to_cached_value(arc_value.clone(), size)),
                    last_accessed: access_time,
                };

                // Update size tracking
                let old_size = content.len();
                self.total_size.fetch_sub(old_size, Ordering::Relaxed);
                self.total_size.fetch_add(size, Ordering::Relaxed);

                // Update type counters
                self.content_count.fetch_sub(1, Ordering::Relaxed);
                self.increment_type_counter::<T>();

                self.cache.insert(file_path.to_path_buf(), parsed_entry);
                Ok(Some((*arc_value).clone()))
            }
            Err(e) => {
                // Cache parse failure to avoid retrying
                self.parse_failures.insert(file_path.to_path_buf(), ());
                self.parse_failures_count.fetch_add(1, Ordering::Relaxed);
                Err(e).with_context(|| format!("Failed to parse file: {}", file_path.display()))
            }
        }
    }

    /// Helper to increment the appropriate type counter
    fn increment_type_counter<T: ParsedType>(&self) {
        match T::value_type() {
            CachedValueType::Json => {
                self.json_count.fetch_add(1, Ordering::Relaxed);
            }
            CachedValueType::Toml => {
                self.toml_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<toml::Value>> {
        self.get_parsed_value::<toml::Value>(file_path.as_ref())
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<serde_json::Value>> {
        self.get_parsed_value::<serde_json::Value>(file_path.as_ref())
    }

    pub fn clear(&self) {
        // DashMap::clear() is thread-safe and doesn't require explicit locking
        self.cache.clear();
        self.parse_failures.clear();

        self.total_size.store(0, Ordering::Relaxed);
        self.content_count.store(0, Ordering::Relaxed);
        self.json_count.store(0, Ordering::Relaxed);
        self.toml_count.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.parse_hits.store(0, Ordering::Relaxed);
        self.parse_misses.store(0, Ordering::Relaxed);
        self.parse_failures_count.store(0, Ordering::Relaxed);
        self.access_counter.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> CacheStats {
        // O(1) stats lookup using atomic counters
        let content_entries = self.content_count.load(Ordering::Relaxed);
        let json_entries = self.json_count.load(Ordering::Relaxed);
        let toml_entries = self.toml_count.load(Ordering::Relaxed);
        let parse_failure_entries = self.parse_failures.len();

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
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        fs::write(&cargo_toml_path, cargo_content)?;

        let result1 = cache.get_toml_value(&cargo_toml_path)?;
        assert!(result1.is_some());

        let result2 = cache.get_toml_value(&cargo_toml_path)?;
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
    fn test_unified_cache_stores_different_types() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        // Create different file types
        fs::write(temp_dir.path().join("test.json"), "{\"key\": \"value\"}")?;
        fs::write(temp_dir.path().join("test.toml"), "key = \"value\"")?;
        fs::write(temp_dir.path().join("test.txt"), "hello world")?;

        // Test JSON parsing
        let json_value = cache.get_json_value(temp_dir.path().join("test.json"))?;
        assert!(json_value.is_some());
        assert_eq!(
            json_value.ok_or("Failed to get json value")?["key"]
                .as_str()
                .ok_or("Missing key")?,
            "value"
        );

        // Test TOML parsing
        let toml_value = cache.get_toml_value(temp_dir.path().join("test.toml"))?;
        assert!(toml_value.is_some());
        assert_eq!(
            toml_value.ok_or("Failed to get toml value")?["key"]
                .as_str()
                .ok_or("Missing key")?,
            "value"
        );

        // Test raw content
        let text_content = cache.get_file_content(temp_dir.path().join("test.txt"))?;
        assert!(text_content.is_some());
        assert_eq!(
            *text_content.ok_or("Failed to get text content")?,
            "hello world"
        );

        // Verify stats using O(1) atomic counters
        let stats = cache.stats();
        assert_eq!(stats.content_entries, 1); // Only test.txt as raw content
        assert_eq!(stats.json_entries, 1); // test.json parsed
        assert_eq!(stats.toml_entries, 1); // test.toml parsed
        assert_eq!(cache.cache.len(), 3); // Total of 3 entries
        assert!(stats.total_size_bytes > 0);
        Ok(())
    }

    #[test]
    fn test_cache_upgrades_raw_to_parsed() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        fs::write(temp_dir.path().join("test.json"), "{\"key\": \"value\"}")?;

        // First access: Read raw content
        let content = cache.get_file_content(temp_dir.path().join("test.json"))?;
        assert!(content.is_some());
        let stats1 = cache.stats();
        assert_eq!(stats1.content_entries, 1); // Raw content
        assert_eq!(stats1.json_entries, 0); // Not parsed yet

        // Second access: Parse JSON (should upgrade cache entry)
        let json_value = cache.get_json_value(temp_dir.path().join("test.json"))?;
        assert!(json_value.is_some());
        let stats2 = cache.stats();
        assert_eq!(stats2.content_entries, 0); // Upgraded from raw
        assert_eq!(stats2.json_entries, 1); // Now parsed
        assert_eq!(cache.cache.len(), 1); // Still just 1 entry (upgraded, not duplicated)

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
