use super::helpers::{check_gradle_dependencies, check_pom_xml_dependencies, first_hit};
use crate::constants::{BUILD_GRADLE, BUILD_GRADLE_KTS, POM_XML};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_java_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[
            (POM_XML, check_pom_xml_dependencies),
            (BUILD_GRADLE, check_gradle_dependencies),
            (BUILD_GRADLE_KTS, check_gradle_dependencies),
        ],
        dependencies,
    )
}
