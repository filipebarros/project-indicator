mod helpers;

use crate::constants::{
    BUILD_GRADLE, BUILD_GRADLE_KTS, BUILD_SBT, BUILD_SC, CARGO_LOCK, CARGO_TOML, COMPOSER_JSON,
    COMPOSER_LOCK, GEMFILE, GEMFILE_LOCK, GO_MOD, LUAROCKS_LOCK, MIX_EXS, PACKAGES_CONFIG,
    PACKAGE_JSON, PACKAGE_LOCK_JSON, PACKAGE_SWIFT, PIPFILE, PNPM_LOCK_YAML, POETRY_LOCK, POM_XML,
    PUBSPEC_LOCK, PUBSPEC_YAML, PYPROJECT_TOML, REQUIREMENTS_TXT, YARN_LOCK,
};
use crate::detection::caches::ParsedFileCache;
use crate::detection::directory::DirectoryListing;
use crate::Result;
use helpers::{
    cargo_lock_dependency_hits, check_composer_lock_dependencies, check_csproj_dependencies,
    check_gemfile_dependencies, check_gemfile_lock_dependencies, check_go_mod_dependencies,
    check_gradle_dependencies, check_json_dependencies, check_luarocks_lock_dependencies,
    check_mill_dependencies, check_mix_dependencies, check_packages_config_dependencies,
    check_pnpm_lock_dependencies, check_poetry_lock_dependencies, check_pom_xml_dependencies,
    check_pubspec_dependencies, check_pubspec_lock_dependencies, check_rockspec_dependencies,
    check_sbt_dependencies, check_swift_package_dependencies, check_yarn_lock_dependencies,
    files_with_extension_in, first_hit, hit, json_dependency_hits, package_lock_dependency_hits,
    pubspec_declares_dependencies, pyproject_dependency_hits, python_text_dependency_hits,
    rockspec_declares_dependencies, toml_dependency_hits, TextCheck,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone)]
pub struct DependencyHit {
    pub file: String,
    pub precedence: usize,
}

pub type DependencyHits = HashMap<usize, DependencyHit>;

fn record_hits(hits: &mut DependencyHits, found: HashSet<usize>, file: &str, precedence: usize) {
    for index in found {
        hits.entry(index)
            .and_modify(|current| {
                if precedence < current.precedence {
                    current.file = file.to_owned();
                    current.precedence = precedence;
                }
            })
            .or_insert_with(|| DependencyHit {
                file: file.to_owned(),
                precedence,
            });
    }
}

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

