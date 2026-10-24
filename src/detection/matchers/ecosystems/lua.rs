use super::helpers::{
    check_luarocks_lock_dependencies, check_rockspec_dependencies, files_with_extension, first_hit,
    rockspec_declares_dependencies,
};
use crate::constants::LUAROCKS_LOCK;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_lua_ecosystem(
    path: &Path,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let mut rockspec_declares_deps = false;
    for rockspec in files_with_extension(path, ".rockspec") {
        if let Some(content) = parsed_cache.get_file_content(path.join(&rockspec))? {
            rockspec_declares_deps |= rockspec_declares_dependencies(&content);
            if check_rockspec_dependencies(&content, packages) {
                return Ok(Some(rockspec));
            }
        }
    }

    // A rockspec that declares dependencies is authoritative (the lock also
    // lists transitive packages)
    if rockspec_declares_deps {
        return Ok(None);
    }

    first_hit(
        parsed_cache,
        path,
        &[(LUAROCKS_LOCK, check_luarocks_lock_dependencies)],
        packages,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_lock_is_not_consulted_when_rockspec_declares_dependencies(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        fs::write(
            dir.path().join("demo-1.0-1.rockspec"),
            "package = \"demo\"\ndependencies = {\n  \"lua >= 5.1\",\n}\n",
        )?;
        fs::write(
            dir.path().join("luarocks.lock"),
            "return { dependencies = { [\"lapis\"] = \"1.0\" } }",
        )?;

        let found =
            check_lua_ecosystem(dir.path(), &["lapis".to_string()], &ParsedFileCache::new())?;
        assert_eq!(found, None);
        Ok(())
    }

    #[test]
    fn test_lock_is_the_fallback_when_rockspec_declares_nothing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        fs::write(
            dir.path().join("demo-1.0-1.rockspec"),
            "package = \"demo\"\n",
        )?;
        fs::write(
            dir.path().join("luarocks.lock"),
            "return { dependencies = { [\"lapis\"] = \"1.0\" } }",
        )?;

        let found =
            check_lua_ecosystem(dir.path(), &["lapis".to_string()], &ParsedFileCache::new())?;
        assert_eq!(found.as_deref(), Some("luarocks.lock"));
        Ok(())
    }
}
