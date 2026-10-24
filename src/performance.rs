use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Per-run memoization of whether a path exists.
///
/// No TTL or eviction: lives for a single detection run, on one thread.
#[derive(Debug, Default)]
pub struct FileSystemCache {
    exists: RefCell<HashMap<PathBuf, bool>>,
}

impl FileSystemCache {
    pub fn exists(&self, path: &Path) -> bool {
        if let Some(&cached) = self.exists.borrow().get(path) {
            return cached;
        }

        let exists = fs::metadata(path).is_ok();
        self.exists.borrow_mut().insert(path.to_path_buf(), exists);
        exists
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_existence_is_memoized_for_the_run() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content")?;

        assert!(cache.exists(&file_path));

        // Memoized: the run sees one consistent snapshot even if the file
        // disappears mid-run
        fs::remove_file(&file_path)?;
        assert!(cache.exists(&file_path));
        Ok(())
    }

    #[test]
    fn test_missing_path_is_cached_as_missing() -> Result<(), Box<dyn std::error::Error>> {
        let cache = FileSystemCache::default();
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("later.txt");

        assert!(!cache.exists(&path));

        fs::write(&path, "x")?;
        assert!(!cache.exists(&path));
        Ok(())
    }

    #[test]
    fn test_directories_exist() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        assert!(FileSystemCache::default().exists(temp_dir.path()));
        Ok(())
    }
}
