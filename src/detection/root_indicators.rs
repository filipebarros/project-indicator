use crate::constants::*;
use crate::detection::caches::ParsedFileCache;
use crate::detection::directory::DirectoryListing;
use crate::detection::scanner::scanning_engine::probe_shape_matches;
use crate::patterns::matches_pattern;
use crate::performance::FileSystemCache;
use crate::types::{DetectionConfig, Indicator};
use std::cell::OnceCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Directory markers that satisfy `require_vcs_root` and trigger the
/// subproject scan (`DetectionEngine::detect`)
pub(crate) const VCS_MARKERS: [&str; 3] = [DOT_GIT, ".hg", ".svn"];

#[derive(Default)]
struct RootPatternIndex {
    literal_patterns: std::collections::HashSet<String>,
    wildcard_patterns: Vec<String>,
}

impl RootPatternIndex {
    fn new(indicators: &[Arc<Indicator>]) -> Self {
        let roots = || {
            indicators
                .iter()
                .flat_map(|indicator| &indicator.root_indicators)
        };
        Self {
            literal_patterns: roots()
                .map(|root| root.pattern.clone())
                .filter(|pattern| !pattern.contains('*') && !pattern.contains('/'))
                .collect(),
            wildcard_patterns: roots()
                .map(|root| root.pattern.clone())
                .filter(|pattern| pattern.contains('*'))
                .collect(),
        }
    }
}

#[derive(Default)]
struct RootDirectoryListing {
    literal_names: std::collections::HashSet<String>,
    wildcard_matches: std::collections::HashMap<String, String>,
}

fn root_markers_from_entries<'a>(
    entries: impl Iterator<Item = &'a std::fs::DirEntry>,
    patterns: &RootPatternIndex,
    file_cache: &FileSystemCache,
) -> RootDirectoryListing {
    let mut listing = RootDirectoryListing::default();
    for entry in entries {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let is_file = entry.file_type().is_ok_and(|file_type| {
            file_type.is_file()
                || (file_type.is_symlink()
                    && file_cache.exists(&entry.path())
                    && entry.path().is_file())
        });
        if !is_file {
            continue;
        }
        if patterns.literal_patterns.contains(name.as_str()) {
            listing.literal_names.insert(name.clone());
        }
        for pattern in patterns
            .wildcard_patterns
            .iter()
            .filter(|pattern| matches_pattern(&name, pattern))
        {
            listing
                .wildcard_matches
                .entry(pattern.clone())
                .and_modify(|current| {
                    if name < *current {
                        *current = name.clone();
                    }
                })
                .or_insert_with(|| name.clone());
        }
    }
    listing
}

fn list_root_markers(
    path: &Path,
    patterns: &RootPatternIndex,
    file_cache: &FileSystemCache,
) -> Option<RootDirectoryListing> {
    let Ok(entries) = std::fs::read_dir(path) else {
        file_cache.mark_read_failed();
        return None;
    };
    let mut valid_entries = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => valid_entries.push(entry),
            Err(_) => file_cache.mark_read_failed(),
        }
    }
    Some(root_markers_from_entries(
        valid_entries.iter(),
        patterns,
        file_cache,
    ))
}

#[derive(Debug, Clone)]
pub struct FoundRootIndicator {
    pub pattern: String,
    pub certainty: f32,
    pub indicator: Arc<Indicator>,
    /// The marker's declared context class: arbitration ranks a LanguageRoot
    /// manifest above source files, and source files above a (shared)
    /// BuildSystem file.
    pub context: crate::types::IndicatorContext,
}

pub struct RootIndicatorEngine {
    indicators: Vec<Arc<Indicator>>,
    patterns: RootPatternIndex,
    config: DetectionConfig,
    cached_home_dir: Option<PathBuf>,
}

