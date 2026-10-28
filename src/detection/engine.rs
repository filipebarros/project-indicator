use crate::detection::caches::ParsedFileCache;
use crate::detection::confidence_scorer::calculate_indicator_score;
use crate::detection::framework_detector;
use crate::detection::indicator_resolver;
use crate::detection::pattern_processor::{normalized_relative, PatternProcessor};
use crate::detection::root_indicators::{FoundRootIndicator, RootIndicatorEngine};
use crate::detection::scanner::{ScanOutcome, ScanningEngine};
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionConfig, DetectionEvidence, DetectionResult, Framework, Indicator, NestedProject,
};
use crate::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A confirmed framework dependency floors confidence here: the dependency
/// exists in a real manifest, which identifies the project near-certainly.
const FRAMEWORK_MATCH_CONFIDENCE_FLOOR: f32 = 0.75;

/// Only app frameworks (React Native, Rails, Django, …) count as
/// framework-dependency evidence in arbitration. Build tooling (Vite,
/// priority 5) sits in many manifests of other-language projects and must
/// not flip the language.
const APP_FRAMEWORK_PRIORITY_CEILING: u8 = 2;

/// Framework-dependency evidence only arbitrates between contenders whose
/// strongest non-build marker is this close to the leader's: a framework
/// found through a clearly weaker manifest (a docs package.json beside a
/// Cargo.toml would still lose by certainty, but a React Native app's
/// package.json at 0.81 may beat the CocoaPods Gemfile at 0.86).
const FRAMEWORK_RULE_CERTAINTY_WINDOW: f32 = 0.1;

/// Subdirectories looked at (across both levels, sorted by name, so the
/// same ones every time) when a VCS-tracked folder is not a project itself
pub const MAX_NESTED_EXAMINED: usize = 24;

/// Projects reported for such a folder
pub const MAX_NESTED_SHOWN: usize = 4;

/// Directory levels above the start that root discovery may climb. The
/// boundary-directory stop in root discovery is the real guard.
pub const MAX_UPWARD_TRAVERSAL: usize = 4;

/// Detects the project type and frameworks of one directory: root discovery,
/// scan, indicator resolution, confidence, then framework detection.
pub struct DetectionEngine {
    indicators: Vec<Arc<Indicator>>,
    frameworks: Arc<[Framework]>,

    pub(crate) scanning_engine: ScanningEngine,
    pub(crate) root_indicator_engine: RootIndicatorEngine,
}

impl DetectionEngine {
    pub fn new(indicators: Vec<Indicator>, frameworks: Vec<Framework>) -> Self {
        Self::with_config(indicators, frameworks, DetectionConfig::default())
    }

    pub fn with_config(
        indicators: Vec<Indicator>,
        frameworks: Vec<Framework>,
        detection_config: DetectionConfig,
    ) -> Self {
        // Filename-shaped root patterns are scan patterns too: union them
        // into `files` so a root marker behaves exactly as if it had been
        // listed there (scan match, confidence numerator and denominator,
        // evidence recording) without every language duplicating its markers.
        // `/`-shaped patterns (project/build.properties) are deliberately NOT
        // unioned: root discovery probes them, and in `files` they would make
        // `root_source_count` treat a probed build file as root SOURCE
        // evidence in arbitration.
        let indicators: Vec<Arc<Indicator>> = indicators
            .into_iter()
            .map(|mut indicator| {
                let (files, roots) = (&mut indicator.files, &indicator.root_indicators);
                for root in roots {
                    if !root.pattern.contains('/') && !files.contains(&root.pattern) {
                        files.push(root.pattern.clone());
                    }
                }
                Arc::new(indicator)
            })
            .collect();
        let frameworks: Arc<[Framework]> = Arc::from(frameworks);

        // Deduplicated set of every file pattern the indicators reference
        let unique_patterns: Arc<[String]> = indicators
            .iter()
            .flat_map(|i| i.files.iter().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let scanning_engine = ScanningEngine::new(PatternProcessor::new(unique_patterns));

        let root_indicator_engine =
            RootIndicatorEngine::from_parts(indicators.clone(), detection_config);

        Self {
            indicators,
            frameworks,
            scanning_engine,
            root_indicator_engine,
        }
    }

    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        if let Some(repository_root) = crate::detection::repository::find_vcs_root(path) {
            return Ok(crate::detection::repository::detect_repository(
                self,
                &repository_root,
            ));
        }
        Ok(self.detect_project(path))
    }

