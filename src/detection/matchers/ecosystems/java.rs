use super::helpers::{check_gradle_dependencies, check_pom_xml_dependencies, try_config_file_deps};
use crate::constants::{BUILD_GRADLE, BUILD_GRADLE_KTS, POM_XML};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Java ecosystem dependencies
pub fn check_java_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if try_config_file_deps(
        parsed_cache,
        &path,
        POM_XML,
        dependencies,
        check_pom_xml_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        BUILD_GRADLE,
        dependencies,
        check_gradle_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        BUILD_GRADLE_KTS,
        dependencies,
        check_gradle_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
