use super::helpers::{
    check_poetry_lock_dependencies, check_pyproject_dependencies, check_python_text_dependencies,
    first_hit, TextCheck,
};
use crate::constants::{PIPFILE, POETRY_LOCK, PYPROJECT_TOML, REQUIREMENTS_TXT};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_python_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let mut pyproject_declares_deps = false;
    if let Some(manifest) = parsed_cache.get_toml_value(path.join(PYPROJECT_TOML))? {
        if check_pyproject_dependencies(&manifest, dependencies) {
            return Ok(Some(PYPROJECT_TOML.to_owned()));
        }
        // A tool-only pyproject (just [tool.ruff]) declares no dependencies
        pyproject_declares_deps = manifest.get("project").is_some()
            || manifest.get("dependency-groups").is_some()
            || manifest.get("tool").and_then(|t| t.get("poetry")).is_some();
    } else if parsed_cache.is_present(path.join(PYPROJECT_TOML))? {
        // present but unparseable: assume it declares dependencies
        pyproject_declares_deps = true;
    }

    let text_manifests: [(&str, TextCheck); 2] = [
        (REQUIREMENTS_TXT, check_python_text_dependencies),
        (PIPFILE, check_python_text_dependencies),
    ];
    if let Some(file) = first_hit(parsed_cache, path, &text_manifests, dependencies)? {
        return Ok(Some(file));
    }

    // Manifests that declare dependencies are authoritative; poetry.lock
    // lists transitive packages too, so it is only a fallback when none do.
    // (Presence checks go through the parse cache: no extra syscalls.)
    for (file, _) in &text_manifests {
        if parsed_cache.get_file_content(path.join(file))?.is_some() {
            return Ok(None);
        }
    }
    if pyproject_declares_deps {
        return Ok(None);
    }

    first_hit(
        parsed_cache,
        path,
        &[(POETRY_LOCK, check_poetry_lock_dependencies)],
        dependencies,
    )
}