    fn detect_project(&self, path: &Path) -> DetectionResult {
        let mut result = self.detect_one(path, MAX_UPWARD_TRAVERSAL, false);
        // A VCS-tracked folder (worktrees keep .git as a FILE, so a plain
        // existence check is right) that is no project and has none above it
        // reports the projects inside instead. Plain folders (~/Developer)
        // are never scanned inside. Always on; creating or removing the VCS
        // marker changes the folder's mtime, which the cache stamp watches.
        let vcs_tracked = || {
            crate::detection::root_indicators::VCS_MARKERS
                .iter()
                .any(|marker| path.join(marker).exists())
        };
        if result.indicator.is_none()
            && result.evidence.scan_root.is_none()
            && !self.root_indicator_engine.is_boundary_directory(path)
            && vcs_tracked()
        {
            self.attach_nested_projects(path, &mut result);
        }
        result
    }

    /// Non-VCS fallback detection for projects up to two levels below `path`
    /// (monorepo layouts keep them in `apps/*`, `services/*`): at most
    /// `MAX_NESTED_EXAMINED` subdirectories across both levels, by name;
    /// hidden ones, symlinks and noise directories (node_modules, vendor,
    /// target, dist, …) skipped; stopping at `MAX_NESTED_SHOWN` projects.
    /// A first-level folder that is no project is looked into; a project's
    /// own subdirectories are not. Everything examined becomes an input of
    /// `result`, for the persistent cache.
    fn attach_nested_projects(&self, path: &Path, result: &mut DetectionResult) {
        let mut examined = 0;
        let first_level = Self::project_candidate_dirs(path);
        let mut queue: std::collections::VecDeque<String> = first_level.into();

        // A subdirectory only needs to look at itself: the folder's own upward
        // search already found no project above it. (With `require_vcs_root`, a
        // child whose VCS marker is out of reach is rejected as a root but is
        // still detected from its own path, so it needs no look upward either.)
        while let Some(name) = queue.pop_front() {
            if result.nested.len() >= MAX_NESTED_SHOWN || examined >= MAX_NESTED_EXAMINED {
                break;
            }
            examined += 1;
            let child = path.join(&name);
            let found = self.detect_one(&child, 0, true);

            merge_child_evidence(&mut result.evidence, &child, &found.evidence);

            if let Some(indicator) = found.indicator {
                result.nested.push(NestedProject {
                    name,
                    indicator,
                    frameworks: found.frameworks,
                });
            } else if !name.contains('/') {
                // a first-level non-project may hold the projects one deeper
                for grandchild in Self::project_candidate_dirs(&child) {
                    queue.push_back(format!("{name}/{grandchild}"));
                }
            }
        }
        result.evidence.watched_dirs.sort();
        result.evidence.watched_dirs.dedup();
    }

