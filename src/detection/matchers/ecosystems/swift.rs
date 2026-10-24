use super::helpers::{check_swift_package_dependencies, first_hit};
use crate::constants::PACKAGE_SWIFT;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_swift_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(PACKAGE_SWIFT, check_swift_package_dependencies)],
        dependencies,
    )
}
