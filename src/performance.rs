use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Per-run memoization of whether a path exists.
///
/// The cache lives for a single detection run (one CLI invocation), so there
/// is no TTL and no eviction: the set of paths touched during one run is
/// naturally bounded by the scan, and the process exits when the run ends.
#[derive(Debug, Default)]
pub struct FileSystemCache {
    exists: Mutex<HashMap<PathBuf, bool>>,
}

impl FileSystemCache {
    /// The lock is only ever held for a lookup or an insert, never across
    /// I/O; a poisoned lock still holds valid data, so recover it.
    fn map(&self) -> MutexGuard<'_, HashMap<PathBuf, bool>> {
        self.exists.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn exists(&self, path: &Path) -> bool {
        if let Some(&cached) = self.map().get(path) {
            return cached;
        }

        let exists = fs::metadata(path).is_ok();
        self.map().insert(path.to_path_buf(), exists);
        exists
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
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

        // Negative results are memoized too
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

    #[test]
    fn test_concurrent_access() -> Result<(), Box<dyn std::error::Error>> {
        let cache = Arc::new(FileSystemCache::default());
        let temp_dir = TempDir::new()?;

        let mut paths = Vec::new();
        for i in 0..20 {
            let path = temp_dir.path().join(format!("file_{}.txt", i));
            fs::write(&path, "x")?;
            paths.push(path);
        }

        let mut handles = Vec::new();
        for _ in 0..4 {
            let cache = Arc::clone(&cache);
            let paths = paths.clone();
            handles.push(std::thread::spawn(move || {
                for path in &paths {
                    assert!(cache.exists(path));
                }
            }));
        }

        for handle in handles {
            handle
                .join()
                .map_err(|_| "concurrent access thread panicked")?;
        }
        Ok(())
    }
}
