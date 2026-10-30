use super::ecosystems::*;
use crate::detection::caches::ParsedFileCache;
use crate::detection::directory::DirectoryListing;
use crate::types::{DetectionType, Ecosystem, Framework, FrameworkMatch};
use crate::Result;
use std::path::Path;

struct EcosystemBatch {
    dependencies: Vec<String>,
    framework_dependencies: std::collections::HashMap<usize, Vec<usize>>,
    hits: Option<Result<DependencyHits>>,
}

impl EcosystemBatch {
    fn new() -> Self {
        Self {
            dependencies: Vec::new(),
            framework_dependencies: std::collections::HashMap::new(),
            hits: None,
        }
    }
}

fn is_batched(ecosystem: Ecosystem) -> bool {
    matches!(
        ecosystem,
        Ecosystem::Npm | Ecosystem::Pypi | Ecosystem::Cargo
    )
}

pub(crate) fn detect_frameworks<'a>(
    path: &Path,
    frameworks: impl IntoIterator<Item = &'a Framework>,
    active_ecosystems: &[Ecosystem],
    listing: Option<&DirectoryListing>,
    parsed_cache: &ParsedFileCache,
) -> Vec<FrameworkMatch> {
    let frameworks: Vec<&Framework> = frameworks.into_iter().collect();
    let mut batches: std::collections::HashMap<Ecosystem, EcosystemBatch> =
        std::collections::HashMap::new();
    for (framework_index, framework) in frameworks.iter().enumerate() {
        let DetectionType::Dependencies(dependencies) = &framework.detection else {
            continue;
        };
        for ecosystem in &framework.ecosystems {
            if !active_ecosystems.contains(ecosystem) || !is_batched(*ecosystem) {
                continue;
            }
            let batch = batches
                .entry(*ecosystem)
                .or_insert_with(EcosystemBatch::new);
            let indices = batch
                .framework_dependencies
                .entry(framework_index)
                .or_default();
            for dependency in dependencies {
                indices.push(batch.dependencies.len());
                batch.dependencies.push(dependency.clone());
            }
        }
    }

    frameworks
        .iter()
        .enumerate()
        .filter_map(|(framework_index, framework)| {
            let DetectionType::Dependencies(dependencies) = &framework.detection else {
                return None;
            };
            'detection: for ecosystem in &framework.ecosystems {
                if !active_ecosystems.contains(ecosystem) {
                    continue;
                }
                let found = if is_batched(*ecosystem) {
                    let Some(batch) = batches.get_mut(ecosystem) else {
                        continue;
                    };
                    let result = batch.hits.get_or_insert_with(|| match ecosystem {
                        Ecosystem::Npm => {
                            check_node_ecosystem_batch(path, &batch.dependencies, parsed_cache)
                        }
                        Ecosystem::Pypi => {
                            check_python_ecosystem_batch(path, &batch.dependencies, parsed_cache)
                        }
                        Ecosystem::Cargo => {
                            check_rust_ecosystem_batch(path, &batch.dependencies, parsed_cache)
                        }
                        _ => unreachable!("only batched ecosystems have a batch"),
                    });
                    match result {
                        Ok(hits) => batch.framework_dependencies[&framework_index]
                            .iter()
                            .filter_map(|index| hits.get(index))
                            .min_by_key(|hit| hit.precedence)
                            .map(|hit| hit.file.clone()),
                        Err(e) => {
                            // An unreadable manifest must not break the prompt
                            log::debug!("{}: skipped ({e:#})", framework.name);
                            break 'detection;
                        }
                    }
                } else {
                    match check_ecosystem(*ecosystem, path, dependencies, listing, parsed_cache) {
                        Ok(found) => found,
                        Err(e) => {
                            log::debug!("{}: skipped ({e:#})", framework.name);
                            break 'detection;
                        }
                    }
                };
                if let Some(file) = found {
                    // The dependency list is a set of alternatives, so any hit
                    // is a full match.
                    return Some(FrameworkMatch::new((*framework).clone(), 1.0, vec![file]));
                }
            }
            None
        })
        .collect()
}