pub(crate) fn check_dotnet_ecosystem(
    path: &Path,
    packages: &[String],
    listing: Option<&DirectoryListing>,
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let csprojs = files_with_extension_in(path, ".csproj", listing);
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

pub(crate) fn check_lua_ecosystem(
    path: &Path,
    packages: &[String],
    listing: Option<&DirectoryListing>,
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let mut rockspec_declares_deps = false;
    for rockspec in files_with_extension_in(path, ".rockspec", listing) {
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
    Ok(
        check_node_ecosystem_batch(path, dependencies, parsed_cache)?
            .into_values()
            .min_by_key(|hit| hit.precedence)
            .map(|hit| hit.file),
    )
}

pub fn check_node_ecosystem_batch(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<DependencyHits> {
    let mut hits = HashMap::new();
    let manifest = parsed_cache.get_json_value(path.join(PACKAGE_JSON))?;
    if let Some(json) = manifest.as_ref() {
        record_hits(
            &mut hits,
            json_dependency_hits(json, dependencies),
            PACKAGE_JSON,
            0,
        );
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
    if declares_dependencies || unparsable {
        return Ok(hits);
    }

    match parsed_cache.get_json_value(path.join(PACKAGE_LOCK_JSON))? {
        Some(json) => record_hits(
            &mut hits,
            package_lock_dependency_hits(&json, dependencies),
            PACKAGE_LOCK_JSON,
            1,
        ),
        None => {
            for (precedence, (file, check)) in [
                (YARN_LOCK, check_yarn_lock_dependencies as TextCheck),
                (PNPM_LOCK_YAML, check_pnpm_lock_dependencies),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(content) = parsed_cache.get_file_content(path.join(file))? {
                    let found = dependencies
                        .iter()
                        .enumerate()
                        .filter(|(_, dependency)| check(&content, std::slice::from_ref(dependency)))
                        .map(|(index, _)| index)
                        .collect();
                    record_hits(&mut hits, found, file, precedence + 1);
                }
            }
        }
    }
    Ok(hits)
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
    Ok(
        check_python_ecosystem_batch(path, dependencies, parsed_cache)?
            .into_values()
            .min_by_key(|hit| hit.precedence)
            .map(|hit| hit.file),
    )
}

pub fn check_python_ecosystem_batch(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<DependencyHits> {
    let mut hits = HashMap::new();
    let mut pyproject_declares_deps = false;
    if let Some(manifest) = parsed_cache.get_toml_value(path.join(PYPROJECT_TOML))? {
        record_hits(
            &mut hits,
            pyproject_dependency_hits(&manifest, dependencies),
            PYPROJECT_TOML,
            0,
        );
        // A tool-only pyproject (just [tool.ruff]) declares no dependencies
        pyproject_declares_deps = manifest.get("project").is_some()
            || manifest.get("dependency-groups").is_some()
            || manifest.get("tool").and_then(|t| t.get("poetry")).is_some();
    } else if parsed_cache.is_present(path.join(PYPROJECT_TOML))? {
        // present but unparseable: assume it declares dependencies
        pyproject_declares_deps = true;
    }

    let mut text_manifest_present = false;
    for (precedence, file) in [REQUIREMENTS_TXT, PIPFILE].into_iter().enumerate() {
        if let Some(content) = parsed_cache.get_file_content(path.join(file))? {
            text_manifest_present = true;
            record_hits(
                &mut hits,
                python_text_dependency_hits(&content, dependencies),
                file,
                precedence + 1,
            );
        }
    }

    // Manifests that declare dependencies are authoritative; poetry.lock
    // lists transitive packages too, so it is only a fallback when none do.
    if text_manifest_present || pyproject_declares_deps {
        return Ok(hits);
    }

    if let Some(content) = parsed_cache.get_file_content(path.join(POETRY_LOCK))? {
        let found = dependencies
            .iter()
            .enumerate()
            .filter(|(_, dependency)| {
                check_poetry_lock_dependencies(&content, std::slice::from_ref(dependency))
            })
            .map(|(index, _)| index)
            .collect();
        record_hits(&mut hits, found, POETRY_LOCK, 3);
    }
    Ok(hits)
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
    Ok(
        check_rust_ecosystem_batch(path, dependencies, parsed_cache)?
            .into_values()
            .min_by_key(|hit| hit.precedence)
            .map(|hit| hit.file),
    )
}

pub fn check_rust_ecosystem_batch(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<DependencyHits> {
    let mut hits = HashMap::new();
    let manifest = parsed_cache.get_toml_value(path.join(CARGO_TOML))?;
    if let Some(manifest) = manifest.as_ref() {
        record_hits(
            &mut hits,
            toml_dependency_hits(manifest, dependencies),
            CARGO_TOML,
            0,
        );
        // A manifest that declares dependencies is authoritative: Cargo.lock
        // also lists transitive crates (tonic pulls in axum). A virtual
        // workspace (no [package]) declares none itself — its members do —
        // so the lockfile stays a fallback there.
        if manifest.get("package").is_some() {
            return Ok(hits);
        }
    } else if parsed_cache.is_present(path.join(CARGO_TOML))? {
        // present but unparseable: still the authority, not the lockfile
        return Ok(hits);
    }

    if let Some(content) = parsed_cache.get_file_content(path.join(CARGO_LOCK))? {
        record_hits(
            &mut hits,
            cargo_lock_dependency_hits(&content, dependencies),
            CARGO_LOCK,
            1,
        );
    }
    Ok(hits)
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

        let found = check_lua_ecosystem(
            dir.path(),
            &["lapis".to_string()],
            None,
            &ParsedFileCache::new(),
        )?;
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

        let found = check_lua_ecosystem(
            dir.path(),
            &["lapis".to_string()],
            None,
            &ParsedFileCache::new(),
        )?;
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
