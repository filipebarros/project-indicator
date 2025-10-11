use super::helpers::{
    check_gemfile_lock_dependencies, check_text_dependencies, try_config_file_deps,
};
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

    if try_config_file_deps(
        parsed_cache,
        &path,
        GEMFILE,
        gems,
        check_text_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    let gemfile_lock_path = path.as_ref().join(GEMFILE_LOCK);
    if let Some(content) = parsed_cache.get_file_content(&gemfile_lock_path)? {
        let lock_deps = check_gemfile_lock_dependencies(&content, gems);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push(GEMFILE_LOCK.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
