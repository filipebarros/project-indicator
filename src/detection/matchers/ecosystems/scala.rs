use super::helpers::{check_sbt_dependencies, check_text_dependencies};
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

    if let Some(content) = parsed_cache.get_config_file(&path, "build.sbt")? {
        let sbt_deps = check_sbt_dependencies(&content, dependencies);
        if !sbt_deps.is_empty() {
            found_deps.extend(sbt_deps);
            evidence.push("build.sbt".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(content) = parsed_cache.get_config_file(&path, "build.sc")? {
        let mill_deps = check_text_dependencies(&content, dependencies);
        if !mill_deps.is_empty() {
            found_deps.extend(mill_deps);
            evidence.push("build.sc".to_owned());
        }
    }

    Ok((found_deps, evidence))
}
