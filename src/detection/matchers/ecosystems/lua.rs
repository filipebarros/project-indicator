use super::helpers::{check_luarocks_lock_dependencies, check_rockspec_dependencies};
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

    if let Some(lock_content) = parsed_cache.get_config_file(&path, "luarocks.lock")? {
        let lock_deps = check_luarocks_lock_dependencies(&lock_content, packages);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push("luarocks.lock".to_owned());
        }
    }

    Ok((found_deps, evidence))
}