    /// The subdirectories of `path` worth examining for projects, sorted by
    /// name: hidden ones, symlinks and noise directories (dependencies,
    /// build output, tests, docs — `DirectoryType::classify`) are skipped.
    fn project_candidate_dirs(path: &Path) -> Vec<String> {
        crate::performance::smallest(
            std::fs::read_dir(path)
                .into_iter()
                .flatten()
                .flatten()
                // `file_type` does not follow symlinks: a linked directory is skipped
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| !name.starts_with('.'))
                .filter(|name| {
                    crate::types::DirectoryType::classify(name)
                        != crate::types::DirectoryType::Noise
                }),
            MAX_NESTED_EXAMINED,
        )
    }

    /// Ordered arbitration for a contested root — several languages with
    /// qualifying markers, or another language with root sources:
    /// 1. an app-framework dependency declared in the contender's own
    ///    manifests (see the two constants above for its limits);
    /// 2. the strongest marker that is NOT a shared build file (manifest,
    ///    lockfile, source directory);
    /// 3. root source files (a file that is itself some marker's name, like
    ///    build.gradle.kts, is build configuration, not sources);
    /// 4. the strongest BuildSystem marker (pom.xml, build.gradle[.kts]);
    /// 5. config order (self.indicators is the stable iteration order).
    #[allow(clippy::too_many_arguments)]
    fn arbitrate_contested(
        &self,
        scan_path: &Path,
        markers: &[FoundRootIndicator],
        source_indicators: &[Arc<Indicator>],
        detailed_files: &[crate::types::MatchedFile],
        evidence: &mut DetectionEvidence,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Option<(Arc<Indicator>, f32)> {
        struct Contender {
            indicator: Arc<Indicator>,
            framework_deps: bool,
            /// best marker that is NOT a shared build file
            language_root_certainty: f32,
            has_source: bool,
            /// best BuildSystem marker: shared between languages, so it
            /// only breaks late ties
            build_system_certainty: f32,
        }
        let mut contenders: Vec<Contender> = Vec::new();
        for candidate in &self.indicators {
            let (mut language_root_certainty, mut build_system_certainty) = (0.0f32, 0.0f32);
            for marker in markers
                .iter()
                .filter(|m| m.indicator.name == candidate.name)
            {
                if marker.context == crate::types::IndicatorContext::BuildSystem {
                    build_system_certainty = build_system_certainty.max(marker.certainty);
                } else {
                    language_root_certainty = language_root_certainty.max(marker.certainty);
                }
            }
            let has_source = source_indicators.iter().any(|s| s.name == candidate.name);
            if language_root_certainty == 0.0 && build_system_certainty == 0.0 && !has_source {
                continue;
            }
            contenders.push(Contender {
                indicator: candidate.clone(),
                framework_deps: false,
                language_root_certainty,
                has_source,
                build_system_certainty,
            });
        }

        // Framework evidence is consulted only inside the certainty window
        // of the leader, and only for app frameworks
        let leader_certainty = contenders
            .iter()
            .map(|c| c.language_root_certainty)
            .fold(0.0f32, f32::max);
        for contender in &mut contenders {
            if leader_certainty - contender.language_root_certainty
                > FRAMEWORK_RULE_CERTAINTY_WINDOW
            {
                continue;
            }
            let mut scratch = DetectionEvidence::default();
            contender.framework_deps = framework_detector::detect_frameworks(
                scan_path,
                &contender.indicator,
                &self.frameworks,
                &mut scratch,
                file_cache,
                parsed_cache,
            )
            .iter()
            .any(|m| {
                matches!(
                    m.framework.detection,
                    crate::types::DetectionType::Dependencies(..)
                ) && m.framework.priority <= APP_FRAMEWORK_PRIORITY_CEILING
            });
        }

        contenders.sort_by(|a, b| {
            b.framework_deps
                .cmp(&a.framework_deps)
                .then(
                    b.language_root_certainty
                        .total_cmp(&a.language_root_certainty),
                )
                .then(b.has_source.cmp(&a.has_source))
                .then(
                    b.build_system_certainty
                        .total_cmp(&a.build_system_certainty),
                )
        });
        let winner = contenders.into_iter().next()?;
        let winner_certainty = winner
            .language_root_certainty
            .max(winner.build_system_certainty);
        let confidence = if winner_certainty > 0.0 {
            winner_certainty
        } else {
            // won on sources alone: report the honest scan ratio
            calculate_indicator_score(&winner.indicator, detailed_files, &self.indicators)
        };
        evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
            "arbitration".to_string(),
            confidence,
            format!(
                "{} won arbitration (framework deps: {}, root sources: {}, marker certainty: {:.2})",
                winner.indicator.name, winner.framework_deps, winner.has_source, winner_certainty
            ),
        ));
        Some((winner.indicator, confidence))
    }

    pub(crate) fn detect_one(
        &self,
        path: &Path,
        max_levels_above: usize,
        vcs_root_known: bool,
    ) -> DetectionResult {
        let mut evidence = DetectionEvidence::default();
        let file_cache = FileSystemCache::default();
        let parsed_cache = ParsedFileCache::new();

        if self.root_indicator_engine.is_boundary_directory(path) {
            log::debug!(
                "Refusing to scan from boundary directory: {}",
                path.display()
            );
            return no_match(evidence);
        }

        if !vcs_root_known
            && !self
                .root_indicator_engine
                .vcs_requirement_met(path, max_levels_above, &file_cache)
        {
            evidence.read_failed |= file_cache.read_failed();
            return no_match(evidence);
        }

        // Every qualifying marker of the chosen root (empty: no marker
        // anywhere in reach). The directory is listed once; uncontested,
        // contested and markerless all decide from this one set.
        let (root_markers, scan_path) = if let Some((root_path, markers)) = self
            .root_indicator_engine
            .find_root_markers_within(path, max_levels_above, &file_cache, &parsed_cache)
        {
            log::info!(
                "Upward traversal: Found project root at {} (started from {})",
                root_path.display(),
                path.display(),
            );
            evidence.scan_root = Some(root_path.clone());
            (markers, root_path)
        } else {
            log::debug!("No project root found via upward traversal, scanning from current path");
            // Upward discovery found no root — including when require_vcs_root
            // rejected one. The folder's own markers still identify it, so
            // probe the start path itself before falling back to source files.
            let own_markers =
                self.root_indicator_engine
                    .qualifying_markers_at(path, &file_cache, &parsed_cache);
            (own_markers, path.to_path_buf())
        };
        if !root_markers.is_empty() {
            let best = RootIndicatorEngine::best_marker(&root_markers);
            evidence.add_root_evidence(crate::types::EvidenceItem::root_indicator(
                scan_path.join(&best.pattern).to_string_lossy().to_string(),
                best.pattern.clone(),
                best.certainty,
            ));
        }

        let scan_start = std::time::Instant::now();
        let outcome = self.scanning_engine.scan_with_status(&scan_path);
        let scan_duration_ms = scan_start.elapsed().as_millis() as u64;

        self.detect_from_scan(
            scan_path,
            root_markers,
            outcome,
            scan_duration_ms,
            evidence,
            file_cache,
            parsed_cache,
        )
    }

    /// Project detection for a directory whose qualifying markers and scan
    /// repository aggregation already produced: no upward discovery, no
    /// second listing, no second scan. The directory is its own root
    /// (`max_levels_above = 0` with the VCS root known), as in repository
    /// aggregation, so the supplied markers play both branches of root
    /// discovery.
    pub(crate) fn detect_one_with_scan(
        &self,
        path: &Path,
        root_markers: Vec<FoundRootIndicator>,
        scan: ScanOutcome,
    ) -> DetectionResult {
        let mut evidence = DetectionEvidence::default();
        let file_cache = FileSystemCache::default();
        let parsed_cache = ParsedFileCache::new();

        if self.root_indicator_engine.is_boundary_directory(path) {
            log::debug!(
                "Refusing to scan from boundary directory: {}",
                path.display()
            );
            return no_match(evidence);
        }

        if !root_markers.is_empty() {
            // Mirror find_root_markers_within: the root is recorded only
            // when the configured VCS requirement accepts the directory
            // itself; otherwise detection still uses the supplied markers,
            // exactly like the own-markers fallback.
            if self
                .root_indicator_engine
                .root_vcs_validated(path, &file_cache)
            {
                evidence.scan_root = Some(path.to_path_buf());
            }
            let best = RootIndicatorEngine::best_marker(&root_markers);
            evidence.add_root_evidence(crate::types::EvidenceItem::root_indicator(
                path.join(&best.pattern).to_string_lossy().to_string(),
                best.pattern.clone(),
                best.certainty,
            ));
        }

        // The scan ran before this call; only its outcome is supplied.
        self.detect_from_scan(
            path.to_path_buf(),
            root_markers,
            scan,
            0,
            evidence,
            file_cache,
            parsed_cache,
        )
    }

    /// Everything after the scan in `detect_one` and `detect_one_with_scan`:
    /// source evidence, arbitration, framework detection, confidence
    /// flooring, evidence finishing. Every early return of the original
    /// scan-then-decide flow lives here.
    // By-value caches and markers keep the extracted body identical to the
    // inline flow it came from; the helper is a mechanical split.
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn detect_from_scan(
        &self,
        scan_path: PathBuf,
        root_markers: Vec<FoundRootIndicator>,
        outcome: ScanOutcome,
        scan_duration_ms: u64,
        mut evidence: DetectionEvidence,
        file_cache: FileSystemCache,
        parsed_cache: ParsedFileCache,
    ) -> DetectionResult {
        let detailed_files = outcome.matches;
        evidence.scan_truncated = outcome.truncated;
        evidence.read_failed = outcome.unreadable;
        evidence.watched_dirs = outcome.watched_dirs;

        evidence.set_scan_metrics(detailed_files.len(), scan_duration_ms);

        // An empty scan only means "no project" when root discovery found
        // nothing either: a marker absent from its indicator's file patterns
        // (settings.gradle, pubspec.lock) must still identify the project
        // instead of blacking out the folder and everything beneath it.
        if detailed_files.is_empty() && root_markers.is_empty() {
            finish_evidence(&mut evidence, &parsed_cache, &file_cache, &scan_path);
            return no_match(evidence);
        }

        // Which indicators have real source evidence at the scan root: a
        // second language's sources contest a marker just like a second
        // marker does (a Groovy build.gradle beside Main.kt is not a Java
        // project yet).
        let source_indicators: Vec<Arc<Indicator>> = self
            .indicators
            .iter()
            .filter(|candidate| {
                indicator_resolver::root_source_evidence(
                    candidate,
                    &detailed_files,
                    &self.indicators,
                )
            })
            .cloned()
            .collect();

        // An uncontested root marker decides by itself: scoring exists to
        // arbitrate contested directories, and a lone manifest IS the answer.
        // Confidence is the marker's certainty (weight × context), so a
        // manifest reads higher than a lockfile alone.
        let uncontested = (!root_markers.is_empty()
            && root_markers
                .iter()
                .all(|m| m.indicator.name == root_markers[0].indicator.name)
            && source_indicators
                .iter()
                .all(|src| src.name == root_markers[0].indicator.name))
        .then(|| RootIndicatorEngine::best_marker(&root_markers).clone());
        let (indicator, confidence) = if let Some(root_match) = uncontested {
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                "root_indicator_decides".to_string(),
                root_match.certainty,
                format!(
                    "{} identified by its root marker alone ({})",
                    root_match.indicator.name, root_match.pattern
                ),
            ));
            (root_match.indicator, root_match.certainty)
        } else if !root_markers.is_empty() {
            match self.arbitrate_contested(
                &scan_path,
                &root_markers,
                &source_indicators,
                &detailed_files,
                &mut evidence,
                &file_cache,
                &parsed_cache,
            ) {
                Some(decision) => decision,
                None => {
                    finish_evidence(&mut evidence, &parsed_cache, &file_cache, &scan_path);
                    return no_match(evidence);
                }
            }
        } else {
            // No qualifying marker anywhere: only source files identify a
            // language — a marker pattern whose content check failed (a
            // tool-only pyproject.toml) and tool configs or version pins
            // alone identify nothing.
            let detected_indicator = indicator_resolver::resolve_indicator(
                &self.indicators,
                &detailed_files,
                &mut evidence,
            );

            let Some(indicator) = detected_indicator else {
                finish_evidence(&mut evidence, &parsed_cache, &file_cache, &scan_path);
                return no_match(evidence);
            };

            let confidence =
                calculate_indicator_score(&indicator, &detailed_files, &self.indicators);
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                "ratio_score".to_string(),
                confidence,
                format!(
                    "Ratio score for {} (before the confidence floor)",
                    indicator.name
                ),
            ));
            (indicator, confidence)
        };

        // Framework detection is not gated on confidence: a framework
        // dependency in a manifest is evidence in its own right
        let frameworks = framework_detector::detect_frameworks(
            &scan_path,
            &indicator,
            &self.frameworks,
            &mut evidence,
            &file_cache,
            &parsed_cache,
        );

        finish_evidence(&mut evidence, &parsed_cache, &file_cache, &scan_path);

        // A confirmed framework DEPENDENCY floors every path: the dependency
        // exists in a real manifest, which identifies the project
        // near-certainly. A FileExists framework (tauri.conf.json beside a
        // lone app.js) is weaker evidence and must not jump a 0.05 source
        // guess to 0.75 — the framework is still listed, the language
        // confidence stays honest.
        let framework_floor = if frameworks.iter().any(|m| {
            matches!(
                m.framework.detection,
                crate::types::DetectionType::Dependencies(..)
            )
        }) {
            FRAMEWORK_MATCH_CONFIDENCE_FLOOR
        } else {
            0.0
        };
        let floored = confidence.max(framework_floor).min(1.0);
        if floored > confidence {
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                "confidence_floor".to_string(),
                floored,
                "Confidence floored by framework evidence".to_string(),
            ));
        }

        DetectionResult::new_with_evidence(Some(indicator), frameworks, floored, evidence)
    }
}

