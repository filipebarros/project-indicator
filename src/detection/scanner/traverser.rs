//! Gitignore-aware directory walking.
//!
//! Only ignore files inside the scan root apply (not parent directories',
//! which are machine-dependent). Hidden files are included: they often hold
//! project configuration (`.eslintrc`, `.babelrc`).

use ignore::WalkBuilder;
use std::path::Path;

/// A name-sorted walker over `path`, at most `max_depth` levels deep
pub fn build_walker(path: &Path, max_depth: usize) -> WalkBuilder {
    let mut builder = WalkBuilder::new(path);
    builder
        .max_depth(Some(max_depth))
        // .gitignore, .ignore and git/info/exclude of the scan root
        .standard_filters(true)
        // Trade-off: for a sub-package scanned on its own (monorepo),
        // entries ignored only by the repo-root .gitignore (node_modules)
        // are walked when max_depth > 1 and count toward the match cap
        .parents(false)
        // Detection must be deterministic across machines: the invoking
        // user's global git ignore (e.g. gitignore.io's Go template ignores
        // `go.work`) has nothing to do with the project being scanned
        .git_global(false)
        .require_git(false)
        .hidden(false)
        // Git rewrites its own directory on every command, which would keep
        // invalidating the persistent cache; it never holds project files
        .filter_entry(|entry| entry.file_name() != ".git")
        // The match cap truncates the walk, so order decides which files survive
        .sort_by_file_name(|a, b| a.cmp(b));
    builder
}
