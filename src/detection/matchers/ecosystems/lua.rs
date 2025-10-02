use super::helpers::{
    check_luarocks_lock_dependencies, check_rockspec_dependencies, try_config_file_deps,
};
use crate::constants::LUAROCKS_LOCK;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Lua ecosystem dependencies
pub fn check_lua_ecosystem<P: AsRef<Path>>(
    path: P,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(rockspec_content) = parsed_cache.get_config_file(&path, "*.rockspec")? {
        let rockspec_deps = check_rockspec_dependencies(&rockspec_content, packages);
        if !rockspec_deps.is_empty() {
            found_deps.extend(rockspec_deps);
            evidence.push("rockspec".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        LUAROCKS_LOCK,
        packages,
        check_luarocks_lock_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
