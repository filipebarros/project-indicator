use super::helpers::{check_cargo_lock_dependencies, check_toml_dependencies, first_hit};
use crate::constants::{CARGO_LOCK, CARGO_TOML};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_rust_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    if let Some(manifest) = parsed_cache.get_toml_value(path.join(CARGO_TOML))? {
        if check_toml_dependencies(&manifest, dependencies) {
            return Ok(Some(CARGO_TOML.to_owned()));
        }
        // A manifest that declares dependencies is authoritative: Cargo.lock
        // also lists transitive crates (tonic pulls in axum). A virtual
        // workspace (no [package]) declares none itself — its members do —
        // so the lockfile stays a fallback there.
        if manifest.get("package").is_some() {
            return Ok(None);
        }
    } else if parsed_cache.is_present(path.join(CARGO_TOML))? {
        // present but unparseable: still the authority, not the lockfile
        return Ok(None);
    }

    first_hit(
        parsed_cache,
        path,
        &[(CARGO_LOCK, check_cargo_lock_dependencies)],
        dependencies,
    )
}
