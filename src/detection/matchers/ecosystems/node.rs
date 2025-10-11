use super::helpers::{
    check_json_dependencies, check_package_lock_dependencies, check_pnpm_lock_dependencies,
    check_yarn_lock_dependencies,
};
use crate::constants::{PACKAGE_JSON, PACKAGE_LOCK_JSON, PNPM_LOCK_YAML, YARN_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Node.js ecosystem dependencies
pub fn check_node_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    let package_json_path = path.as_ref().join(PACKAGE_JSON);
    if let Some(json_value) = parsed_cache.get_json_value(&package_json_path)? {
        let package_deps = check_json_dependencies(&json_value, dependencies);
        if !package_deps.is_empty() {
            found_deps.extend(package_deps);
            evidence.push(PACKAGE_JSON.to_owned());
            return Ok((found_deps, evidence));
        }
    }

    let package_lock_path = path.as_ref().join(PACKAGE_LOCK_JSON);
    if let Some(json_value) = parsed_cache.get_json_value(&package_lock_path)? {
        let lock_deps = check_package_lock_dependencies(&json_value, dependencies);
        if !lock_deps.is_empty() {
            found_deps.extend(lock_deps);
            evidence.push(PACKAGE_LOCK_JSON.to_owned());
        }
    } else {
        let yarn_lock_path = path.as_ref().join(YARN_LOCK);
        if let Some(content) = parsed_cache.get_file_content(&yarn_lock_path)? {
            let yarn_deps = check_yarn_lock_dependencies(&content, dependencies);
            if !yarn_deps.is_empty() {
                found_deps.extend(yarn_deps);
                evidence.push(YARN_LOCK.to_owned());
            }
        } else {
            let pnpm_lock_path = path.as_ref().join(PNPM_LOCK_YAML);
            if let Some(content) = parsed_cache.get_file_content(&pnpm_lock_path)? {
                let pnpm_deps = check_pnpm_lock_dependencies(&content, dependencies);
                if !pnpm_deps.is_empty() {
                    found_deps.extend(pnpm_deps);
                    evidence.push(PNPM_LOCK_YAML.to_owned());
                }
            }
        }
    }

    Ok((found_deps, evidence))
}
