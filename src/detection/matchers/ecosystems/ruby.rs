use super::helpers::{check_gemfile_lock_dependencies, check_text_dependencies};
use crate::constants::{GEMFILE, GEMFILE_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Ruby ecosystem dependencies
pub fn check_ruby_ecosystem<P: AsRef<Path>>(
    path: P,
    gems: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(content) = parsed_cache.get_config_file(&path, "Gemfile")? {
        let gemfile_deps = check_text_dependencies(&content, gems);
        if !gemfile_deps.is_empty() {
            found_deps.extend(gemfile_deps);
            evidence.push(GEMFILE.to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(content) = parsed_cache.get_gemfile_lock(&path)? {
        let lock_deps = check_gemfile_lock_dependencies(&content, gems);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push(GEMFILE_LOCK.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