impl RootIndicatorEngine {
    pub fn from_parts(indicators: Vec<Arc<Indicator>>, config: DetectionConfig) -> Self {
        // Checked in config order at every ancestor level: the vec order IS
        // the tie-break order
        let patterns = RootPatternIndex::new(&indicators);
        Self {
            indicators,
            patterns,
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
    #[cfg(test)]
    pub fn find_project_root(
        &self,
        start_path: &Path,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Option<(PathBuf, FoundRootIndicator)> {
        self.find_project_root_within(
            start_path,
            crate::detection::engine::MAX_UPWARD_TRAVERSAL,
            file_cache,
            parsed_cache,
        )
    }

    /// `find_project_root` looking at most `max_levels` directories above
    /// the start (0: the start directory only), whatever the config says
    #[cfg(test)]
    pub fn find_project_root_within(
        &self,
        start_path: &Path,
        max_levels: usize,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Option<(PathBuf, FoundRootIndicator)> {
        self.find_root_markers_within(start_path, max_levels, file_cache, parsed_cache)
            .map(|(path, markers)| {
                let best = Self::best_marker(&markers).clone();
                (path, best)
            })
    }

    /// The nearest directory with any qualifying marker, with EVERY
    /// qualifying marker found there: the engine decides from the full set
    /// (one indicator alone is uncontested; several are arbitrated), so the
    /// directory is listed exactly once.
    pub fn find_root_markers_within(
        &self,
        start_path: &Path,
        max_levels: usize,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Option<(PathBuf, Vec<FoundRootIndicator>)> {
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
        let mut vcs_pending: Option<(PathBuf, Vec<FoundRootIndicator>)> = None;

        // `levels` counts ancestors above the start dir: N means N levels up
        while levels <= max_levels {
            if vcs_pending.is_some() {
                // The deepest indicator root wins; higher levels are only
                // consulted for the VCS marker that validates it
                if Self::has_vcs_marker(&current, file_cache) {
                    return vcs_pending.take();
                }
            } else {
                let markers = self.qualifying_markers_at(&current, file_cache, parsed_cache);
                if !markers.is_empty() {
                    if !self.config.require_vcs_root || Self::has_vcs_marker(&current, file_cache) {
                        let best = Self::best_marker(&markers);
                        log::debug!(
                            "Found project root at {} (pattern: {}, certainty: {:.2})",
                            current.display(),
                            best.pattern,
                            best.certainty
                        );
                        return Some((current, markers));
                    }
                    vcs_pending = Some((current.clone(), markers));
                }
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
        crate::detection::repository::is_boundary_directory(path, self.cached_home_dir.as_deref())
    }

    fn has_vcs_marker(path: &Path, file_cache: &FileSystemCache) -> bool {
        VCS_MARKERS
            .iter()
            .any(|marker| file_cache.exists(&path.join(marker)))
    }

    pub(crate) fn requires_vcs_root(&self) -> bool {
        self.config.require_vcs_root
    }

    pub fn vcs_requirement_met(
        &self,
        start_path: &Path,
        max_levels: usize,
        file_cache: &FileSystemCache,
    ) -> bool {
        if !self.config.require_vcs_root {
            return true;
        }
        start_path
            .ancestors()
            .take(max_levels + 1)
            .take_while(|dir| !self.is_boundary_directory(dir))
            .any(|dir| Self::has_vcs_marker(dir, file_cache))
    }

    /// Whether the configured VCS requirement accepts `path` as a project
    /// root: the check `find_root_markers_within` applies to markers found
    /// at the start directory itself, extracted for callers that supply
    /// markers from an existing directory listing.
    pub(crate) fn root_vcs_validated(&self, path: &Path, file_cache: &FileSystemCache) -> bool {
        !self.config.require_vcs_root || Self::has_vcs_marker(path, file_cache)
    }

    /// The strongest marker of a directory: highest certainty, then config
    /// order (the collection iterates indicators in config order, so the
    /// first max wins ties). Callers pass a non-empty slice: every path here
    /// guards on `!markers.is_empty()`.
    pub fn best_marker(markers: &[FoundRootIndicator]) -> &FoundRootIndicator {
        let mut best = &markers[0];
        for marker in &markers[1..] {
            if marker.certainty.total_cmp(&best.certainty).is_gt() {
                best = marker;
            }
        }
        best
    }

    /// Every qualifying (certainty > 0.2) root marker of every indicator at
    /// `path`, content checks applied. VCS markers are never root candidates
    /// themselves; `find_root_markers_within` consults them separately for the
    /// require_vcs_root gate.
    pub fn qualifying_markers_at(
        &self,
        path: &Path,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Vec<FoundRootIndicator> {
        self.qualifying_markers(path, file_cache, parsed_cache, None, false)
    }

    pub(crate) fn qualifying_markers_in(
        &self,
        path: &Path,
        supplied_listing: &DirectoryListing,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Vec<FoundRootIndicator> {
        if supplied_listing.unreadable() {
            file_cache.mark_read_failed();
        }
        let listing =
            root_markers_from_entries(supplied_listing.entries(), &self.patterns, file_cache);
        self.qualifying_markers(path, file_cache, parsed_cache, Some(Some(listing)), true)
    }

    fn qualifying_markers(
        &self,
        path: &Path,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
        mut listing: Option<Option<RootDirectoryListing>>,
        supplied: bool,
    ) -> Vec<FoundRootIndicator> {
        let mut found_indicators = Vec::new();

        // package.json is read and parsed at most once per directory, however
        // many indicators (TypeScript, JavaScript) ask about it
        let ts_deps = OnceCell::new();

        for indicator in &self.indicators {
            for root_indicator in &indicator.root_indicators {
                // The concrete filename that satisfied the pattern — for
                // literals it is the pattern itself
                let matched_name = if root_indicator.pattern.contains('*') {
                    listing
                        .get_or_insert_with(|| list_root_markers(path, &self.patterns, file_cache))
                        .as_ref()
                        .and_then(|listing| {
                            listing
                                .wildcard_matches
                                .get(&root_indicator.pattern)
                                .cloned()
                        })
                } else {
                    let pattern = &root_indicator.pattern;
                    // A stat is enough to rule a marker out during lazy upward
                    // discovery. A hit is confirmed against the real directory
                    // entry names, which also keeps case-sensitive matching.
                    let candidate = path.join(pattern);
                    let present = if pattern.contains('/') {
                        // creating a missing path marker must be noticed. The
                        // last component tells file from directory probes: a
                        // 1-byte file named src/main/java is no source tree,
                        // and a directory named lib/main.dart is no manifest
                        file_cache.exists_watched(&candidate)
                            && match probe_shape_matches(&candidate, pattern) {
                                Ok(matches) => matches,
                                Err(_) => {
                                    file_cache.mark_read_failed();
                                    false
                                }
                            }
                    } else if supplied {
                        true
                    } else {
                        // A directory named like a manifest (a folder called
                        // "Cargo.toml") is no marker; only path probes
                        // (src/main/java) are directories
                        file_cache.exists(&candidate) && candidate.is_file()
                    };
                    (present
                        && (pattern.contains('/')
                            || listing
                                .get_or_insert_with(|| {
                                    list_root_markers(path, &self.patterns, file_cache)
                                })
                                .as_ref()
                                .is_none_or(|listing| listing.literal_names.contains(pattern))))
                    .then(|| pattern.clone())
                };

                if let Some(matched_name) = matched_name {
                    let indicator_path = path.join(&matched_name);
                    let certainty = Self::calculate_indicator_certainty(
                        &indicator_path,
                        root_indicator,
                        indicator,
                        &ts_deps,
                        parsed_cache,
                    );

                    if certainty > 0.2 {
                        // Report the concrete file: evidence and the cache's
                        // mtime-invalidation set need a real path
                        found_indicators.push(FoundRootIndicator {
                            pattern: matched_name,
                            certainty,
                            indicator: indicator.clone(),
                            context: root_indicator.context.clone(),
                        });
                    }
                }
            }
        }

        found_indicators
    }

    /// Every marker is scored the same way: its declared weight scaled by its
    /// context class. The two shared manifests that name more than one
    /// language are first checked against their content, and score 0 when
    /// they belong to another language.
    fn calculate_indicator_certainty(
        file_path: &Path,
        root_indicator: &crate::types::RootIndicator,
        indicator: &Indicator,
        ts_deps: &OnceCell<bool>,
        parsed_cache: &ParsedFileCache,
    ) -> f32 {
        let belongs_to_indicator = match root_indicator.pattern.as_str() {
            // A pyproject.toml without a project table is tool config
            PYPROJECT_TOML => Self::is_valid_python_project(file_path, parsed_cache),
            // A tsconfig.json beside a plainly-JS package.json (dependencies
            // declared, no typescript/ts-node) is editor configuration
            // (checkJs), not a TypeScript project. With no package.json at
            // all it still marks TypeScript.
            TSCONFIG_JSON => {
                let sibling = file_path
                    .parent()
                    .map(|dir| dir.join(PACKAGE_JSON))
                    .filter(|p| parsed_cache.get_json_value(p).ok().flatten().is_some());
                match sibling {
                    Some(package_json) => *ts_deps.get_or_init(|| {
                        Self::has_typescript_dependencies(&package_json, parsed_cache)
                    }),
                    None => true,
                }
            }
            // DESCRIPTION is a generic name; only the R-package form counts
            "DESCRIPTION" => Self::is_r_package_description(file_path, parsed_cache),
            // A tsconfig.json marker makes an indicator the TypeScript flavour
            // of a shared package.json; anything else claiming it is plain JS
            PACKAGE_JSON => {
                let typescript_flavour = indicator
                    .root_indicators
                    .iter()
                    .any(|r| r.pattern == TSCONFIG_JSON);
                let has_typescript = *ts_deps
                    .get_or_init(|| Self::has_typescript_dependencies(file_path, parsed_cache));
                typescript_flavour == has_typescript
            }
            _ => true,
        };
        if belongs_to_indicator {
            root_indicator.weight * root_indicator.context.base_priority()
        } else {
            0.0
        }
    }

    /// Candidate manifests are read through the run's `ParsedFileCache`, so
    /// the persistent cache sees them as inputs even when they are rejected.
    fn text_of(path: &Path, parsed_cache: &ParsedFileCache) -> Option<Arc<String>> {
        parsed_cache.get_file_content(path).ok().flatten()
    }

    fn is_valid_python_project(path: &Path, parsed_cache: &ParsedFileCache) -> bool {
        // [project] is the PEP 621 table: a pyproject that declares a
        // project is Python even without a build-system section. Parsed
        // keys, not raw text: a comment mentioning requires-python inside
        // [tool.black] must not count.
        parsed_cache
            .get_toml_value(path)
            .ok()
            .flatten()
            .is_some_and(|toml| {
                toml.get("project").is_some()
                    || toml.get("build-system").is_some()
                    || toml
                        .get("tool")
                        .and_then(|tool| tool.get("poetry"))
                        .is_some()
            })
    }

    /// An R package's DESCRIPTION is a DCF file with a `Package:` field; a
    /// prose file that happens to be called DESCRIPTION is not an R project.
    fn is_r_package_description(path: &Path, parsed_cache: &ParsedFileCache) -> bool {
        Self::text_of(path, parsed_cache)
            .is_some_and(|text| text.lines().any(|line| line.starts_with("Package:")))
    }

    /// True when package.json declares the TypeScript compiler or runner
    /// (`typescript` or `ts-node`) in any dependency section. `@types/*`
    /// alone does NOT count: plain JavaScript projects add `@types/node` for
    /// editor hints without compiling TypeScript. Reads the parsed keys, not
    /// the raw text, so a prose mention or an ESM `"type": "module"` marker
    /// can't count.
    fn has_typescript_dependencies(path: &Path, parsed_cache: &ParsedFileCache) -> bool {
        let Some(json) = parsed_cache.get_json_value(path).ok().flatten() else {
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
        .any(|k| k == "typescript" || k == "ts-node")
    }
}

#[cfg(test)]
mod tests {
    use super::{FileSystemCache, FoundRootIndicator, ParsedFileCache};
    use crate::detection::repository::is_windows_system_dir;
    use crate::detection::root_indicators::RootIndicatorEngine;
    use crate::types::{DetectionConfig, Indicator};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tempfile::TempDir;

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator_with_indicators;

    #[test]
    fn test_qualifying_markers_in_uses_the_supplied_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());
        let dir = TempDir::new()?;
        let marker = dir.path().join("Cargo.toml");
        fs::write(&marker, "[package]\nname = \"test\"")?;
        let listing = crate::detection::directory::DirectoryListing::read(dir.path());
        fs::remove_file(marker)?;

        let markers = engine.qualifying_markers_in(
            dir.path(),
            &listing,
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );

        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].pattern, "Cargo.toml");
        assert_eq!(markers[0].indicator.name, "Rust");
        Ok(())
    }

    #[test]
    fn test_find_project_root_success() -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let result = find(&engine, &subdir);

        assert!(result.is_some());
        let (root_path, indicator) = result.ok_or("Expected Some but got None")?;
        assert_eq!(indicator.pattern, "Cargo.toml");
        assert_eq!(root_path, temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_the_upward_search_can_be_limited_below_the_configured_bound(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());
        let temp_dir = TempDir::new()?;
        let sub = temp_dir.path().join("sub");
        fs::create_dir(&sub)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"",
        )?;
        let within = |levels| {
            engine.find_project_root_within(
                &sub,
                levels,
                &FileSystemCache::default(),
                &ParsedFileCache::new(),
            )
        };

        assert!(within(0).is_none(), "0 levels: the start directory only");
        assert!(within(1).is_some(), "1 level reaches the parent");
        Ok(())
    }

    #[test]
    fn test_find_project_root_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;

        let result = find(&engine, &subdir);

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
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = FileSystemCache::default();
        let result =
            engine.find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new());

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
        let result =
            engine.find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new());

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

        let result = find(&engine, &subdir);

        let (root_path, found) =
            result.ok_or("wildcard root indicator must fire during upward traversal")?;
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
        };
        let engine =
            RootIndicatorEngine::from_parts(indicators.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::create_dir(temp_dir.path().join(".git"))?;
        let proj = temp_dir.path().join("projA");
        fs::create_dir(&proj)?;
        fs::write(proj.join("Cargo.toml"), "[package]\nname = \"test\"")?;

        let result = find(&engine, &proj);

        let (root_path, found) =
            result.ok_or("manifest under a VCS ancestor must satisfy require_vcs_root")?;
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
        let result =
            engine.find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new());

        assert!(
            result.is_some(),
            "an .hg checkout must satisfy require_vcs_root"
        );
        Ok(())
    }

    #[test]
    fn test_is_boundary_directory() -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());

