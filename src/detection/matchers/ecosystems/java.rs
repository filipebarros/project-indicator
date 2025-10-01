use super::helpers::{check_gradle_dependencies, check_pom_xml_dependencies};
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

    if let Some(xml_content) = parsed_cache.get_config_file(&path, "pom.xml")? {
        let maven_deps = check_pom_xml_dependencies(&xml_content, dependencies);
        if !maven_deps.is_empty() {
            found_deps.extend(maven_deps);
            evidence.push("pom.xml".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(gradle_content) = parsed_cache.get_config_file(&path, "build.gradle")? {
        let gradle_deps = check_gradle_dependencies(&gradle_content, dependencies);
        if !gradle_deps.is_empty() {
            found_deps.extend(gradle_deps);
            evidence.push("build.gradle".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(gradle_kts_content) = parsed_cache.get_config_file(&path, "build.gradle.kts")? {
        let gradle_kts_deps = check_gradle_dependencies(&gradle_kts_content, dependencies);
        if !gradle_kts_deps.is_empty() {
            found_deps.extend(gradle_kts_deps);
            evidence.push("build.gradle.kts".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    Ok((found_deps, evidence))
}