/// Make a nested project's inputs inputs of the folder's result: the child
/// directory, and what it read and listed as absolute paths (its own evidence
/// is relative to its scan root). An incomplete child makes the folder's
/// result incomplete.
pub(crate) fn merge_child_evidence(
    parent: &mut DetectionEvidence,
    child_dir: &Path,
    found: &DetectionEvidence,
) {
    let root = found.scan_root.as_deref().unwrap_or(child_dir);
    let absolute = |relative: &String| root.join(relative).to_string_lossy().into_owned();
    parent
        .watched_dirs
        .push(child_dir.to_string_lossy().into_owned());
    parent
        .watched_dirs
        .extend(found.watched_dirs.iter().map(absolute));
    parent
        .read_files
        .extend(found.read_files.iter().map(absolute));
    parent.scan_truncated |= found.scan_truncated;
    parent.read_failed |= found.read_failed;
}

/// What the persistent cache must watch for this result, recorded on every
/// return path after the scan (a "no match" depends on it too): every file
/// read, and the directories where a probed path marker would appear.
fn finish_evidence(
    evidence: &mut DetectionEvidence,
    parsed_cache: &ParsedFileCache,
    file_cache: &FileSystemCache,
    scan_path: &Path,
) {
    let name = |path: &Path| {
        normalized_relative(path, scan_path).unwrap_or_else(|| path.to_string_lossy().into_owned())
    };
    evidence.read_failed |= parsed_cache.read_failed() || file_cache.read_failed();
    evidence.read_files = parsed_cache.read_paths().iter().map(|p| name(p)).collect();
    // the scan root itself is always watched: no entry for it
    evidence.watched_dirs.extend(
        file_cache
            .watched_dirs()
            .iter()
            .map(|p| name(p))
            .filter(|dir| !dir.is_empty()),
    );
    evidence.watched_dirs.sort();
    evidence.watched_dirs.dedup();
}