fn check_ecosystem(
    ecosystem: Ecosystem,
    path: &Path,
    dependencies: &[String],
    listing: Option<&DirectoryListing>,
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    match ecosystem {
        Ecosystem::Npm => check_node_ecosystem(path, dependencies, parsed_cache),
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
        Ecosystem::Nuget => check_dotnet_ecosystem(path, dependencies, listing, parsed_cache),
        Ecosystem::Sbt => check_scala_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Pub => check_dart_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Hex => check_elixir_ecosystem(path, dependencies, parsed_cache),
        Ecosystem::Luarocks => check_lua_ecosystem(path, dependencies, listing, parsed_cache),
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
            detection: DetectionType::Dependencies(
                dependencies.iter().map(|d| d.to_string()).collect(),
            ),
            icon: None,
            color: None,
            priority: 1,
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
        Ok(detect_frameworks(
            dir.path(),
            frameworks,
            active,
            None,
            &ParsedFileCache::new(),
        ))
    }

    /// `dependency` is declared in `file` (holding `content`): exactly one
    /// full-confidence match, citing `file`
    fn assert_found_in(
        case: &str,
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
        assert_eq!(matches.len(), 1, "{case}");
        assert_eq!(matches[0].confidence, 1.0, "{case}");
        assert_eq!(matches[0].evidence, vec![file], "{case}");
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
    fn test_broken_manifests_ignore_lockfiles() -> TestResult {
        let cases = [
            (
                "package.json",
                (
                    "package-lock.json",
                    r#"{"packages":{"node_modules/vite":{"version":"5"}}}"#,
                ),
                Ecosystem::Npm,
                "vite",
            ),
            (
                "composer.json",
                (
                    "composer.lock",
                    r#"{"packages":[{"name":"laravel/framework"}]}"#,
                ),
                Ecosystem::Packagist,
                "laravel/framework",
            ),
            (
                "pyproject.toml",
                (
                    "poetry.lock",
                    "[[package]]\nname = \"django\"\nversion = \"4.2\"\n",
                ),
                Ecosystem::Pypi,
                "django",
            ),
            (
                "Cargo.toml",
                (
                    "Cargo.lock",
                    "[[package]]\nname = \"axum\"\nversion = \"0.6\"\n",
                ),
                Ecosystem::Cargo,
                "axum",
            ),
        ];

        for (manifest, lock, ecosystem, dependency) in cases {
            assert_broken_manifest_ignores_lockfile(manifest, lock, ecosystem, dependency)?;
        }
        Ok(())
    }

    #[test]
    fn test_dependency_manifest_formats() -> TestResult {
        let cases = [
            (
                "Cargo manifest",
                "Cargo.toml",
                "[dependencies]\ntokio = \"1\"\n",
                Ecosystem::Cargo,
                "tokio",
            ),
            (
                "npm manifest",
                "package.json",
                r#"{"dependencies":{"react":"18"}}"#,
                Ecosystem::Npm,
                "react",
            ),
            (
                "npm modern lockfile",
                "package-lock.json",
                r#"{"packages":{"node_modules/react":{"version":"18"}}}"#,
                Ecosystem::Npm,
                "react",
            ),
            (
                "npm v1 nested lockfile",
                "package-lock.json",
                r#"{"lockfileVersion":1,"dependencies":{"app":{"version":"1","dependencies":{"react":{"version":"18"}}}}}"#,
                Ecosystem::Npm,
                "react",
            ),
            (
                "Yarn lockfile",
                "yarn.lock",
                "react@^18.2.0:\n  version \"18.2.0\"\n",
                Ecosystem::Npm,
                "react",
            ),
            (
                "pnpm lockfile",
                "pnpm-lock.yaml",
                "dependencies:\n  react: 18.2.0\n",
                Ecosystem::Npm,
                "react",
            ),
            (
                "Composer lockfile",
                "composer.lock",
                r#"{"packages":[{"name":"laravel/framework"}]}"#,
                Ecosystem::Packagist,
                "laravel/framework",
            ),
            (
                "Gemfile lockfile",
                "Gemfile.lock",
                "GEM\n  specs:\n    rails (7.0.5)\n",
                Ecosystem::Rubygems,
                "rails",
            ),
            (
                "Poetry lockfile",
                "poetry.lock",
                "[[package]]\nname = \"django\"\nversion = \"4.2\"\n",
                Ecosystem::Pypi,
                "django",
            ),
            (
                "Cargo lockfile",
                "Cargo.lock",
                "[[package]]\nname = \"axum\"\nversion = \"0.6\"\n",
                Ecosystem::Cargo,
                "axum",
            ),
            (
                "Maven manifest",
                "pom.xml",
                "<dependency><artifactId>spring-boot-starter</artifactId></dependency>",
                Ecosystem::Maven,
                "spring-boot-starter",
            ),
            (
                "Gradle manifest",
                "build.gradle",
                "implementation 'org.springframework.boot:spring-boot-starter'",
                Ecosystem::Gradle,
                "spring-boot-starter",
            ),
            (
                ".NET project",
                "web.csproj",
                r#"<PackageReference Include="Microsoft.AspNetCore.App" Version="2.1.1" />"#,
                Ecosystem::Nuget,
                "Microsoft.AspNetCore.App",
            ),
        ];

        for (case, file, content, ecosystem, dependency) in cases {
            assert_found_in(case, file, content, ecosystem, dependency)?;
        }
        Ok(())
    }
}
