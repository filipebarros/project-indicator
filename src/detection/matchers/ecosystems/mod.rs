mod helpers;

use crate::constants::{
    BUILD_GRADLE, BUILD_GRADLE_KTS, BUILD_SBT, BUILD_SC, CARGO_LOCK, CARGO_TOML, COMPOSER_JSON,
    COMPOSER_LOCK, GEMFILE, GEMFILE_LOCK, GO_MOD, LUAROCKS_LOCK, MIX_EXS, PACKAGES_CONFIG,
    PACKAGE_JSON, PACKAGE_LOCK_JSON, PACKAGE_SWIFT, PIPFILE, PNPM_LOCK_YAML, POETRY_LOCK, POM_XML,
    PUBSPEC_LOCK, PUBSPEC_YAML, PYPROJECT_TOML, REQUIREMENTS_TXT, YARN_LOCK,
};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use helpers::{
    check_cargo_lock_dependencies, check_composer_lock_dependencies, check_csproj_dependencies,
    check_gemfile_dependencies, check_gemfile_lock_dependencies, check_go_mod_dependencies,
    check_gradle_dependencies, check_json_dependencies, check_luarocks_lock_dependencies,
    check_mill_dependencies, check_mix_dependencies, check_package_lock_dependencies,
    check_packages_config_dependencies, check_pnpm_lock_dependencies,
    check_poetry_lock_dependencies, check_pom_xml_dependencies, check_pubspec_dependencies,
    check_pubspec_lock_dependencies, check_pyproject_dependencies, check_python_text_dependencies,
    check_rockspec_dependencies, check_sbt_dependencies, check_swift_package_dependencies,
    check_toml_dependencies, check_yarn_lock_dependencies, files_with_extension, first_hit, hit,
    pubspec_declares_dependencies, rockspec_declares_dependencies, TextCheck,
};
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

pub fn check_dotnet_ecosystem(
    path: &Path,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let csprojs = files_with_extension(path, ".csproj");
    let mut probes: Vec<(&str, TextCheck)> = csprojs
        .iter()
        .map(|name| (name.as_str(), check_csproj_dependencies as TextCheck))
        .collect();
    probes.push((PACKAGES_CONFIG, check_packages_config_dependencies));
    probes.push(("Directory.Build.props", check_csproj_dependencies));

    first_hit(parsed_cache, path, &probes, packages)
}

pub fn check_elixir_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(MIX_EXS, check_mix_dependencies)],
        dependencies,
    )
}

pub fn check_go_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(GO_MOD, check_go_mod_dependencies)],
        dependencies,
    )
}

pub fn check_java_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[
            (POM_XML, check_pom_xml_dependencies),
            (BUILD_GRADLE, check_gradle_dependencies),
            (BUILD_GRADLE_KTS, check_gradle_dependencies),
        ],
        dependencies,
    )
}

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
    // also list transitive packages (vitest pulls in vite), so they only run
    // without a manifest or when the manifest declares no dependencies at
    // all (the rule every other ecosystem follows)
    let declares_dependencies = manifest.as_ref().is_some_and(|json| {
        [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ]
        .iter()
        .filter_map(|section| json.get(section).and_then(|v| v.as_object()))
        .any(|deps| !deps.is_empty())
    });
    let unparsable = manifest.is_none() && parsed_cache.is_present(path.join(PACKAGE_JSON))?;
    if !declares_dependencies && !unparsable {
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

    Ok(None)
}

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
    } else if parsed_cache.is_present(path.join(COMPOSER_JSON))? {
        // present but unparseable: still the authority, not the lockfile
        return Ok(None);
    }

    let Some(lock) = parsed_cache.get_json_value(path.join(COMPOSER_LOCK))? else {
        return Ok(None);
    };
    Ok(hit(
        COMPOSER_LOCK,
        check_composer_lock_dependencies(&lock, packages),
    ))
}

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

pub fn check_ruby_ecosystem(
    path: &Path,
    gems: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    if let Some(file) = first_hit(
        parsed_cache,
        path,
        &[(GEMFILE, check_gemfile_dependencies)],
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

pub fn check_scala_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[
            (BUILD_SBT, check_sbt_dependencies),
            (BUILD_SC, check_mill_dependencies),
        ],
        dependencies,
    )
}

pub fn check_swift_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    first_hit(
        parsed_cache,
        path,
        &[(PACKAGE_SWIFT, check_swift_package_dependencies)],
        dependencies,
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

    #[test]
    fn test_optional_dependencies_make_package_json_authoritative(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        fs::write(
            dir.path().join("package.json"),
            r#"{"optionalDependencies":{"lodash":"4"}}"#,
        )?;
        fs::write(
            dir.path().join("package-lock.json"),
            r#"{"packages":{"node_modules/react":{"version":"18"}}}"#,
        )?;

        let found =
            check_node_ecosystem(dir.path(), &["react".to_string()], &ParsedFileCache::new())?;
        assert_eq!(found, None);
        Ok(())
    }
}
