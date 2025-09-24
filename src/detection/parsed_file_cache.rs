use anyhow::{Context, Result};
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const MAX_FILE_SIZE: u64 = 1_048_576;
const MAX_CACHE_SIZE: usize = 52_428_800;
const MAX_ENTRIES: usize = 1000;

#[derive(Debug)]
pub struct ParsedFileCache {
    content_cache: DashMap<PathBuf, Option<Arc<String>>>,
    total_size: Arc<AtomicUsize>,
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
            content_cache: DashMap::new(),
            total_size: Arc::new(AtomicUsize::new(0)),
            max_cache_size: MAX_CACHE_SIZE,
            max_entries: MAX_ENTRIES,
        }
    }

    pub fn with_limits(max_cache_size: usize, max_entries: usize) -> Self {
        Self {
            content_cache: DashMap::new(),
            total_size: Arc::new(AtomicUsize::new(0)),
            max_cache_size,
            max_entries,
        }
    }

    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        let file_path = file_path.as_ref().to_path_buf();

        if let Some(cached_result) = self.content_cache.get(&file_path) {
            return Ok(cached_result.value().clone());
        }

        if !file_path.exists() {
            self.content_cache.insert(file_path, None);
            return Ok(None);
        }

        let metadata = fs::metadata(&file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

        if metadata.len() > MAX_FILE_SIZE {
            let content = fs::read_to_string(&file_path)
                .with_context(|| format!("Failed to read large file: {}", file_path.display()))?;
            return Ok(Some(Arc::new(content)));
        }

        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let content_size = content.len();
        let result = Some(Arc::new(content));

        if self.content_cache.len() >= self.max_entries {
            self.evict_entries();
        }

        self.content_cache.insert(file_path, result.clone());
        self.total_size.fetch_add(content_size, Ordering::Relaxed);

        Ok(result)
    }

    fn evict_entries(&self) {
        let current_entries = self.content_cache.len();
        let target_size = (self.max_entries as f64 * 0.75) as usize;
        let to_remove = current_entries.saturating_sub(target_size);

        if to_remove == 0 {
            return;
        }

        let mut removed_size = 0;

        for (count, entry) in self.content_cache.iter().enumerate() {
            if count >= to_remove {
                break;
            }

            let key = entry.key().clone();
            if let Some(content) = entry.value().as_ref() {
                removed_size += content.len();
            }
            drop(entry);

            self.content_cache.remove(&key);
        }

        if removed_size > 0 {
            self.total_size.fetch_sub(removed_size, Ordering::Relaxed);
        }
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<toml::Value>> {
        if let Some(content) = self.get_file_content(file_path.as_ref())? {
            let value = toml::from_str(&content).with_context(|| {
                format!(
                    "Failed to parse TOML file: {}",
                    file_path.as_ref().display()
                )
            })?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<serde_json::Value>> {
        if let Some(content) = self.get_file_content(file_path.as_ref())? {
            let value = serde_json::from_str(&content).with_context(|| {
                format!(
                    "Failed to parse JSON file: {}",
                    file_path.as_ref().display()
                )
            })?;
            Ok(Some(value))
        } else {
            Ok(None)
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
        let package_lock_path = dir_path.as_ref().join("package-lock.json");
        self.get_json_value(package_lock_path)
    }

    pub fn get_yarn_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let yarn_lock_path = dir_path.as_ref().join("yarn.lock");
        self.get_file_content(yarn_lock_path)
    }

    pub fn get_pnpm_lock_yaml<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let pnpm_lock_path = dir_path.as_ref().join("pnpm-lock.yaml");
        self.get_file_content(pnpm_lock_path)
    }

    pub fn get_composer_lock<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<Option<serde_json::Value>> {
        let composer_lock_path = dir_path.as_ref().join("composer.lock");
        self.get_json_value(composer_lock_path)
    }

    pub fn get_gemfile_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let gemfile_lock_path = dir_path.as_ref().join("Gemfile.lock");
        self.get_file_content(gemfile_lock_path)
    }

    pub fn get_poetry_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let poetry_lock_path = dir_path.as_ref().join("poetry.lock");
        self.get_file_content(poetry_lock_path)
    }

    pub fn get_cargo_lock<P: AsRef<Path>>(&self, dir_path: P) -> Result<Option<Arc<String>>> {
        let cargo_lock_path = dir_path.as_ref().join("Cargo.lock");
        self.get_file_content(cargo_lock_path)
    }

    pub fn clear(&self) {
        self.content_cache.clear();
        self.total_size.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.content_cache.len(),
            total_size_bytes: self.total_size.load(Ordering::Relaxed),
            max_size_bytes: self.max_cache_size,
            max_entries: self.max_entries,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub total_size_bytes: usize,
    pub max_size_bytes: usize,
    pub max_entries: usize,
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
        assert_eq!(stats.entries, 2);
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

        assert_eq!(cache.stats().entries, 3);
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
