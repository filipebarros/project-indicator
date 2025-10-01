use super::helpers::{check_composer_lock_dependencies, check_json_dependencies};
use crate::constants::{COMPOSER_JSON, COMPOSER_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect PHP ecosystem dependencies
pub fn check_php_ecosystem<P: AsRef<Path>>(
    path: P,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(json_value) = parsed_cache.get_composer_json(&path)? {
        let composer_deps = check_json_dependencies(&json_value, packages);
        if !composer_deps.is_empty() {
            found_deps.extend(composer_deps);
            evidence.push(COMPOSER_JSON.to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(json_value) = parsed_cache.get_composer_lock(&path)? {
        let lock_deps = check_composer_lock_dependencies(&json_value, packages);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push(COMPOSER_LOCK.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
