use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// The `limit` smallest of `items`, sorted, in memory bounded by `limit`
/// however many items stream through (and the same ones whatever the order
/// they arrive in, which a directory listing does not promise).
pub(crate) fn smallest<T: Ord>(items: impl Iterator<Item = T>, limit: usize) -> Vec<T> {
    let mut kept = std::collections::BinaryHeap::with_capacity(limit + 1);
    for item in items {
        kept.push(item);
        if kept.len() > limit {
            kept.pop(); // the largest of the kept ones
        }
    }
    kept.into_sorted_vec()
}

/// Per-run memoization of whether a path exists.
///
/// No TTL or eviction: lives for a single detection run, on one thread.
#[derive(Debug, Default)]
pub struct FileSystemCache {
    exists: RefCell<HashMap<PathBuf, bool>>,
    /// Directories whose mtime would reveal a probed path appearing
    watched: RefCell<BTreeSet<PathBuf>>,
    /// A probed path was a symlink whose target is missing: creating the
    /// target moves no timestamp the persistent cache could watch
    dangling_symlink: Cell<bool>,
}

impl FileSystemCache {
    pub fn exists(&self, path: &Path) -> bool {
        if let Some(&cached) = self.exists.borrow().get(path) {
            return cached;
        }

        let exists = fs::metadata(path).is_ok();
        if !exists && fs::symlink_metadata(path).is_ok() {
            self.dangling_symlink.set(true);
        }
        self.exists.borrow_mut().insert(path.to_path_buf(), exists);
        exists
    }

    /// `exists`, and when `path` is missing remember the nearest existing
    /// directory above it: creating the path changes that directory's mtime,
    /// which is how the persistent cache notices it appearing. Needed for
    /// multi-level paths (`src/main/java`), where no level directory changes.
    pub fn exists_watched(&self, path: &Path) -> bool {
        let exists = self.exists(path);
        if !exists {
            if let Some(dir) = path.ancestors().skip(1).find(|dir| self.exists(dir)) {
                self.watched.borrow_mut().insert(dir.to_path_buf());
            }
        }
        exists
    }

    /// Whether any probed path was a symlink to nowhere
    pub fn saw_dangling_symlink(&self) -> bool {
        self.dangling_symlink.get()
    }

    /// The directories recorded by `exists_watched`, sorted
    pub fn watched_dirs(&self) -> Vec<PathBuf> {
        self.watched.borrow().iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_smallest_ignores_the_order_it_is_given() {
        let descending = (0..100).rev().map(|i| format!("p{i:03}"));
        let expected: Vec<String> = (0..5).map(|i| format!("p{i:03}")).collect();

        assert_eq!(smallest(descending, 5), expected);
    }

    #[test]
    fn test_smallest_returns_everything_when_there_are_few() {
        let names = ["b", "a"].map(String::from);
        assert_eq!(smallest(names.into_iter(), 5), ["a", "b"]);
    }

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

    #[cfg(unix)]
    #[test]
    fn test_a_dangling_symlink_is_remembered_as_unresolvable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let link = temp_dir.path().join("package.json");
        std::os::unix::fs::symlink(temp_dir.path().join("missing"), &link)?;
        let cache = FileSystemCache::default();

        assert!(!cache.exists(&link));
        assert!(!cache.exists(&temp_dir.path().join("absent")));

        assert!(cache.saw_dangling_symlink());
        Ok(())
    }

    #[test]
    fn test_a_plain_missing_path_is_not_a_dangling_symlink(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = FileSystemCache::default();

        assert!(!cache.exists(&temp_dir.path().join("absent")));

        assert!(!cache.saw_dangling_symlink());
        Ok(())
    }

    #[test]
    fn test_directories_exist() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        assert!(FileSystemCache::default().exists(temp_dir.path()));
        Ok(())
    }

    #[test]
    fn test_a_missing_path_records_the_nearest_existing_directory_above_it(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("a/b"))?;
        let cache = FileSystemCache::default();

        assert!(!cache.exists_watched(&temp_dir.path().join("a/b/c/d/marker")));

        // creating c (or anything below it) changes a/b, nothing deeper
        assert_eq!(cache.watched_dirs(), vec![temp_dir.path().join("a/b")]);
        Ok(())
    }

    #[test]
    fn test_an_existing_path_records_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("marker"), "")?;
        let cache = FileSystemCache::default();

        assert!(cache.exists_watched(&temp_dir.path().join("marker")));

        assert!(cache.watched_dirs().is_empty());
        Ok(())
    }
}
