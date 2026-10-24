use super::helpers::{check_mill_dependencies, check_sbt_dependencies, first_hit};
use crate::constants::{BUILD_SBT, BUILD_SC};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_scala_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[
            (BUILD_SBT, check_sbt_dependencies),
            (BUILD_SC, check_mill_dependencies),
        ],
        dependencies,
    )
}
