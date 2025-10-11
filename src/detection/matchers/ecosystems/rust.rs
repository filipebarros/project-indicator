use super::helpers::{check_cargo_lock_dependencies, check_toml_dependencies};
use crate::constants::{CARGO_LOCK, CARGO_TOML};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Rust ecosystem dependencies
pub fn check_rust_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    let cargo_toml_path = path.as_ref().join(CARGO_TOML);
    if let Some(toml_value) = parsed_cache.get_toml_value(&cargo_toml_path)? {
        let cargo_deps = check_toml_dependencies(&toml_value, dependencies);
        if !cargo_deps.is_empty() {
            found_deps.extend(cargo_deps);
            evidence.push(CARGO_TOML.to_owned());
            return Ok((found_deps, evidence));
        }
    }

    let cargo_lock_path = path.as_ref().join(CARGO_LOCK);
    if let Some(content) = parsed_cache.get_file_content(&cargo_lock_path)? {
        let lock_deps = check_cargo_lock_dependencies(&content, dependencies);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push(CARGO_LOCK.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
