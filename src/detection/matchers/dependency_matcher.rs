use super::ecosystems::*;
use crate::detection::caches::ParsedFileCache;
use crate::types::{DetectionType, Ecosystem, Framework, FrameworkMatch};
use crate::Result;
use std::path::Path;

pub struct DependencyMatcher;

impl DependencyMatcher {
    pub fn detect_frameworks<'a>(
        path: &Path,
        frameworks: impl IntoIterator<Item = &'a Framework>,
        active_ecosystems: &[Ecosystem],
        parsed_cache: &ParsedFileCache,
    ) -> Vec<FrameworkMatch> {
        frameworks
            .into_iter()
            .filter_map(|framework| {
                Self::try_detect_framework(path, framework, active_ecosystems, parsed_cache)
                    .unwrap_or_else(|e| {
                        // An unreadable manifest must not break the prompt
                        log::debug!("{}: skipped ({e:#})", framework.name);
                        None
                    })
            })
            .collect()
    }

    /// Run the matcher for each of the framework's ecosystems that is also
    /// active for the detected indicator, until one produces a hit.
    fn try_detect_framework(
        path: &Path,
        framework: &Framework,
        active_ecosystems: &[Ecosystem],
        parsed_cache: &ParsedFileCache,
    ) -> Result<Option<FrameworkMatch>> {
        let DetectionType::Dependencies { dependencies } = &framework.detection else {
            return Ok(None);
        };

        for ecosystem in &framework.ecosystems {
            if !active_ecosystems.contains(ecosystem) {
                continue;
            }

            if let Some(file) = check_ecosystem(*ecosystem, path, dependencies, parsed_cache)? {
                // The dependency list is a set of alternatives (react OR
                // react-dom), so any hit is a full match
                return Ok(Some(FrameworkMatch::new(
                    framework.clone(),
                    1.0,
                    vec![file],
                )));
            }
        }

        Ok(None)
    }
}

