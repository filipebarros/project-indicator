use crate::constants::*;
use crate::detection::caches::parsed_file::read_text;
use crate::detection::scanner::scanning_engine::EXTREME_SIZE_THRESHOLD;
use crate::patterns::matches_pattern;
use crate::performance::FileSystemCache;
use crate::types::{DetectionConfig, Indicator};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Directory markers that satisfy `require_vcs_root`
const VCS_MARKERS: [&str; 3] = [DOT_GIT, ".hg", ".svn"];

/// Sorted entry names of a directory (names only — no per-entry stat; callers
/// check candidates with `is_file()`). Sorted for a stable choice among
/// several wildcard matches and for binary search. `None` when the directory
/// has more entries than the scanner would walk: listing it at every level
/// of the upward traversal would cost more than any marker is worth.
fn list_names(path: &Path) -> Option<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(path).ok()?.filter_map(|e| e.ok()) {
        if names.len() >= EXTREME_SIZE_THRESHOLD {
            return None;
        }
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Some(names)
}

/// Windows system directories, matched case-insensitively with any verbatim
/// `\\?\` prefix (which `canonicalize` adds) stripped
fn is_windows_system_dir(path: &str) -> bool {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    [
        r"C:\Windows",
        r"C:\Program Files",
        r"C:\Program Files (x86)",
        r"C:\Users",
    ]
    .iter()
    .any(|dir| path.eq_ignore_ascii_case(dir))
}

#[derive(Debug, Clone)]
pub struct FoundRootIndicator {
    pub pattern: String,
    pub certainty: f32,
    pub indicator: Arc<Indicator>,
    pub specificity: u8,
}

pub struct RootIndicatorEngine {
    indicators: Vec<Arc<Indicator>>,
    config: DetectionConfig,
    cached_home_dir: Option<PathBuf>,
}

impl RootIndicatorEngine {
    pub fn from_parts(mut indicators: Vec<Arc<Indicator>>, config: DetectionConfig) -> Self {
        // Checked in priority order at every ancestor level: sort once here
        indicators.sort_by_key(|indicator| indicator.priority);
        Self {
            indicators,
            config,
            cached_home_dir: std::env::home_dir().map(|h| h.canonicalize().unwrap_or(h)),
        }
    }

    /// Walk up from `start_path` to the nearest directory with a root
    /// indicator, returning that directory and the indicator.
    ///
    /// With `require_vcs_root`, an indicator root is accepted only when a
    /// VCS marker (.git/.hg/.svn) exists in the same directory or any
    /// ancestor within the traversal bound (monorepo subprojects count).
    pub fn find_project_root(
        &self,
        start_path: &Path,
        file_cache: &FileSystemCache,
    ) -> Option<(PathBuf, FoundRootIndicator)> {
        if self.is_boundary_directory(start_path) {
            log::debug!(
                "Starting path {} is a boundary directory, skipping upward traversal",
                start_path.display()
            );
            return None;
        }

        let mut current = start_path.to_path_buf();
        let mut levels = 0;
        // A root found below here that still awaits a VCS marker above it
        let mut vcs_pending: Option<(PathBuf, FoundRootIndicator)> = None;

        // `levels` counts ancestors above the start dir: N means N levels up
        while levels <= self.config.max_upward_traversal {
            if vcs_pending.is_some() {
                // The deepest indicator root wins; higher levels are only
                // consulted for the VCS marker that validates it
                if self.has_vcs_marker(&current, file_cache) {
                    return vcs_pending.take();
                }
            } else if let Some(indicator) = self.check_root_indicators_at_path(&current, file_cache)
            {
                if !self.config.require_vcs_root || self.has_vcs_marker(&current, file_cache) {
                    log::debug!(
                        "Found project root at {} (pattern: {}, certainty: {:.2})",
                        current.display(),
                        indicator.pattern,
                        indicator.certainty
                    );
                    return Some((current, indicator));
                }
                vcs_pending = Some((current.clone(), indicator));
            }

            let parent = current.parent()?;
            if self.is_boundary_directory(parent) {
                break;
            }
            current = parent.to_path_buf();
            levels += 1;
        }

        log::debug!(
            "No project root found after traversing {levels} levels up from {}",
            start_path.display()
        );
        None
    }

