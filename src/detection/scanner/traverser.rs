//! File system traversal with gitignore support
//!
//! This module provides efficient directory walking with:
//! - GitIgnore support (.gitignore, .ignore, git/info/exclude)
//! - Configurable depth limits
//! - Directory size estimation for adaptive timeouts
//! - Standard filters for common ignore patterns
//!
//! # GitIgnore Integration
//!
//! The traverser respects:
//! - `.gitignore` files (repository-specific)
//! - `.ignore` files (tool-specific)
//! - `git/info/exclude` (global excludes)
//! - Parent directory ignore files
//!
//! Hidden files are included by default since they often contain
//! project configuration (e.g., `.eslintrc`, `.babelrc`).

use ignore::WalkBuilder;
use std::path::Path;

pub struct FileSystemTraverser {
    max_depth: usize,
}

impl FileSystemTraverser {
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }

    /// Configure WalkBuilder with gitignore support
    fn configure_walker(&self, path: &Path) -> WalkBuilder {
        let mut builder = WalkBuilder::new(path);
        builder
            .max_depth(Some(self.max_depth))
            .standard_filters(true) // Enable .gitignore, .ignore, git/info/exclude
            .parents(true) // Walk up directory tree to find ignore files
            .git_ignore(true) // Specifically enable gitignore support
            .require_git(false) // Work without .git directory
            .hidden(false); // Include hidden files/dirs
        builder
    }

    /// Estimate directory size by counting immediate children
    /// Used for adaptive timeout calculation
    pub fn estimate_directory_size(&self, path: &Path) -> usize {
        std::fs::read_dir(path)
            .map(|entries| entries.count())
            .unwrap_or(0)
    }

    /// Get configured WalkBuilder for external use
    pub fn build_walker(&self, path: &Path) -> WalkBuilder {
        self.configure_walker(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_directory() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create some test files
        fs::write(root.join("file1.rs"), "fn main() {}")?;
        fs::write(root.join("file2.rs"), "fn test() {}")?;
        fs::write(root.join(".gitignore"), "target/")?;

        // Create nested directory
        let nested = root.join("src");
        fs::create_dir(&nested)?;
        fs::write(nested.join("lib.rs"), "// lib")?;

        // Create ignored directory
        let ignored = root.join("target");
        fs::create_dir(&ignored)?;
        fs::write(ignored.join("build.rs"), "// should be ignored")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_traverser_creation() {
        let traverser = FileSystemTraverser::new(3);
        assert_eq!(traverser.max_depth, 3);
    }

    #[test]
    fn test_directory_size_estimation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_directory()?;
        let traverser = FileSystemTraverser::new(3);

        let size = traverser.estimate_directory_size(temp_dir.path());
        // Should count: file1.rs, file2.rs, .gitignore, src/, target/
        assert!(size >= 3, "Should count at least the immediate children");

        Ok(())
    }

    #[test]
    fn test_walker_configuration() -> Result<(), Box<dyn std::error::Error>> {
        let traverser = FileSystemTraverser::new(5);
        let temp_dir = TempDir::new()?;
        let walker = traverser.build_walker(temp_dir.path());

        // Can't directly test WalkBuilder internals, but we can verify it builds
        // Just verify it executes without panicking
        let _count = walker.build().count();
        Ok(())
    }

    #[test]
    fn test_estimate_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let traverser = FileSystemTraverser::new(3);

        let size = traverser.estimate_directory_size(temp_dir.path());
        assert_eq!(size, 0);
        Ok(())
    }

    #[test]
    fn test_estimate_nonexistent_directory() {
        let traverser = FileSystemTraverser::new(3);
        let size = traverser.estimate_directory_size(Path::new("/nonexistent/path"));
        assert_eq!(size, 0);
    }
}