        if let Some(home) = std::env::home_dir() {
            assert!(engine.is_boundary_directory(&home));
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
    fn test_the_marker_with_the_higher_certainty_wins() -> Result<(), Box<dyn std::error::Error>> {
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
        let result =
            engine.find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new());

        assert!(result.is_some());
        let (_, found) = result.ok_or("Expected Some but got None")?;
        assert_eq!(found.indicator.name, "Rust");
        Ok(())
    }

    /// Patterns without a hardcoded validator must still drive root
    /// discovery via their declared weight × context base priority.
    #[test]
    fn test_data_driven_certainty_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = create_test_indicator_with_indicators("Lang", vec![("lang.toml", 0.95)]);
        let languages = vec![indicator];
        let config = DetectionConfig::default();
        let engine =
            RootIndicatorEngine::from_parts(languages.into_iter().map(Arc::new).collect(), config);

        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("lang.toml"), "")?;

        let file_cache = FileSystemCache::default();
        let result =
            engine.find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new());

        let (_, found) = result.ok_or("expected root discovery on lang.toml")?;
        assert_eq!(found.indicator.name, "Lang");
        Ok(())
    }

    fn rust_engine(config: DetectionConfig) -> RootIndicatorEngine {
        engine_for(
            vec![create_test_indicator_with_indicators(
                "Rust",
                vec![("Cargo.toml", 0.95)],
            )],
            config,
        )
    }

    fn find(engine: &RootIndicatorEngine, start: &Path) -> Option<(PathBuf, FoundRootIndicator)> {
        engine.find_project_root(start, &FileSystemCache::default(), &ParsedFileCache::new())
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
        let found = engine_with_root("Cargo.toml").find_project_root(
            dir.path(),
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        assert!(found.is_some());
        Ok(())
    }

    fn vcs_engine() -> RootIndicatorEngine {
        engine_for(
            vec![create_test_indicator_with_indicators(
                "Rust",
                vec![("Cargo.toml", 0.95)],
            )],
            DetectionConfig {
                require_vcs_root: true,
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
        let found = vcs_engine().find_project_root_within(
            &start,
            1,
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        assert!(found.is_none());
        Ok(())
    }

    #[test]
    fn test_vcs_marker_within_the_traversal_bound_accepts_the_root(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, start) = project_with_distant_git()?;
        let found = vcs_engine().find_project_root_within(
            &start,
            2,
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        assert_eq!(found.map(|(root, _)| root), Some(start));
        Ok(())
    }

    #[test]
    fn test_wildcard_marker_is_found_in_a_huge_directory() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = directory_with_many_files("App.csproj")?;
        let found = engine_with_root("*.csproj").find_project_root(
            dir.path(),
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        assert_eq!(
            found.map(|(_, marker)| marker.pattern),
            Some("App.csproj".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_wildcard_marker_is_found_in_a_normal_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("App.csproj"), "")?;
        let found = engine_with_root("*.csproj").find_project_root(
            dir.path(),
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        assert!(found.is_some());
        Ok(())
    }

    /// On a case-insensitive filesystem (macOS, Windows) a literal stat finds
    /// `cargo.toml` for the pattern `Cargo.toml`, but the scanner matches
    /// names case-sensitively: the two must agree.
    #[test]
    fn test_root_marker_with_different_case_is_not_a_root() -> Result<(), Box<dyn std::error::Error>>
    {
        let engine = rust_engine(DetectionConfig::default());
        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("cargo.toml"),
            "[package]\nname = \"x\"",
        )?;

        let file_cache = FileSystemCache::default();
        assert!(engine
            .find_project_root(temp_dir.path(), &file_cache, &ParsedFileCache::new())
            .is_none());
        Ok(())
    }

    #[test]
    fn test_utf16_root_manifests_are_validated() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let pyproject = dir.path().join("pyproject.toml");
        fs::write(
            &pyproject,
            crate::test_utils::utf16le_with_bom("[project]\nname = \"x\"\n"),
        )?;
        assert!(RootIndicatorEngine::is_valid_python_project(
            &pyproject,
            &ParsedFileCache::new()
        ));
        Ok(())
    }

    #[test]
    fn test_windows_system_dirs_match_canonical_and_plain_forms() {
        for dir in [
            r"C:\Windows",
            "C:/Windows",
            r"\\?\C:\Windows",
            r"\\?\c:\users",
            r"\\?\c:/users",
            r"\\?\C:\Program Files (x86)",
            "C:/Program Files (x86)",
        ] {
            assert!(is_windows_system_dir(dir), "{dir}");
        }
        assert!(!is_windows_system_dir(r"\\?\C:\Windows\System32"));
        assert!(!is_windows_system_dir(r"\\?\C:\code"));
    }

    #[test]
    fn test_max_upward_traversal_counts_levels_above_start(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = engine_for(
            vec![create_test_indicator_with_indicators(
                "Rust",
                vec![("Cargo.toml", 0.95)],
            )],
            DetectionConfig::default(),
        );
        let temp_dir = TempDir::new()?;
        let start = temp_dir.path().join("a").join("b");
        fs::create_dir_all(&start)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"",
        )?;
        let cache = FileSystemCache::default();

        // Root is 2 levels above `start`
        assert!(engine
            .find_project_root_within(&start, 2, &cache, &ParsedFileCache::new())
            .is_some());
        assert!(engine
            .find_project_root_within(&start, 1, &cache, &ParsedFileCache::new())
            .is_none());
        Ok(())
    }

    #[test]
    fn test_shared_root_pattern_tie_prefers_first_in_config_order(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Two indicators claim the same root file with equal certainty:
        // everywhere else in the crate the earlier config row wins
        let early = create_test_indicator_with_indicators("Early", vec![("shared.lock", 0.8)]);
        let late = create_test_indicator_with_indicators("Late", vec![("shared.lock", 0.8)]);
        let engine = engine_for(vec![early, late], DetectionConfig::default());

        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("shared.lock"), "")?;
        let cache = FileSystemCache::default();
        let found = engine.find_project_root(temp_dir.path(), &cache, &ParsedFileCache::new());
        assert_eq!(
            found.map(|(_, f)| f.indicator.name.clone()),
            Some("Early".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_a_directory_named_like_a_literal_marker_is_not_a_marker(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let engine = rust_engine(DetectionConfig::default());
        let temp_dir = TempDir::new()?;
        fs::create_dir(temp_dir.path().join("Cargo.toml"))?;

        let cache = FileSystemCache::default();
        assert!(engine
            .find_project_root(temp_dir.path(), &cache, &ParsedFileCache::new())
            .is_none());
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
        let found = engine.find_project_root(temp_dir.path(), &cache, &ParsedFileCache::new());
        assert_eq!(found.map(|(_, f)| f.pattern), Some("A.csproj".to_string()));
        Ok(())
    }

    /// `package.json` certainty follows what the indicator claims (a
    /// `tsconfig.json` marker makes it the TypeScript flavour), not what the
    /// user happened to name it.
    fn package_json_certainty(
        indicator_name: &str,
        markers: Vec<(&str, f32)>,
        manifest: &str,
    ) -> Result<f32, Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let file = dir.path().join("package.json");
        fs::write(&file, manifest)?;
        let indicator = Arc::new(create_test_indicator_with_indicators(
            indicator_name,
            markers,
        ));
        let root = indicator
            .root_indicators
            .iter()
            .find(|r| r.pattern == "package.json")
            .ok_or("no package.json marker")?;
        Ok(RootIndicatorEngine::calculate_indicator_certainty(
            &file,
            root,
            &indicator,
            &std::cell::OnceCell::new(),
            &ParsedFileCache::new(),
        ))
    }

    const TS_MANIFEST: &str = r#"{"devDependencies":{"typescript":"^5"}}"#;
    const JS_MANIFEST: &str = r#"{"dependencies":{"express":"4"}}"#;

    #[test]
    fn test_renamed_typescript_indicator_keeps_typescript_certainty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let markers = vec![("tsconfig.json", 0.95), ("package.json", 0.9)];
        assert_eq!(
            package_json_certainty("Typed", markers.clone(), TS_MANIFEST)?,
            0.9_f32 * 0.9
        );
        assert_eq!(package_json_certainty("Typed", markers, JS_MANIFEST)?, 0.0);
        Ok(())
    }

    #[test]
    fn test_renamed_javascript_indicator_keeps_javascript_certainty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let markers = vec![("package.json", 0.95)];
        assert_eq!(
            package_json_certainty("Scripts", markers.clone(), JS_MANIFEST)?,
            0.95_f32 * 0.9
        );
        assert_eq!(
            package_json_certainty("Scripts", markers, TS_MANIFEST)?,
            0.0
        );
        Ok(())
    }

    #[test]
    fn test_typescript_detected_in_utf16_package_json() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let p = dir.path().join("package.json");
        fs::write(
            &p,
            crate::test_utils::utf16le_with_bom(r#"{"devDependencies":{"typescript":"^5"}}"#),
        )?;
        assert!(RootIndicatorEngine::has_typescript_dependencies(
            &p,
            &ParsedFileCache::new()
        ));
        Ok(())
    }

    #[test]
    fn test_typescript_detected_from_dependency_keys_only() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = TempDir::new()?;
        let check = |json: &str| -> Result<bool, Box<dyn std::error::Error>> {
            let p = dir.path().join("package.json");
            fs::write(&p, json)?;
            Ok(RootIndicatorEngine::has_typescript_dependencies(
                &p,
                &ParsedFileCache::new(),
            ))
        };

        // Real TypeScript signals, in any dependency section
        assert!(check(r#"{"devDependencies":{"typescript":"^5"}}"#)?);
        assert!(check(r#"{"dependencies":{"ts-node":"10"}}"#)?);
        assert!(check(r#"{"peerDependencies":{"typescript":">=4"}}"#)?);
        // Not TypeScript: @types/* alone is editor hints, not compiling
        assert!(!check(r#"{"devDependencies":{"@types/node":"20"}}"#)?);
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
