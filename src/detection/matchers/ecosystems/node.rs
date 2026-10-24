use super::helpers::{
    check_json_dependencies, check_package_lock_dependencies, check_pnpm_lock_dependencies,
    check_yarn_lock_dependencies, first_hit, hit,
};
use crate::constants::{PACKAGE_JSON, PACKAGE_LOCK_JSON, PNPM_LOCK_YAML, YARN_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_node_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let manifest = parsed_cache.get_json_value(path.join(PACKAGE_JSON))?;
    if manifest
        .as_ref()
        .is_some_and(|json| check_json_dependencies(json, dependencies))
    {
        return Ok(Some(PACKAGE_JSON.to_owned()));
    }

    // package.json is authoritative, even when it does not parse: lockfiles
    // also list transitive packages (vitest pulls in vite), so they are only
    // a fallback without a manifest
    if manifest.is_none() && !parsed_cache.is_present(path.join(PACKAGE_JSON))? {
        let found = match parsed_cache.get_json_value(path.join(PACKAGE_LOCK_JSON))? {
            Some(json) => hit(
                PACKAGE_LOCK_JSON,
                check_package_lock_dependencies(&json, dependencies),
            ),
            None => first_hit(
                parsed_cache,
                path,
                &[
                    (YARN_LOCK, check_yarn_lock_dependencies),
                    (PNPM_LOCK_YAML, check_pnpm_lock_dependencies),
                ],
                dependencies,
            )?,
        };
        if found.is_some() {
            return Ok(found);
        }
    }

    // Deno projects declare npm dependencies in deno.json(c) imports
    // (e.g. "react": "npm:react@18.2.0")
    for deno_manifest in ["deno.json", "deno.jsonc"] {
        if let Some(json) = parsed_cache.get_json_value(path.join(deno_manifest))? {
            if super::deno::imports_dependencies(&json, dependencies) {
                return Ok(Some(deno_manifest.to_owned()));
            }
        }
    }
    Ok(None)
}