pub fn check_ecosystem(
    ecosystem: Ecosystem,
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    match ecosystem {
        Ecosystem::Npm => check_node_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Deno => check_deno_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Pypi => check_python_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Cargo => check_rust_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Go => check_go_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Packagist => check_php_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Rubygems => check_ruby_ecosystem(path, dependencies, parsed_cache),
        // Both JVM ecosystems share manifests (pom.xml, build.gradle[.kts]);
        // the java matcher reads all of them
        Ecosystem::Maven | Ecosystem::Gradle => {
            check_java_ecosystem(path, dependencies, parsed_cache)
        }
        Ecosystem::Nuget => check_dotnet_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Sbt => check_scala_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Pub => check_dart_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Hex => check_elixir_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Luarocks => check_lua_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Swiftpm => check_swift_ecosystem(path, dependencies, parsed_cache),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Every ecosystem active, as for a polyglot indicator
    const ALL_ECOSYSTEMS: [Ecosystem; 14] = [
        Ecosystem::Npm,
        Ecosystem::Pypi,
        Ecosystem::Cargo,
        Ecosystem::Go,
        Ecosystem::Packagist,
        Ecosystem::Rubygems,
        Ecosystem::Maven,
        Ecosystem::Gradle,
        Ecosystem::Nuget,
        Ecosystem::Sbt,
        Ecosystem::Pub,
        Ecosystem::Hex,
        Ecosystem::Luarocks,
        Ecosystem::Swiftpm,
    ];

    fn framework(name: &str, ecosystem: Ecosystem, dependencies: &[&str]) -> Framework {
        Framework {
            name: name.to_string(),
            ecosystems: vec![ecosystem],
            detection: DetectionType::Dependencies {
                dependencies: dependencies.iter().map(|d| d.to_string()).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            root_indicators: vec![],
        }
    }

    fn detect_in(
        files: &[(&str, &str)],
        frameworks: &[Framework],
        active: &[Ecosystem],
    ) -> Result<Vec<FrameworkMatch>, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        for (name, content) in files {
            fs::write(dir.path().join(name), content)?;
        }
        Ok(DependencyMatcher::detect_frameworks(
            dir.path(),
            frameworks,
            active,
            &ParsedFileCache::new(),
        ))
    }

    /// `dependency` is declared in `file` (holding `content`): exactly one
    /// full-confidence match, citing `file`
    fn assert_found_in(
        file: &str,
        content: &str,
        ecosystem: Ecosystem,
        dependency: &str,
    ) -> TestResult {
        let matches = detect_in(
            &[(file, content)],
            &[framework("F", ecosystem, &[dependency])],
            &ALL_ECOSYSTEMS,
        )?;
        assert_eq!(matches.len(), 1, "{file}");
        assert_eq!(matches[0].confidence, 1.0, "{file}");
        assert_eq!(matches[0].evidence, vec![file], "{file}");
        Ok(())
    }

    #[test]
    fn test_any_listed_dependency_gives_full_confidence() -> TestResult {
        let matches = detect_in(
            &[("package.json", r#"{"dependencies":{"react":"18"}}"#)],
            &[framework("React", Ecosystem::Npm, &["react", "react-dom"])],
            &ALL_ECOSYSTEMS,
        )?;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].confidence, 1.0);
        Ok(())
    }

    #[test]
    fn test_multiple_frameworks_single_file() -> TestResult {
        let matches = detect_in(
            &[(
                "package.json",
                r#"{"dependencies":{"react":"18","vue":"3"}}"#,
            )],
            &[
                framework("React", Ecosystem::Npm, &["react"]),
                framework("Vue", Ecosystem::Npm, &["vue"]),
            ],
            &ALL_ECOSYSTEMS,
        )?;
        let names: Vec<&str> = matches.iter().map(|m| m.framework.name.as_str()).collect();
        assert_eq!(names, ["React", "Vue"]);
        Ok(())
    }

    #[test]
    fn test_framework_outside_the_active_ecosystems_is_skipped() -> TestResult {
        let matches = detect_in(
            &[("package.json", r#"{"dependencies":{"react":"18"}}"#)],
            &[framework("React", Ecosystem::Npm, &["react"])],
            &[Ecosystem::Cargo],
        )?;
        assert!(matches.is_empty());
        Ok(())
    }

    /// A manifest that exists but does not parse (mid-edit) is still the
    /// authority: the lockfile's transitive packages must not stand in for it
    fn assert_broken_manifest_ignores_lockfile(
        manifest: &str,
        lock: (&str, &str),
        ecosystem: Ecosystem,
        dependency: &str,
    ) -> TestResult {
        let matches = detect_in(
            &[(manifest, "{ not valid"), lock],
            &[framework("F", ecosystem, &[dependency])],
            &ALL_ECOSYSTEMS,
        )?;
        assert!(matches.is_empty(), "{manifest}: {matches:?}");
        Ok(())
    }

    #[test]
    fn test_broken_package_json_ignores_package_lock() -> TestResult {
        assert_broken_manifest_ignores_lockfile(
            "package.json",
            (
                "package-lock.json",
                r#"{"packages":{"node_modules/vite":{"version":"5"}}}"#,
            ),
            Ecosystem::Npm,
            "vite",
        )
    }

    #[test]
    fn test_broken_composer_json_ignores_composer_lock() -> TestResult {
        assert_broken_manifest_ignores_lockfile(
            "composer.json",
            (
                "composer.lock",
                r#"{"packages":[{"name":"laravel/framework"}]}"#,
            ),
            Ecosystem::Packagist,
            "laravel/framework",
        )
    }

    #[test]
    fn test_broken_pyproject_ignores_poetry_lock() -> TestResult {
        assert_broken_manifest_ignores_lockfile(
            "pyproject.toml",
            (
                "poetry.lock",
                "[[package]]\nname = \"django\"\nversion = \"4.2\"\n",
            ),
            Ecosystem::Pypi,
            "django",
        )
    }

    #[test]
    fn test_broken_cargo_toml_ignores_cargo_lock() -> TestResult {
        assert_broken_manifest_ignores_lockfile(
            "Cargo.toml",
            (
                "Cargo.lock",
                "[[package]]\nname = \"axum\"\nversion = \"0.6\"\n",
            ),
            Ecosystem::Cargo,
            "axum",
        )
    }

    #[test]
    fn test_cargo_toml() -> TestResult {
        assert_found_in(
            "Cargo.toml",
            "[dependencies]\ntokio = \"1\"\n",
            Ecosystem::Cargo,
            "tokio",
        )
    }

    #[test]
    fn test_package_json() -> TestResult {
        assert_found_in(
            "package.json",
            r#"{"dependencies":{"react":"18"}}"#,
            Ecosystem::Npm,
            "react",
        )
    }

    #[test]
    fn test_package_lock_json() -> TestResult {
        assert_found_in(
            "package-lock.json",
            r#"{"packages":{"node_modules/react":{"version":"18"}}}"#,
            Ecosystem::Npm,
            "react",
        )
    }

    #[test]
    fn test_yarn_lock() -> TestResult {
        assert_found_in(
            "yarn.lock",
            "react@^18.2.0:\n  version \"18.2.0\"\n",
            Ecosystem::Npm,
            "react",
        )
    }

    #[test]
    fn test_pnpm_lock_yaml() -> TestResult {
        assert_found_in(
            "pnpm-lock.yaml",
            "dependencies:\n  react: 18.2.0\n",
            Ecosystem::Npm,
            "react",
        )
    }

    #[test]
    fn test_composer_lock() -> TestResult {
        assert_found_in(
            "composer.lock",
            r#"{"packages":[{"name":"laravel/framework"}]}"#,
            Ecosystem::Packagist,
            "laravel/framework",
        )
    }

    #[test]
    fn test_gemfile_lock() -> TestResult {
        assert_found_in(
            "Gemfile.lock",
            "GEM\n  specs:\n    rails (7.0.5)\n",
            Ecosystem::Rubygems,
            "rails",
        )
    }

    #[test]
    fn test_poetry_lock() -> TestResult {
        assert_found_in(
            "poetry.lock",
            "[[package]]\nname = \"django\"\nversion = \"4.2\"\n",
            Ecosystem::Pypi,
            "django",
        )
    }

    #[test]
    fn test_cargo_lock() -> TestResult {
        assert_found_in(
            "Cargo.lock",
            "[[package]]\nname = \"axum\"\nversion = \"0.6\"\n",
            Ecosystem::Cargo,
            "axum",
        )
    }

    #[test]
    fn test_maven_pom() -> TestResult {
        assert_found_in(
            "pom.xml",
            "<dependency><artifactId>spring-boot-starter</artifactId></dependency>",
            Ecosystem::Maven,
            "spring-boot-starter",
        )
    }

    #[test]
    fn test_gradle_build_script() -> TestResult {
        assert_found_in(
            "build.gradle",
            "implementation 'org.springframework.boot:spring-boot-starter'",
            Ecosystem::Gradle,
            "spring-boot-starter",
        )
    }

    #[test]
    fn test_dotnet_csproj() -> TestResult {
        assert_found_in(
            "web.csproj",
            r#"<PackageReference Include="Microsoft.AspNetCore.App" Version="2.1.1" />"#,
            Ecosystem::Nuget,
            "Microsoft.AspNetCore.App",
        )
    }
}
