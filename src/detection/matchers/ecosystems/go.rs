use super::helpers::{check_go_mod_dependencies, first_hit};
use crate::constants::GO_MOD;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_go_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(GO_MOD, check_go_mod_dependencies)],
        dependencies,
    )
}
