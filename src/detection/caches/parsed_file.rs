//! Per-run cache for file contents and parsed JSON/TOML values.
//!
//! Manifests like `package.json` are touched by several components during one
//! detection run (fast path, framework detector, root indicators); this cache
//! ensures each file is read and parsed at most once per run. The cache lives
//! for a single CLI invocation, so there is no eviction or memory budget.

use anyhow::{Context, Result};
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Files above this size are read but not cached (1MB safety limit).
const MAX_FILE_SIZE: u64 = 1_048_576;

#[derive(Debug, Default)]
pub struct ParsedFileCache {
    /// Raw file contents; `None` records a missing file so it isn't re-stat'd
    contents: DashMap<PathBuf, Option<Arc<String>>>,
    /// Parsed JSON values; `None` records a missing file or parse failure
    json_values: DashMap<PathBuf, Option<Arc<serde_json::Value>>>,
    /// Parsed TOML values; `None` records a missing file or parse failure
    toml_values: DashMap<PathBuf, Option<Arc<toml::Value>>>,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl ParsedFileCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        let file_path = file_path.as_ref();

        // DashMap supports &Path lookups via the Borrow trait, avoiding a
        // PathBuf allocation on cache hits
        if let Some(cached) = self.contents.get(file_path) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached.clone());
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        if !file_path.exists() {
            self.contents.insert(file_path.to_path_buf(), None);
            return Ok(None);
        }

        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

        if metadata.len() > MAX_FILE_SIZE {
            // Read but don't cache oversized files
            let content = fs::read_to_string(file_path)
                .with_context(|| format!("Failed to read large file: {}", file_path.display()))?;
            return Ok(Some(Arc::new(content)));
        }

        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let arc_content = Arc::new(content);
        self.contents
            .insert(file_path.to_path_buf(), Some(arc_content.clone()));

        Ok(Some(arc_content))
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let file_path = file_path.as_ref();

        if let Some(cached) = self.json_values.get(file_path) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached.as_ref().map(|value| (**value).clone()));
        }

        let parsed = match self.get_file_content(file_path)? {
            Some(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(value) => Some(Arc::new(value)),
                Err(e) => {
                    // Memoize the failure so the file isn't re-parsed this run
                    log::warn!("Failed to parse JSON from {}: {}", file_path.display(), e);
                    None
                }
            },
            None => None,
        };

        self.json_values
            .insert(file_path.to_path_buf(), parsed.clone());

        Ok(parsed.map(|value| (*value).clone()))
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<toml::Value>> {
        let file_path = file_path.as_ref();

        if let Some(cached) = self.toml_values.get(file_path) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached.as_ref().map(|value| (**value).clone()));
        }

        let parsed = match self.get_file_content(file_path)? {
            Some(content) => match toml::from_str::<toml::Value>(&content) {
                Ok(value) => Some(Arc::new(value)),
                Err(e) => {
                    // Memoize the failure so the file isn't re-parsed this run
                    log::warn!("Failed to parse TOML from {}: {}", file_path.display(), e);
                    None
                }
            },
            None => None,
        };

        self.toml_values
            .insert(file_path.to_path_buf(), parsed.clone());

        Ok(parsed.map(|value| (*value).clone()))
    }

    pub fn clear(&self) {
        self.contents.clear();
        self.json_values.clear();
        self.toml_values.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let hit_rate = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64
        } else {
            0.0
        };

        CacheStats {
            content_entries: self.contents.len(),
            json_entries: self.json_values.len(),
            toml_entries: self.toml_values.len(),
            hits,
            misses,
            hit_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub content_entries: usize,
    pub json_entries: usize,
    pub toml_entries: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_content_is_cached() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "hello")?;

        let first = cache
            .get_file_content(&file_path)?
            .ok_or("expected content")?;
        assert_eq!(*first, "hello");

        let second = cache
            .get_file_content(&file_path)?
            .ok_or("expected content")?;
        assert_eq!(*second, "hello");

        let stats = cache.stats();
        assert_eq!(stats.content_entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        Ok(())
    }

    #[test]
    fn test_missing_file_is_cached_as_none() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("missing.txt");

        assert!(cache.get_file_content(&file_path)?.is_none());
        assert!(cache.get_file_content(&file_path)?.is_none());

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        Ok(())
    }

    #[test]
    fn test_json_parsing_and_memoization() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("package.json");
        fs::write(&file_path, r#"{"name": "test", "version": "1.0.0"}"#)?;

        let value = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert_eq!(
            value.get("name").and_then(|n| n.as_str()),
            Some("test"),
            "parsed JSON should expose fields"
        );

        // Second lookup hits the parsed-value memo
        let value = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert_eq!(value.get("version").and_then(|v| v.as_str()), Some("1.0.0"));

        assert_eq!(cache.stats().json_entries, 1);
        Ok(())
    }

    #[test]
    fn test_toml_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("Cargo.toml");
        fs::write(&file_path, "[package]\nname = \"test\"\n")?;

        let value = cache.get_toml_value(&file_path)?.ok_or("expected TOML")?;
        assert_eq!(
            value
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str()),
            Some("test")
        );
        Ok(())
    }

    #[test]
    fn test_parse_failure_returns_none_and_is_memoized() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("broken.json");
        fs::write(&file_path, "not json at all")?;

        assert!(cache.get_json_value(&file_path)?.is_none());

        // The failure is memoized for the rest of the run: fixing the file
        // does not change the cached result
        fs::write(&file_path, "{}")?;
        assert!(cache.get_json_value(&file_path)?.is_none());
        Ok(())
    }

    #[test]
    fn test_missing_file_json_lookup() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        assert!(cache
            .get_json_value(temp_dir.path().join("missing.json"))?
            .is_none());
        Ok(())
    }

    #[test]
    fn test_clear() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.json");
        fs::write(&file_path, "{}")?;

        cache.get_json_value(&file_path)?;
        assert!(cache.stats().json_entries > 0);

        cache.clear();
        let stats = cache.stats();
        assert_eq!(stats.content_entries, 0);
        assert_eq!(stats.json_entries, 0);
        assert_eq!(stats.toml_entries, 0);
        Ok(())
    }
}
