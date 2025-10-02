use super::helpers::{
    check_pubspec_dependencies, check_pubspec_lock_dependencies, try_config_file_deps,
};
use crate::constants::{PUBSPEC_LOCK, PUBSPEC_YAML};
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

    if try_config_file_deps(
        parsed_cache,
        &path,
        PUBSPEC_YAML,
        dependencies,
        check_pubspec_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        PUBSPEC_LOCK,
        dependencies,
        check_pubspec_lock_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
