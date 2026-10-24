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

    /// Configured WalkBuilder with gitignore support
    pub fn build_walker(&self, path: &Path) -> WalkBuilder {
        let mut builder = WalkBuilder::new(path);
        builder
            .max_depth(Some(self.max_depth))
            .standard_filters(true) // Enable .gitignore, .ignore, git/info/exclude
            .parents(true) // Walk up directory tree to find ignore files
            .git_ignore(true)
            // Detection must be deterministic across machines: the
            // invoking user's personal global git ignore (core.excludesFile
            // or the ~/.config/git/ignore default) has nothing to do with
            // the project being scanned. Without this, a common template
            // (e.g. gitignore.io's Go template ignores `go.work`) silently
            // makes real root-indicator files invisible to detection.
            .git_global(false)
            .require_git(false) // Work without .git directory
            .hidden(false) // Include hidden files/dirs
            // Stable traversal order: the match cap truncates the walk, so
            // order decides which files survive
            .sort_by_file_name(|a, b| a.cmp(b));
        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_traverser_creation() {
        let traverser = FileSystemTraverser::new(3);
        assert_eq!(traverser.max_depth, 3);
    }

    #[test]
    fn test_walker_configuration() -> Result<(), Box<dyn std::error::Error>> {
        let traverser = FileSystemTraverser::new(5);
        let temp_dir = TempDir::new()?;
        let walker = traverser.build_walker(temp_dir.path());

        // Can't directly test WalkBuilder internals, but we can verify it builds
        let _count = walker.build().count();
        Ok(())
    }
}