fn no_match(evidence: DetectionEvidence) -> DetectionResult {
    DetectionResult::new_with_evidence(None, Vec::new(), 0.0, evidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator;
    use crate::test_utils::create_test_rust_project;

    #[test]
    fn test_detect_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator("Rust", &["Cargo.toml", "*.rs"])];
        let engine = DetectionEngine::new(indicators, vec![]);
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.indicator.is_some());
        assert_eq!(
            result
                .indicator
                .as_ref()
                .ok_or("Failed to get indicator reference")?
                .name,
            "Rust"
        );
        assert!(result.confidence > 0.0);
        Ok(())
    }

    #[test]
    fn test_detect_one_with_scan_uses_the_supplied_snapshot(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rust = Indicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#FF0000".to_string(),
            "R".to_string(),
            vec![],
            vec![crate::types::RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 0.95,
                context: crate::types::IndicatorContext::LanguageRoot,
                alternative_group: None,
            }],
        );
        let engine = DetectionEngine::new(vec![rust], vec![]);
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"snap\"\n",
        )?;
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n")?;

        let listing = crate::detection::directory::DirectoryListing::read(dir.path());
        let markers = engine.root_indicator_engine.qualifying_markers_in(
            dir.path(),
            &listing,
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        );
        let scan = engine
            .scanning_engine
            .scan_with_listing(dir.path(), &listing);
        let matched = scan.matches.len();
        std::fs::remove_file(dir.path().join("Cargo.toml"))?;

        let result = engine.detect_one_with_scan(dir.path(), markers, scan);

        let indicator = result.indicator.as_ref().ok_or("expected a Rust project")?;
        assert_eq!(indicator.name, "Rust");
        assert!(result.confidence > 0.0);
        assert_eq!(result.evidence.files_matched, matched);
        assert!(result.evidence.scan_root.is_some());
        assert!(result
            .evidence
            .root_discovery
            .iter()
            .any(|item| item.pattern_matched == "Cargo.toml"));
        Ok(())
    }

    #[test]
    fn test_large_directory_wildcard_root_marker_wins() -> Result<(), Box<dyn std::error::Error>> {
        let config = crate::config::built_in_config();
        let engine = DetectionEngine::new(config.indicators, config.frameworks);
        let dir = tempfile::TempDir::new()?;
        for index in 0..520 {
            std::fs::write(dir.path().join(format!("f{index:03}.rs")), "")?;
        }
        std::fs::write(dir.path().join("zz.csproj"), "<Project/>")?;

        let result = engine.detect(dir.path())?;

        assert_eq!(
            result
                .indicator
                .as_ref()
                .map(|indicator| indicator.name.as_str()),
            Some("C#")
        );
        Ok(())
    }

    fn engine_requiring_vcs() -> DetectionEngine {
        let mut config = crate::config::built_in_config();
        config.detection.require_vcs_root = true;
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection)
    }

    #[test]
    fn test_vcs_root_is_required_for_manifest_detection() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
        let engine = engine_requiring_vcs();

        assert!(engine.detect(dir.path())?.indicator.is_none());

        std::fs::create_dir(dir.path().join(".git"))?;
        assert_eq!(
            engine
                .detect(dir.path())?
                .nested
                .first()
                .map(|project| project.indicator.name.as_str()),
            Some("Rust")
        );
        Ok(())
    }

    #[test]
    fn test_vcs_root_is_required_for_source_detection() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n")?;
        let engine = engine_requiring_vcs();

        assert!(engine.detect(dir.path())?.indicator.is_none());

        std::fs::create_dir(dir.path().join(".git"))?;
        assert_eq!(
            engine
                .detect(dir.path())?
                .nested
                .first()
                .map(|project| project.indicator.name.as_str()),
            Some("Rust")
        );
        Ok(())
    }

    mod root_pattern_union {
        use super::*;
        use crate::types::{IndicatorContext, RootIndicator};

        fn lang_with_roots() -> Indicator {
            let root = |pattern: &str| RootIndicator {
                pattern: pattern.to_string(),
                weight: 0.95,
                context: IndicatorContext::LanguageRoot,
                alternative_group: None,
            };
            Indicator::with_root_indicators(
                "Lang".to_string(),
                vec!["*.foo".to_string()],
                "#FF0000".to_string(),
                "L".to_string(),
                vec![],
                vec![root("lang.toml"), root("project/lang.properties")],
            )
        }

        #[test]
        fn test_filename_shaped_root_patterns_are_unioned_into_files() {
            let engine = DetectionEngine::new(vec![lang_with_roots()], vec![]);

            let files = &engine.indicators[0].files;
            assert!(
                files.contains(&"lang.toml".to_string()),
                "a root marker not listed in files must be derived, got: {files:?}"
            );
            assert!(
                !files.contains(&"project/lang.properties".to_string()),
                "a `/`-shaped root pattern must stay probed-only (it would \
                 count as root SOURCE evidence in arbitration), got: {files:?}"
            );
        }

        #[test]
        fn test_a_derived_root_pattern_produces_a_scan_match(
        ) -> Result<(), Box<dyn std::error::Error>> {
            // Swift's built-in files list does not repeat Package.swift; the
            // union must still let the scan match it (confidence numerator,
            // evidence, cache inputs).
            let config = crate::config::built_in_config();
            let engine = DetectionEngine::new(config.indicators, config.frameworks);
            let dir = tempfile::TempDir::new()?;
            std::fs::write(dir.path().join("Package.swift"), "// swift-tools-version:5")?;

            let result = engine.detect(dir.path())?;

            assert_eq!(
                result.indicator.as_ref().map(|i| i.name.as_str()),
                Some("Swift")
            );
            assert_eq!(
                result.evidence.files_matched, 1,
                "the derived Package.swift pattern must match in the scan"
            );
            Ok(())
        }

        #[test]
        fn test_a_path_shaped_root_pattern_is_probed_but_not_scanned(
        ) -> Result<(), Box<dyn std::error::Error>> {
            // Scala's project/build.properties identifies the root via the
            // probe, but must not enter the scan set: a scan match for it
            // would count as root source evidence in arbitration.
            let config = crate::config::built_in_config();
            let engine = DetectionEngine::new(config.indicators, config.frameworks);
            let dir = tempfile::TempDir::new()?;
            std::fs::create_dir(dir.path().join("project"))?;
            std::fs::write(
                dir.path().join("project/build.properties"),
                "sbt.version=1.9",
            )?;

            let result = engine.detect(dir.path())?;

            assert_eq!(
                result.indicator.as_ref().map(|i| i.name.as_str()),
                Some("Scala")
            );
            assert_eq!(
                result.evidence.files_matched, 0,
                "a `/`-shaped root pattern must not produce a scan match"
            );
            Ok(())
        }
    }

    /// The file caches belong to one `detect` call: a second call on the same
    /// engine must see the project as it is now, not as it was.
    #[test]
    fn test_second_detect_sees_manifest_changes() -> Result<(), Box<dyn std::error::Error>> {
        let config = crate::config::built_in_config();
        let engine = DetectionEngine::new(config.indicators, config.frameworks);
        let dir = tempfile::TempDir::new()?;
        let manifest = dir.path().join("package.json");
        std::fs::write(&manifest, r#"{"dependencies":{}}"#)?;
        let names = |r: &DetectionResult| -> Vec<String> {
            r.frameworks
                .iter()
                .map(|f| f.framework.name.clone())
                .collect()
        };

        assert!(!names(&engine.detect(dir.path())?).contains(&"React".to_string()));

        std::fs::write(&manifest, r#"{"dependencies":{"react":"18"}}"#)?;
        assert!(names(&engine.detect(dir.path())?).contains(&"React".to_string()));
        Ok(())
    }

    #[test]
    fn test_detect_no_match() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator(
            "Python",
            &["*.py", "requirements.txt"],
        )];
        let engine = DetectionEngine::new(indicators, vec![]);
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.indicator.is_none());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }

    mod merge {
        use super::super::merge_child_evidence;
        use crate::types::DetectionEvidence;
        use std::path::{Path, PathBuf};

        /// Expected absolute input: joined exactly as the production code
        /// joins it, so the assertion holds on Windows (`\\`) and Unix (`/`)
        fn abs(root: &str, relative: &str) -> String {
            Path::new(root)
                .join(relative)
                .to_string_lossy()
                .into_owned()
        }

        fn child_evidence() -> DetectionEvidence {
            DetectionEvidence {
                scan_root: Some(PathBuf::from("/ws/app")),
                read_files: vec!["package.json".to_string()],
                watched_dirs: vec!["src/main".to_string()],
                ..DetectionEvidence::default()
            }
        }

        #[test]
        fn test_a_child_is_watched_and_its_inputs_become_absolute() {
            let mut parent = DetectionEvidence::default();

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &child_evidence());

            assert_eq!(parent.read_files, [abs("/ws/app", "package.json")]);
            assert_eq!(
                parent.watched_dirs,
                ["/ws/app".to_string(), abs("/ws/app", "src/main")]
            );
        }

        #[test]
        fn test_inputs_are_relative_to_the_childs_scan_root_not_its_directory() {
            // a root found elsewhere than the directory the child was run on
            let mut found = child_evidence();
            found.scan_root = Some(PathBuf::from("/ws/elsewhere"));
            let mut parent = DetectionEvidence::default();

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &found);

            assert_eq!(parent.read_files, [abs("/ws/elsewhere", "package.json")]);
        }

        #[test]
        fn test_an_unreadable_child_makes_the_result_uncacheable() {
            let mut found = child_evidence();
            found.read_failed = true;
            let mut parent = DetectionEvidence::default();

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &found);

            assert!(!parent.is_cacheable());
        }

        #[test]
        fn test_a_truncated_child_makes_the_result_uncacheable() {
            let mut found = child_evidence();
            found.scan_truncated = true;
            let mut parent = DetectionEvidence::default();

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &found);

            assert!(!parent.is_cacheable());
        }

        #[test]
        fn test_a_clean_child_leaves_a_cacheable_parent_cacheable() {
            let mut parent = DetectionEvidence::default();

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &child_evidence());

            assert!(parent.is_cacheable());
        }

        #[test]
        fn test_a_dirty_parent_is_not_cleaned_by_a_clean_child() {
            let mut parent = DetectionEvidence {
                read_failed: true,
                ..DetectionEvidence::default()
            };

            merge_child_evidence(&mut parent, Path::new("/ws/app"), &child_evidence());

            assert!(!parent.is_cacheable());
        }
    }
}
