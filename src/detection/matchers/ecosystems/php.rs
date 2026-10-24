use super::helpers::{check_composer_lock_dependencies, check_json_dependencies, hit};
use crate::constants::{COMPOSER_JSON, COMPOSER_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_php_ecosystem(
    path: &Path,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    if let Some(manifest) = parsed_cache.get_json_value(path.join(COMPOSER_JSON))? {
        if check_json_dependencies(&manifest, packages) {
            return Ok(Some(COMPOSER_JSON.to_owned()));
        }
        // A manifest that requires anything is authoritative; composer.lock
        // lists transitive packages too, so it is only a fallback when
        // composer.json requires nothing itself
        if manifest.get("require").is_some() || manifest.get("require-dev").is_some() {
            return Ok(None);
        }
    }

    let Some(lock) = parsed_cache.get_json_value(path.join(COMPOSER_LOCK))? else {
        return Ok(None);
    };
    Ok(hit(
        COMPOSER_LOCK,
        check_composer_lock_dependencies(&lock, packages),
    ))
}
