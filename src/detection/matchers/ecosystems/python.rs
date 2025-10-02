use super::helpers::{
    check_poetry_lock_dependencies, check_pyproject_dependencies, check_text_dependencies,
    try_config_file_deps,
};
use crate::constants::{PIPFILE, POETRY_LOCK, PYPROJECT_TOML, REQUIREMENTS_TXT};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Python ecosystem dependencies
pub fn check_python_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(toml_value) = parsed_cache.get_pyproject_toml(&path)? {
        let pyproject_deps = check_pyproject_dependencies(&toml_value, dependencies);
        if !pyproject_deps.is_empty() {
            found_deps.extend(pyproject_deps);
            evidence.push(PYPROJECT_TOML.to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        REQUIREMENTS_TXT,
        dependencies,
        check_text_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        PIPFILE,
        dependencies,
        check_text_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if let Some(content) = parsed_cache.get_poetry_lock(&path)? {
        let poetry_deps = check_poetry_lock_dependencies(&content, dependencies);
        if !poetry_deps.is_empty() {
            found_deps.extend(poetry_deps);
            evidence.push(POETRY_LOCK.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
