use super::helpers::{check_swift_package_dependencies, try_config_file_deps};
use crate::constants::PACKAGE_SWIFT;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Swift ecosystem dependencies
pub fn check_swift_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    // Check Package.swift (Swift Package Manager)
    if try_config_file_deps(
        parsed_cache,
        &path,
        PACKAGE_SWIFT,
        dependencies,
        check_swift_package_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
