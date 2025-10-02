use super::helpers::{check_mix_dependencies, try_config_file_deps};
use crate::constants::MIX_EXS;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Elixir ecosystem dependencies
pub fn check_elixir_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    // Check mix.exs (Elixir/Mix)
    if try_config_file_deps(
        parsed_cache,
        &path,
        MIX_EXS,
        dependencies,
        check_mix_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
