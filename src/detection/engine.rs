use crate::constants::*;
use crate::detection::caches::FileSystemCacheManager;
use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::detection::framework_detector::FrameworkDetector;
use crate::detection::language_resolver::LanguageResolver;
use crate::detection::pattern_compiler::PatternCompiler;
use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::detection::root_indicators::RootIndicatorEngine;
use crate::detection::scanner::ScanningEngine;
use crate::tracking::{DetectionMetadata, DetectionSnapshot, ResultTracker};
use crate::types::{DetectionConfig, DetectionEvidence, DetectionResult, ProjectIndicator};
use crate::Result;
use anyhow::Context;
use std::path::Path;
use std::sync::Arc;

/// Builder for creating `DetectionEngine` instances.
///
/// ## Usage
///
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngineBuilder;
/// # use project_indicator::types::ProjectIndicator;
/// let languages = vec![/* ... */];
/// let engine = DetectionEngineBuilder::new(languages).build();
/// ```
pub struct DetectionEngineBuilder {
    languages: Vec<ProjectIndicator>,
    config: DetectionConfig,
    result_tracker: Option<Arc<ResultTracker>>,
}

impl DetectionEngineBuilder {
    /// Creates a new builder with the given languages.
    ///
    /// All components will use default configurations unless overridden
    /// via `with_*` methods.
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self {
            languages,
            config: DetectionConfig::default(),
            result_tracker: None,
        }
    }

    /// Sets a custom detection configuration.
    pub fn with_config(mut self, config: DetectionConfig) -> Self {
        self.config = config;
        self
    }

    /// Inject a result tracker for recording detection history.
    ///
    /// If not provided, tracking is disabled (zero overhead).
    pub fn with_result_tracker(mut self, tracker: Arc<ResultTracker>) -> Self {
        self.result_tracker = Some(tracker);
        self
    }

    /// Builds the `DetectionEngine`.
    ///
    /// A single `PatternMatcher` is created and shared across the scorer and
    /// scanner so pattern-match memoization works across the whole pipeline.
    pub fn build(self) -> DetectionEngine {
        let languages: Vec<Arc<ProjectIndicator>> =
            self.languages.into_iter().map(Arc::new).collect();

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let cache_manager = FileSystemCacheManager::new();
        let confidence_scorer = ConfidenceScorer::with_pattern_matcher(pattern_matcher.clone());
        let language_resolver = LanguageResolver::default();
        let framework_detector = FrameworkDetector::new();

        // Create pattern compiler and scanning engine
        let pattern_compiler = PatternCompiler::new(&languages);
        let file_cache = cache_manager.file_existence_cache();

        let scanning_engine = ScanningEngine::with_cache(
            PatternProcessor::new(
                pattern_matcher.clone(),
                pattern_compiler.unique_patterns(),
                languages.clone(),
            ),
            self.config.max_depth,
            Some(file_cache),
        );

        let root_indicator_engine =
            RootIndicatorEngine::from_arc_languages(languages.clone(), self.config.clone());

        // Create result tracker if not provided
        let result_tracker = self
            .result_tracker
            .or_else(|| ResultTracker::new().ok().map(Arc::new));

        DetectionEngine {
            languages, // Use the converted Arc<ProjectIndicator> version
            config: self.config,
            pattern_matcher,
            cache_manager,
            confidence_scorer,
            language_resolver,
            framework_detector,
            scanning_engine,
            root_indicator_engine,
            result_tracker,
        }
    }
}

