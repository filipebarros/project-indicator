use super::helpers::{check_gemfile_lock_dependencies, check_text_dependencies, first_hit};
use crate::constants::{GEMFILE, GEMFILE_LOCK};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_ruby_ecosystem(
    path: &Path,
    gems: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    if let Some(file) = first_hit(
        parsed_cache,
        path,
        &[(GEMFILE, check_text_dependencies)],
        gems,
    )? {
        return Ok(Some(file));
    }

    // A Gemfile that lists gems is authoritative; Gemfile.lock lists
    // transitive gems too. A Gemfile with no `gem` lines (just `gemspec`)
    // declares nothing itself, so the lockfile stays a fallback there.
    if parsed_cache
        .get_file_content(path.join(GEMFILE))?
        .is_some_and(|gemfile| gemfile.lines().any(|l| l.trim_start().starts_with("gem ")))
    {
        return Ok(None);
    }

    first_hit(
        parsed_cache,
        path,
        &[(GEMFILE_LOCK, check_gemfile_lock_dependencies)],
        gems,
    )
}