    /// Home, any filesystem root, and per-OS system directories: never traversed or scanned
    pub fn is_boundary_directory(&self, path: &Path) -> bool {
        if self.cached_home_dir.as_deref() == Some(path) {
            return true;
        }

        // Any filesystem root: "/" on Unix, drive roots (C:\, D:\) on Windows
        if path.parent().is_none() {
            return true;
        }

        let path_str = path.to_string_lossy();

        if is_windows_system_dir(&path_str) {
            return true;
        }

        matches!(
            path_str.as_ref(),
            "/Users" | "/home" | "/root" | "/System" | "/Library" | "/Applications"
        )
    }

    fn has_vcs_marker(&self, path: &Path, file_cache: &FileSystemCache) -> bool {
        VCS_MARKERS
            .iter()
            .any(|marker| file_cache.exists(&path.join(marker)))
    }

    /// Check for indicator root files at a specific path. VCS markers are
    /// never root candidates themselves; `find_project_root` consults them
    /// separately for the require_vcs_root gate.
    fn check_root_indicators_at_path(
        &self,
        path: &Path,
        file_cache: &FileSystemCache,
    ) -> Option<FoundRootIndicator> {
        let mut found_indicators = Vec::new();

        // package.json is read and parsed at most once per directory, however
        // many indicators (TypeScript, JavaScript) ask about it
        let ts_deps = OnceCell::new();

        // Wildcard patterns (*.csproj, *.kt) can't be checked with a literal
        // stat; they need the directory listing, built at most once per level
        let mut listing: Option<Option<Vec<String>>> = None;

        for indicator in &self.indicators {
            for root_indicator in &indicator.root_indicators {
                // The concrete filename that satisfied the pattern — for
                // literals it is the pattern itself
                let matched_name = if root_indicator.pattern.contains('*') {
                    listing
                        .get_or_insert_with(|| list_names(path))
                        .iter()
                        .flatten()
                        .find(|name| {
                            matches_pattern(name, &root_indicator.pattern)
                                && path.join(name).is_file()
                        })
                        .cloned()
                } else {
                    let pattern = &root_indicator.pattern;
                    // A stat is enough to rule a marker out. A hit is
                    // confirmed against the real directory entry names:
                    // case-insensitive filesystems also "find" `cargo.toml`
                    // for `Cargo.toml`, which the scanner then never matches.
                    // Patterns with a path separator stay stat-only.
                    (file_cache.exists(&path.join(pattern))
                        && (pattern.contains('/')
                            || listing
                                .get_or_insert_with(|| list_names(path))
                                .as_ref()
                                .is_none_or(|names| names.binary_search(pattern).is_ok())))
                    .then(|| pattern.clone())
                };

                if let Some(matched_name) = matched_name {
                    let indicator_path = path.join(&matched_name);
                    let certainty = self.calculate_indicator_certainty(
                        &indicator_path,
                        root_indicator,
                        indicator,
                        &ts_deps,
                    );

                    if certainty > 0.2 {
                        let specificity = self.get_pattern_specificity(&root_indicator.pattern);

                        // Report the concrete file: evidence and the cache's
                        // mtime-invalidation set need a real path
                        let found_root_indicator = FoundRootIndicator {
                            pattern: matched_name,
                            certainty,
                            indicator: indicator.clone(),
                            specificity,
                        };

                        // Strong, specific match: skip the remaining indicators
                        if certainty > 0.9 && specificity >= 8 {
                            log::trace!(
                                "Found strong root indicator '{}' (certainty: {:.2}, specificity: {}), early exit",
                                root_indicator.pattern,
                                certainty,
                                specificity
                            );
                            return Some(found_root_indicator);
                        }

                        found_indicators.push(found_root_indicator);
                    }
                }
            }
        }

        self.resolve_indicator_conflicts(&found_indicators).cloned()
    }

    fn calculate_indicator_certainty(
        &self,
        file_path: &Path,
        root_indicator: &crate::types::RootIndicator,
        indicator: &Arc<Indicator>,
        ts_deps: &OnceCell<bool>,
    ) -> f32 {
        let has_typescript =
            || *ts_deps.get_or_init(|| self.has_typescript_dependencies(file_path));
        match root_indicator.pattern.as_str() {
            CARGO_TOML if self.is_valid_cargo_toml(file_path) => 0.95,
            CARGO_TOML => 0.1,
            GO_MOD => 0.95,
            PYPROJECT_TOML if self.is_valid_python_project(file_path) => 0.90,
            PYPROJECT_TOML => 0.1,
            TSCONFIG_JSON => 0.90,
            PACKAGE_JSON => match indicator.name.as_str() {
                "TypeScript" if has_typescript() => 0.85,
                "TypeScript" => 0.1,
                "JavaScript" if has_typescript() => 0.1,
                "JavaScript" => 0.70,
                _ => 0.0,
            },
            // Data-driven fallback: the indicator's declared weight scaled
            // by its context class base priority (RuntimeRoot, ToolchainRoot, …),
            // so patterns without a hardcoded validator (deno.json, flake.nix,
            // main.tf, …) still participate in root discovery
            _ => root_indicator.weight * root_indicator.context.base_priority(),
        }
    }

