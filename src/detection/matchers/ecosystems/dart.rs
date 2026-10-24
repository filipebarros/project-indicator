use super::helpers::{
    check_pubspec_dependencies, check_pubspec_lock_dependencies, first_hit,
    pubspec_declares_dependencies,
};
use crate::constants::{PUBSPEC_LOCK, PUBSPEC_YAML};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_dart_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    if let Some(file) = first_hit(
        parsed_cache,
        path,
        &[(PUBSPEC_YAML, check_pubspec_dependencies)],
        dependencies,
    )? {
        return Ok(Some(file));
    }

    // A pubspec.yaml that declares dependencies is authoritative: the lock
    // also lists transitive packages, which must not count
    if parsed_cache
        .get_file_content(path.join(PUBSPEC_YAML))?
        .is_some_and(|content| pubspec_declares_dependencies(&content))
    {
        return Ok(None);
    }

    first_hit(
        parsed_cache,
        path,
        &[(PUBSPEC_LOCK, check_pubspec_lock_dependencies)],
        dependencies,
    )
}