/// Main detection engine for identifying project languages and frameworks.
///
/// The DetectionEngine coordinates multiple specialized components to analyze
/// a project directory and determine its primary language and frameworks.
///
/// ## Construction
///
/// **Recommended**: Use `DetectionEngineBuilder` for maximum flexibility:
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngineBuilder;
/// # use project_indicator::types::ProjectIndicator;
/// let languages = vec![/* ... */];
/// let engine = DetectionEngineBuilder::new(languages).build();
/// ```
///
/// **Simple**: Use direct constructors for default configuration:
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngine;
/// # use project_indicator::types::ProjectIndicator;
/// let languages = vec![/* ... */];
/// let engine = DetectionEngine::new(languages);
/// ```
///
/// ## Architecture
///
/// - **PatternCompiler**: Extracts and compiles file patterns from language definitions
/// - **FileSystemCacheManager**: Manages file existence and parsed file caches
/// - **ScanningEngine**: Performs directory traversal and file matching with adaptive performance
/// - **LanguageResolver**: Resolves language conflicts when multiple languages detected
/// - **ConfidenceScorer**: Calculates confidence scores for language matches
/// - **FrameworkDetector**: Identifies frameworks within detected languages
/// - **RootIndicatorEngine**: Fast path detection using root indicator files
///
/// ## Shared Resources
///
/// ### PatternMatcher Ownership
///
/// DetectionEngine creates a single `Arc<PatternMatcher>` instance and shares it with:
/// - `ConfidenceScorer` - for calculating language match confidence
/// - `ScanningEngine` (via `PatternProcessor`) - for efficient file pattern matching
///
/// This design ensures:
/// 1. **Single cache instance** - All pattern matches benefit from shared cache
/// 2. **Memory efficiency** - No duplicate matcher instances
/// 3. **Thread safety** - PatternMatcher uses DashMap for concurrent access
/// 4. **Performance** - Cache hits across entire detection pipeline
pub struct DetectionEngine {
    languages: Vec<Arc<ProjectIndicator>>,
    config: DetectionConfig,

    // Shared pattern matcher (also held by scorer and scanner); kept here so
    // cache hit/miss deltas can be recorded in detection metadata
    pattern_matcher: Arc<PatternMatcher>,

    // Specialized components
    cache_manager: FileSystemCacheManager,
    confidence_scorer: ConfidenceScorer,
    language_resolver: LanguageResolver,
    framework_detector: FrameworkDetector,
    scanning_engine: ScanningEngine,
    root_indicator_engine: RootIndicatorEngine,

    // Optional result tracker
    result_tracker: Option<Arc<ResultTracker>>,
}

