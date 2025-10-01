use super::helpers::check_text_dependencies;
use crate::constants::GO_MOD;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Go ecosystem dependencies
pub fn check_go_ecosystem<P: AsRef<Path>>(
    path: P,
    modules: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut found_deps = Vec::new();
    let mut evidence = Vec::new();

    if let Some(content) = parsed_cache.get_go_mod(&path)? {
        let go_deps = check_text_dependencies(&content, modules);
        if !go_deps.is_empty() {
            found_deps.extend(go_deps);
            evidence.push(GO_MOD.to_owned());
        }
    }

    Ok((found_deps, evidence))
}
