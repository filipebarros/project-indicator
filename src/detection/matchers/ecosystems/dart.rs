use super::helpers::{check_pubspec_dependencies, check_pubspec_lock_dependencies};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Dart ecosystem dependencies
pub fn check_dart_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(yaml_content) = parsed_cache.get_config_file(&path, "pubspec.yaml")? {
        let pubspec_deps = check_pubspec_dependencies(&yaml_content, dependencies);
        if !pubspec_deps.is_empty() {
            found_deps.extend(pubspec_deps);
            evidence.push("pubspec.yaml".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(content) = parsed_cache.get_config_file(&path, "pubspec.lock")? {
        let lock_deps = check_pubspec_lock_dependencies(&content, dependencies);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push("pubspec.lock".to_owned());
        }
    }

    Ok((found_deps, evidence))
}