    fn resolve_indicator_conflicts<'a>(
        &self,
        indicators: &'a [FoundRootIndicator],
    ) -> Option<&'a FoundRootIndicator> {
        indicators.iter().max_by(|a, b| {
            a.certainty
                .partial_cmp(&b.certainty)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.specificity.cmp(&b.specificity))
                // lower priority number wins, as everywhere else
                .then(b.indicator.priority.cmp(&a.indicator.priority))
        })
    }

    fn get_pattern_specificity(&self, pattern: &str) -> u8 {
        match pattern {
            CARGO_TOML | GO_MOD | TSCONFIG_JSON => 10,
            PYPROJECT_TOML => 8,
            PACKAGE_JSON => 5,
            _ => 1,
        }
    }

    fn is_valid_cargo_toml(&self, path: &Path) -> bool {
        read_text(path).is_ok_and(|c| c.contains("[package]") || c.contains("[workspace]"))
    }

    fn is_valid_python_project(&self, path: &Path) -> bool {
        // [project] is the PEP 621 table: a pyproject that declares a
        // project is Python even without a build-system section
        read_text(path).is_ok_and(|c| {
            c.contains("[project]")
                || c.contains("[tool.poetry]")
                || c.contains("[build-system]")
                || c.contains("requires-python")
        })
    }

    /// True when package.json declares TypeScript tooling as a dependency
    /// (`typescript`, `ts-node`, or any `@types/*`) in any dependency section.
    /// Reads the parsed keys, not the raw text, so a prose mention or an ESM
    /// `"type": "module"` marker can't count.
    fn has_typescript_dependencies(&self, path: &Path) -> bool {
        let Ok(content) = read_text(path) else {
            return false;
        };
        // Cheap pre-check: a matching KEY implies its text appears, so
        // plain-JS manifests skip the JSON parse entirely
        if !["typescript", "ts-node", "@types/"]
            .iter()
            .any(|needle| content.contains(needle))
        {
            return false;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            return false;
        };
        [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ]
        .iter()
        .filter_map(|section| json.get(section).and_then(|v| v.as_object()))
        .flat_map(|deps| deps.keys())
        .any(|k| k == "typescript" || k == "ts-node" || k.starts_with("@types/"))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_windows_system_dir, FileSystemCache};
    use crate::detection::root_indicators::RootIndicatorEngine;
    use crate::types::{DetectionConfig, Indicator};
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::TempDir;

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator_with_indicators;

    #[test]
    fn test_find_project_root_success() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(&subdir, &file_cache);

        assert!(result.is_some());
        let (root_path, indicator) = result.ok_or("Expected Some but got None")?;
        assert_eq!(indicator.pattern, "Cargo.toml");
        assert_eq!(root_path, temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_find_project_root_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(&subdir, &file_cache);

        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_find_project_root_max_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            max_upward_traversal: 1,
            ..Default::default()
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("deep").join("nested").join("path");
        fs::create_dir_all(&subdir)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(&subdir, &file_cache);

        // Should not find it due to max traversal limit
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_required() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(temp_dir.path(), &file_cache);

        assert!(
            result.is_none(),
            "require_vcs_root must reject an indicator root without .git, got: {:?}",
            result.map(|(path, found)| (path, found.pattern))
        );
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_present() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        fs::create_dir(temp_dir.path().join(".git"))?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(temp_dir.path(), &file_cache);

        // Should find it because VCS root is present
        assert!(result.is_some());
        Ok(())
    }

    #[test]
    fn test_find_project_root_wildcard_root_indicator() -> Result<(), Box<dyn std::error::Error>> {
        // C#/Java/Kotlin declare wildcard root indicators (*.csproj, *.kt);
        // upward traversal must match them against real filenames, not stat
        // a literal file named "*.csproj"
        let indicators = vec![create_test_indicator_with_indicators(
            "C#",
            vec![("*.csproj", 0.95)],
        )];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );

        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("MyApp.csproj"), "<Project/>")?;
        let subdir = temp_dir.path().join("src");
        fs::create_dir(&subdir)?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(&subdir, &file_cache);

        let (root_path, found) = match result {
            Some(r) => r,
            None => panic!("wildcard root indicator must fire during upward traversal"),
        };
        assert_eq!(root_path, temp_dir.path());
        assert_eq!(
            found.pattern, "MyApp.csproj",
            "the concrete matched file must be reported, not the glob"
        );
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_at_ancestor() -> Result<(), Box<dyn std::error::Error>> {
        // Monorepo layout: .git at the repo root, manifest in a subproject.
        // require_vcs_root accepts an indicator root when a VCS marker exists
        // at that directory OR any ancestor within the traversal bound.
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::create_dir(temp_dir.path().join(".git"))?;
        let proj = temp_dir.path().join("projA");
        fs::create_dir(&proj)?;
        fs::write(proj.join("Cargo.toml"), "[package]\nname = \"test\"")?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(&proj, &file_cache);

        let (root_path, found) = match result {
            Some(r) => r,
            None => panic!("manifest under a VCS ancestor must satisfy require_vcs_root"),
        };
        assert_eq!(root_path, proj, "project root is the manifest's directory");
        assert_eq!(found.pattern, "Cargo.toml");
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_accepts_non_git_vcs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // The flag is require_vcs_root, not require_git_root: Mercurial and
        // SVN checkouts must satisfy it too.
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        fs::create_dir(temp_dir.path().join(".hg"))?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(temp_dir.path(), &file_cache);

        assert!(
            result.is_some(),
            "an .hg checkout must satisfy require_vcs_root"
        );
        Ok(())
    }

    #[test]
    fn test_is_boundary_directory() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );

        if let Some(home) = std::env::home_dir() {
            // scan paths are canonical (`\\?\C:\…` on Windows), so is the cached home
            assert!(engine.is_boundary_directory(&home.canonicalize().unwrap_or(home)));
        }

        #[cfg(windows)]
        {
            assert!(engine.is_boundary_directory(Path::new(r"C:\")));
            assert!(engine.is_boundary_directory(Path::new(r"C:\Windows")));
            assert!(engine.is_boundary_directory(Path::new(r"C:\Users")));
            assert!(!engine.is_boundary_directory(Path::new(r"C:\code")));
        }

        assert!(engine.is_boundary_directory(Path::new("/")));

        let temp_dir = TempDir::new()?;
        assert!(!engine.is_boundary_directory(temp_dir.path()));

        Ok(())
    }

    #[test]
    fn test_specificity_resolution_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![
            create_test_indicator_with_indicators("Rust", vec![("Cargo.toml", 0.95)]),
            create_test_indicator_with_indicators("TypeScript", vec![("tsconfig.json", 0.90)]),
        ];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        fs::write(temp_dir.path().join("tsconfig.json"), "{}")?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(temp_dir.path(), &file_cache);

        assert!(result.is_some());
        let (_, found) = result.ok_or("Expected Some but got None")?;
        // Should prefer Rust due to higher weight (0.95 vs 0.90)
        assert_eq!(found.indicator.name, "Rust");
        Ok(())
    }

    /// Patterns without a hardcoded validator must still drive root
    /// discovery via their declared weight × context base priority.
    #[test]
    fn test_data_driven_certainty_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = create_test_indicator_with_indicators("Deno", vec![("deno.json", 0.95)]);
        let languages = vec![indicator];
        let config = DetectionConfig::default();
        let engine =
            RootIndicatorEngine::from_parts(languages.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("deno.json"), "{}")?;

        let file_cache = FileSystemCache::default();
        let result = engine.find_project_root(temp_dir.path(), &file_cache);

        let (_, found) = result.ok_or("expected root discovery on deno.json")?;
        assert_eq!(found.indicator.name, "Deno");
        Ok(())
    }

    fn engine_for(indicators: Vec<Indicator>, config: DetectionConfig) -> RootIndicatorEngine {
        RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config)
    }

    fn engine_with_root(pattern: &str) -> RootIndicatorEngine {
        engine_for(
            vec![create_test_indicator_with_indicators(
                "Test",
                vec![(pattern, 0.95)],
            )],
            DetectionConfig::default(),
        )
    }

    fn directory_with_many_files(marker: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        for i in 0..600 {
            fs::write(dir.path().join(format!("f{i}.txt")), "")?;
        }
        fs::write(dir.path().join(marker), "[package]\nname = \"x\"")?;
        Ok(dir)
    }

    #[test]
    fn test_literal_marker_is_found_in_a_huge_directory() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = directory_with_many_files("Cargo.toml")?;
        let found = engine_with_root("Cargo.toml")
            .find_project_root(dir.path(), &FileSystemCache::default());
        assert!(found.is_some());
        Ok(())
    }

    fn vcs_engine(max_upward_traversal: usize) -> RootIndicatorEngine {
        engine_for(
            vec![create_test_indicator_with_indicators(
                "Rust",
                vec![("Cargo.toml", 0.95)],
            )],
            DetectionConfig {
                max_upward_traversal,
                require_vcs_root: true,
                ..DetectionConfig::default()
            },
        )
    }

    /// Cargo.toml in `c`, the only .git two levels above it
    fn project_with_distant_git(
    ) -> Result<(TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let start = dir.path().join("a/b/c");
        fs::create_dir_all(&start)?;
        fs::create_dir(dir.path().join("a/.git"))?;
        fs::write(start.join("Cargo.toml"), "[package]\nname = \"x\"")?;
        Ok((dir, start))
    }

    #[test]
    fn test_vcs_marker_beyond_the_traversal_bound_rejects_the_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, start) = project_with_distant_git()?;
        let found = vcs_engine(1).find_project_root(&start, &FileSystemCache::default());
        assert!(found.is_none());
        Ok(())
    }

    #[test]
    fn test_vcs_marker_within_the_traversal_bound_accepts_the_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, start) = project_with_distant_git()?;
        let found = vcs_engine(2).find_project_root(&start, &FileSystemCache::default());
        assert_eq!(found.map(|(root, _)| root), Some(start));
        Ok(())
    }

    /// Listing a huge directory at every level costs milliseconds: past the
    /// scanner's own entry limit, wildcard markers are not looked for
    #[test]
    fn test_wildcard_marker_is_not_listed_for_in_a_huge_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = directory_with_many_files("App.csproj")?;
        let found =
            engine_with_root("*.csproj").find_project_root(dir.path(), &FileSystemCache::default());
        assert!(found.is_none());
        Ok(())
    }

    #[test]
    fn test_wildcard_marker_is_found_in_a_normal_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("App.csproj"), "")?;
        let found =
            engine_with_root("*.csproj").find_project_root(dir.path(), &FileSystemCache::default());
        assert!(found.is_some());
        Ok(())
    }

    /// On a case-insensitive filesystem (macOS, Windows) a literal stat finds
    /// `cargo.toml` for the pattern `Cargo.toml`, but the scanner matches
    /// names case-sensitively: the two must agree.
    #[test]
    fn test_root_marker_with_different_case_is_not_a_root() -> Result<(), Box<dyn std::error::Error>>
    {
        let indicators = vec![create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::from_parts(
            indicators.into_iter().map(Arc::new).collect(),
            DetectionConfig::default(),
        );
        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("cargo.toml"),
            "[package]\nname = \"x\"",
        )?;

        let file_cache = FileSystemCache::default();
        assert!(engine
            .find_project_root(temp_dir.path(), &file_cache)
            .is_none());
        Ok(())
    }

    #[test]
    fn test_utf16_root_manifests_are_validated() -> Result<(), Box<dyn std::error::Error>> {
        let engine = engine_for(vec![], DetectionConfig::default());
        let dir = TempDir::new()?;
        let utf16 = |text: &str| -> Vec<u8> {
            let mut bytes = vec![0xFF, 0xFE];
            for unit in text.encode_utf16() {
                bytes.extend(unit.to_le_bytes());
            }
            bytes
        };
        let cargo = dir.path().join("Cargo.toml");
        fs::write(&cargo, utf16("[package]\nname = \"x\"\n"))?;
        assert!(engine.is_valid_cargo_toml(&cargo));
        let pyproject = dir.path().join("pyproject.toml");
        fs::write(&pyproject, utf16("[project]\nname = \"x\"\n"))?;
        assert!(engine.is_valid_python_project(&pyproject));
        Ok(())
    }

    #[test]
    fn test_windows_system_dirs_match_canonical_and_plain_forms() {
        for dir in [
            r"C:\Windows",
            r"\\?\C:\Windows",
            r"\\?\c:\users",
            r"\\?\C:\Program Files (x86)",
        ] {
            assert!(is_windows_system_dir(dir), "{dir}");
        }
        assert!(!is_windows_system_dir(r"\\?\C:\Windows\System32"));
        assert!(!is_windows_system_dir(r"\\?\C:\code"));
    }

    #[test]
    fn test_max_upward_traversal_counts_levels_above_start(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mk = |n| {
            engine_for(
                vec![create_test_indicator_with_indicators(
                    "Rust",
                    vec![("Cargo.toml", 0.95)],
                )],
                DetectionConfig {
                    max_upward_traversal: n,
                    ..Default::default()
                },
            )
        };
        let temp_dir = TempDir::new()?;
        let start = temp_dir.path().join("a").join("b");
        fs::create_dir_all(&start)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"",
        )?;
        let cache = FileSystemCache::default();

        // Root is 2 levels above `start`
        assert!(mk(2).find_project_root(&start, &cache).is_some());
        assert!(mk(1).find_project_root(&start, &cache).is_none());
        Ok(())
    }

    #[test]
    fn test_shared_root_pattern_tie_prefers_lower_priority_number(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Two indicators claim the same root file with equal certainty:
        // everywhere else in the crate the LOWER priority number wins
        let mut high = create_test_indicator_with_indicators("Late", vec![("shared.lock", 0.8)]);
        high.priority = 9;
        let mut low = create_test_indicator_with_indicators("Early", vec![("shared.lock", 0.8)]);
        low.priority = 2;
        let engine = engine_for(vec![high, low], DetectionConfig::default());

        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("shared.lock"), "")?;
        let cache = FileSystemCache::default();
        let found = engine.find_project_root(temp_dir.path(), &cache);
        assert_eq!(
            found.map(|(_, f)| f.indicator.name.clone()),
            Some("Early".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_wildcard_root_match_is_deterministic_and_files_only(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = engine_for(
            vec![create_test_indicator_with_indicators(
                "C#",
                vec![("*.csproj", 0.95)],
            )],
            DetectionConfig::default(),
        );
        let temp_dir = TempDir::new()?;
        // A DIRECTORY named like the pattern must not count; of several
        // matching files the alphabetically first is reported
        fs::create_dir(temp_dir.path().join("0dir.csproj"))?;
        fs::write(temp_dir.path().join("B.csproj"), "")?;
        fs::write(temp_dir.path().join("A.csproj"), "")?;
        let cache = FileSystemCache::default();
        let found = engine.find_project_root(temp_dir.path(), &cache);
        assert_eq!(found.map(|(_, f)| f.pattern), Some("A.csproj".to_string()));
        Ok(())
    }

    #[test]
    fn test_typescript_detected_in_utf16_package_json() -> Result<(), Box<dyn std::error::Error>> {
        let engine = engine_for(vec![], DetectionConfig::default());
        let dir = TempDir::new()?;
        let p = dir.path().join("package.json");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in r#"{"devDependencies":{"typescript":"^5"}}"#.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        fs::write(&p, bytes)?;
        assert!(engine.has_typescript_dependencies(&p));
        Ok(())
    }

    #[test]
    fn test_typescript_detected_from_dependency_keys_only() -> Result<(), Box<dyn std::error::Error>>
    {
        let engine = engine_for(vec![], DetectionConfig::default());
        let dir = TempDir::new()?;
        let check = |json: &str| -> Result<bool, Box<dyn std::error::Error>> {
            let p = dir.path().join("package.json");
            fs::write(&p, json)?;
            Ok(engine.has_typescript_dependencies(&p))
        };

        // Real TypeScript signals, in any dependency section
        assert!(check(r#"{"devDependencies":{"typescript":"^5"}}"#)?);
        assert!(check(r#"{"dependencies":{"ts-node":"10"}}"#)?);
        assert!(check(r#"{"devDependencies":{"@types/node":"20"}}"#)?);
        assert!(check(r#"{"peerDependencies":{"typescript":">=4"}}"#)?);
        // Not TypeScript: ESM modules (any spacing) and prose mentions
        assert!(!check(
            "{\"type\": \"module\", \"dependencies\": {\"express\": \"4\"}}"
        )?);
        assert!(!check(
            "{\"type\":\"module\",\"dependencies\":{\"express\":\"4\"}}"
        )?);
        assert!(!check(
            r#"{"description":"typescript lover","dependencies":{"express":"4"}}"#
        )?);
        assert!(!check(
            r#"{"scripts":{"lint":"ts-node check"},"dependencies":{}}"#
        )?);
        // Unparsable manifest: no guess
        assert!(!check("{not json")?);
        Ok(())
    }
}