impl DetectionEngine {
    /// Creates a new `DetectionEngine` with default configuration.
    ///
    /// This is a convenience method that delegates to `DetectionEngineBuilder`.
    /// All components will be created with default settings.
    ///
    /// For custom configuration, use `DetectionEngineBuilder`:
    /// ```rust
    /// # use project_indicator::detection::engine::DetectionEngineBuilder;
    /// # use project_indicator::types::{ProjectIndicator, DetectionConfig};
    /// # let languages = vec![];
    /// # let custom_config = DetectionConfig::default();
    /// let engine = DetectionEngineBuilder::new(languages)
    ///     .with_config(custom_config)
    ///     .build();
    /// ```
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        DetectionEngineBuilder::new(languages).build()
    }

    /// Creates a new `DetectionEngine` with custom detection configuration.
    ///
    /// This is a convenience method that delegates to `DetectionEngineBuilder`.
    ///
    /// Equivalent to:
    /// ```rust
    /// # use project_indicator::detection::engine::DetectionEngineBuilder;
    /// # use project_indicator::types::{ProjectIndicator, DetectionConfig};
    /// # let languages = vec![];
    /// # let config = DetectionConfig::default();
    /// let engine = DetectionEngineBuilder::new(languages)
    ///     .with_config(config)
    ///     .build();
    /// ```
    pub fn with_config(
        languages: Vec<ProjectIndicator>,
        detection_config: DetectionConfig,
    ) -> Self {
        DetectionEngineBuilder::new(languages)
            .with_config(detection_config)
            .build()
    }

    /// Record a detection result to the tracker if enabled.
    ///
    /// This is a helper method to avoid code duplication across different detection paths.
    fn record_detection(
        &self,
        result: &DetectionResult,
        path: &Path,
        metadata: DetectionMetadata,
    ) -> Result<()> {
        if let Some(tracker) = &self.result_tracker {
            if tracker.is_enabled() {
                let snapshot = DetectionSnapshot::from_detection_result(
                    result,
                    path,
                    metadata,
                    Some(tracker.path_cache()),
                )?;
                tracker.record(snapshot)?;
            }
        }
        Ok(())
    }

    /// Calculate cache statistics delta for tracking purposes.
    ///
    /// Returns (hits, misses) that occurred since the provided baseline stats.
    fn calculate_cache_delta(&self, baseline: &crate::performance::CacheStats) -> (u64, u64) {
        let current = self.cache_manager.stats();
        let hits = current.hits.saturating_sub(baseline.hits);
        let misses = current.misses.saturating_sub(baseline.misses);
        (hits, misses)
    }

    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        let mut evidence = DetectionEvidence::new();
        let detection_start = std::time::Instant::now();

        // Capture cache state BEFORE detection (for tracking)
        let fs_stats_before = self.cache_manager.stats();
        let (pm_hits_before, pm_misses_before) = self.pattern_matcher.hit_miss_counts();

        // Safety check: Don't scan from boundary directories (home, system dirs)
        // These are too large and not meaningful to scan
        if self
            .root_indicator_engine
            .is_boundary_directory_public(path)
        {
            log::warn!(
                "Refusing to scan from boundary directory: {}",
                path.display()
            );
            return Ok(DetectionResult::new_with_evidence(
                None,
                Vec::new(),
                0.0,
                evidence,
            ));
        }

        // NEW: First, try to find the actual project root by walking upward
        let scan_path = if let Some((root_path, root_indicator)) = self
            .root_indicator_engine
            .find_project_root(path, &self.cache_manager.file_existence_cache())?
        {
            log::info!(
                "Upward traversal: Found project root at {} (started from {}, pattern: {})",
                root_path.display(),
                path.display(),
                root_indicator.pattern
            );

            let indicator_file_path = root_path.join(&root_indicator.pattern);
            evidence.add_root_evidence(crate::types::EvidenceItem::root_indicator(
                indicator_file_path.to_string_lossy().to_string(),
                root_indicator.pattern,
                root_indicator.certainty,
            ));

            root_path
        } else {
            log::debug!("No project root found via upward traversal, scanning from current path");
            path.to_path_buf()
        };

        // Continue with existing detection logic using scan_path instead of path
        if let Some(early_result) = self
            .root_indicator_engine
            .detect_with_early_termination(&scan_path, &self.cache_manager.file_existence_cache())
            .with_context(|| "Failed to check root indicators")?
        {
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                EARLY_TERMINATION.to_owned(),
                early_result.confidence,
                1.0,
                EARLY_TERMINATION_MSG.to_owned(),
            ));
            evidence.set_scan_metrics(0, detection_start.elapsed().as_millis() as u64);

            log::debug!(
                "Early termination successful: {} with confidence {:.3} in {}ms",
                early_result
                    .language
                    .as_ref()
                    .map(|l| l.name.as_str())
                    .unwrap_or("Unknown"),
                early_result.confidence,
                detection_start.elapsed().as_millis()
            );

            let frameworks = if early_result.confidence >= self.config.confidence_threshold {
                if let Some(ref language) = early_result.language {
                    self.framework_detector
                        .detect_frameworks_with_evidence(
                            &scan_path,
                            language,
                            &mut evidence,
                            &self.cache_manager.file_existence_cache(),
                            self.cache_manager.parsed_file_cache(),
                        )
                        .with_context(|| "Failed to detect frameworks")?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let mut result = early_result;
            result.frameworks = frameworks;
            result.evidence = evidence;

            // Record detection snapshot if tracking is enabled
            let (fs_hits, fs_misses) = self.calculate_cache_delta(&fs_stats_before);
            let (pm_hits, pm_misses) = self.pattern_matcher.hit_miss_counts();
            let metadata = DetectionMetadata {
                duration_micros: detection_start.elapsed().as_micros() as u64,
                from_cache: false,
                cached_at: None,
                pattern_cache_hits: pm_hits.saturating_sub(pm_hits_before),
                pattern_cache_misses: pm_misses.saturating_sub(pm_misses_before),
                fs_cache_hits: fs_hits as usize,
                fs_cache_misses: fs_misses as usize,
            };
            self.record_detection(&result, path, metadata)?;

            return Ok(result);
        }

        log::debug!("No definitive root indicators found, falling back to file scanning");
        let scan_start = std::time::Instant::now();

        let detailed_files = self
            .scanning_engine
            .scan(&scan_path)
            .with_context(|| format!("Failed to scan files in path: {}", scan_path.display()))?;

        evidence.set_scan_metrics(
            detailed_files.len(),
            scan_start.elapsed().as_millis() as u64,
        );

        if detailed_files.is_empty() {
            return Ok(DetectionResult::new_with_evidence(
                None,
                Vec::new(),
                0.0,
                evidence,
            ));
        }

        let detected_language = self
            .language_resolver
            .detect_language_with_conflict_resolution_and_evidence(
                &self.languages,
                &detailed_files,
                &mut evidence,
            );

        let language = match detected_language {
            Some(lang) => lang,
            None => {
                return Ok(DetectionResult::new_with_evidence(
                    None,
                    Vec::new(),
                    0.0,
                    evidence,
                ));
            }
        };

        let confidence = self
            .confidence_scorer
            .calculate_language_score_with_evidence(
                &language,
                &detailed_files,
                &mut evidence,
                &self.languages,
            );

        let frameworks = if confidence >= self.config.confidence_threshold {
            self.framework_detector
                .detect_frameworks_with_evidence(
                    &scan_path,
                    &language,
                    &mut evidence,
                    &self.cache_manager.file_existence_cache(),
                    self.cache_manager.parsed_file_cache(),
                )
                .with_context(|| "Failed to detect frameworks")?
        } else {
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                FRAMEWORK_DETECTION_SKIPPED.to_owned(),
                0.0,
                1.0,
                FRAMEWORK_DETECTION_SKIPPED_MSG.to_owned(),
            ));
            Vec::new()
        };

        let result =
            DetectionResult::new_with_evidence(Some(language), frameworks, confidence, evidence);

        // Record detection snapshot if tracking is enabled
        let (fs_hits, fs_misses) = self.calculate_cache_delta(&fs_stats_before);
        let (pm_hits, pm_misses) = self.pattern_matcher.hit_miss_counts();
        let metadata = DetectionMetadata {
            duration_micros: detection_start.elapsed().as_micros() as u64,
            from_cache: false,
            cached_at: None,
            pattern_cache_hits: pm_hits.saturating_sub(pm_hits_before),
            pattern_cache_misses: pm_misses.saturating_sub(pm_misses_before),
            fs_cache_hits: fs_hits as usize,
            fs_cache_misses: fs_misses as usize,
        };
        self.record_detection(&result, path, metadata)?;

        Ok(result)
    }

    pub fn get_root_indicator_stats(
        &self,
    ) -> crate::detection::root_indicators::RootIndicatorStats {
        self.root_indicator_engine.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::detection::matchers::test_helpers::helpers::create_test_language;
    use crate::test_utils::create_test_rust_project;

    #[test]
    fn test_detection_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngineBuilder::new(languages).build();

        assert_eq!(engine.languages.len(), 1);
        assert_eq!(engine.languages[0].name, "Rust");
        Ok(())
    }

    #[test]
    fn test_detection_engine_with_config() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let config = DetectionConfig::default();
        let engine = DetectionEngineBuilder::new(languages)
            .with_config(config)
            .build();

        assert_eq!(engine.languages.len(), 1);
        // Pattern compilation is handled internally during initialization
        Ok(())
    }

    #[test]
    fn test_detect_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngineBuilder::new(languages).build();
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.language.is_some());
        assert_eq!(
            result
                .language
                .as_ref()
                .ok_or("Failed to get language reference")?
                .name,
            "Rust"
        );
        assert!(result.confidence > 0.0);
        Ok(())
    }

    #[test]
    fn test_detect_no_match() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language(
            "Python",
            vec!["*.py", "requirements.txt"],
        )];
        let engine = DetectionEngineBuilder::new(languages).build();
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.language.is_none());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }
}
