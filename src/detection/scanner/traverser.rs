//! Gitignore-aware directory walking.
//!
//! Only ignore files inside the scan root apply (not parent directories',
//! which are machine-dependent). Hidden files are included: they often hold
//! project configuration (`.eslintrc`, `.babelrc`).

use crate::detection::directory::DirectoryListing;
use ignore::WalkBuilder;
use std::fs;
use std::path::Path;

/// Ignore files larger than this are not trusted to the walker
const MAX_IGNORE_FILE_BYTES: u64 = 1_048_576;

/// An ignore file is safe to read when absent or a small regular file. A
/// repository can plant a FIFO (reading blocks forever) or a symlink to
/// /dev/zero (reading never ends) under one of these names.
fn ignore_file_is_safe(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.len() <= MAX_IGNORE_FILE_BYTES,
        Err(_) => true,
    }
}

pub(crate) fn root_ignore_files_are_safe(root: &Path) -> bool {
    [".gitignore", ".ignore", ".git/info/exclude"]
        .iter()
        .all(|name| ignore_file_is_safe(&root.join(name)))
}

pub(crate) fn listing_ignore_files_are_safe(root: &Path, listing: &DirectoryListing) -> bool {
    [".gitignore", ".ignore"]
        .iter()
        .all(|name| !listing.contains_name(name) || ignore_file_is_safe(&root.join(name)))
        && (!listing.contains_name(".git") || ignore_file_is_safe(&root.join(".git/info/exclude")))
}

/// A name-sorted walker over `path`'s immediate entries (the walk never
/// descends below the root)
pub fn build_walker(path: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(path);
    let ignores_safe = root_ignore_files_are_safe(path);
    builder
        .max_depth(Some(1))
        // .gitignore, .ignore and git/info/exclude of the scan root, unless
        // one of them is not a plain small file
        .standard_filters(true)
        .git_ignore(ignores_safe)
        .ignore(ignores_safe)
        .git_exclude(ignores_safe)
        // For a sub-package scanned on its own (monorepo), an ancestor
        // .gitignore is machine-dependent context and must not hide the
        // sub-package's own root files
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
