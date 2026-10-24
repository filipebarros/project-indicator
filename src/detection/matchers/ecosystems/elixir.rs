use super::helpers::{check_mix_dependencies, first_hit};
use crate::constants::MIX_EXS;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_elixir_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(MIX_EXS, check_mix_dependencies)],
        dependencies,
    )
}
