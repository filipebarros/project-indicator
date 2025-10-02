use super::helpers::{check_sbt_dependencies, check_text_dependencies, try_config_file_deps};
use crate::constants::{BUILD_SBT, BUILD_SC};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Scala ecosystem dependencies
pub fn check_scala_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if try_config_file_deps(
        parsed_cache,
        &path,
        BUILD_SBT,
        dependencies,
        check_sbt_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        BUILD_SC,
        dependencies,
        check_text_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
