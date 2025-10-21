use crate::constants::*;
use crate::detection::caches::{DetectionCache, FileSystemCacheManager};
use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::detection::framework_detector::FrameworkDetector;
use crate::detection::language_resolver::LanguageResolver;
use crate::detection::pattern_compiler::PatternCompiler;
use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::detection::root_indicators::RootIndicatorEngine;
use crate::detection::scanner::ScanningEngine;
use crate::tracking::{DetectionMetadata, DetectionSnapshot, ResultTracker};
use crate::types::{
    DetectionConfig, DetectionEvidence, DetectionResult, DetectionType, ProjectIndicator,
};
use crate::Result;
use anyhow::Context;
use std::path::Path;
use std::sync::Arc;

/// Estimated duration in microseconds for cached detection results.
/// Cached results are typically very fast (< 100 microseconds).
const CACHED_DETECTION_DURATION_MICROS: u64 = 100;

/// Builder for creating `DetectionEngine` instances with optional component injection.
///
/// The builder pattern enables:
/// - **Flexible construction**: Inject custom components for testing or customization
/// - **Backward compatibility**: Existing code continues to work via `DetectionEngine::new()`
/// - **Future extensibility**: Easy to add new optional components (e.g., metrics)
///
/// ## Usage
///
/// ### Default construction (simple)
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngineBuilder;
/// # use project_indicator::types::ProjectIndicator;
/// let languages = vec![/* ... */];
/// let engine = DetectionEngineBuilder::new(languages).build();
/// ```
///
/// ### Custom cache configuration
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngineBuilder;
/// # use project_indicator::detection::caches::FileSystemCacheManager;
/// # use project_indicator::types::ProjectIndicator;
/// let languages = vec![/* ... */];
/// let custom_cache = FileSystemCacheManager::with_ttl(60); // 1 minute TTL
///
/// let engine = DetectionEngineBuilder::new(languages)
///     .with_cache_manager(custom_cache)
///     .build();
/// ```
///
/// ### Testing with custom components
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngineBuilder;
/// # use project_indicator::detection::pattern_matching::PatternMatcher;
/// # use project_indicator::types::ProjectIndicator;
/// # use std::sync::Arc;
/// let languages = vec![/* ... */];
/// let test_matcher = Arc::new(PatternMatcher::new());
///
/// let engine = DetectionEngineBuilder::new(languages)
///     .with_pattern_matcher(test_matcher)
///     .build();
/// ```
pub struct DetectionEngineBuilder {
    languages: Vec<ProjectIndicator>,
    config: DetectionConfig,

    // Optional injected components
    pattern_matcher: Option<Arc<PatternMatcher>>,
    cache_manager: Option<FileSystemCacheManager>,
    confidence_scorer: Option<ConfidenceScorer>,
    language_resolver: Option<LanguageResolver>,
    framework_detector: Option<FrameworkDetector>,
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
            pattern_matcher: None,
            cache_manager: None,
            confidence_scorer: None,
            language_resolver: None,
            framework_detector: None,
            result_tracker: None,
        }
    }

    /// Sets a custom detection configuration.
    pub fn with_config(mut self, config: DetectionConfig) -> Self {
        self.config = config;
        self
    }

    /// Injects a custom pattern matcher.
    ///
    /// Useful for:
    /// - Testing with pre-populated caches
    /// - Sharing a matcher across multiple engines
    /// - Custom pattern matching behavior
    pub fn with_pattern_matcher(mut self, matcher: Arc<PatternMatcher>) -> Self {
        self.pattern_matcher = Some(matcher);
        self
    }

    /// Injects a custom cache manager.
    ///
    /// Useful for:
    /// - Custom TTL configurations
    /// - Testing with controlled cache behavior
    /// - Sharing cache across multiple engines
    pub fn with_cache_manager(mut self, cache: FileSystemCacheManager) -> Self {
        self.cache_manager = Some(cache);
        self
    }

    /// Injects a custom confidence scorer.
    ///
    /// Useful for:
    /// - Testing with custom scoring algorithms
    /// - Custom confidence thresholds
    pub fn with_confidence_scorer(mut self, scorer: ConfidenceScorer) -> Self {
        self.confidence_scorer = Some(scorer);
        self
    }

    /// Injects a custom language resolver.
    ///
    /// Useful for:
    /// - Testing language conflict resolution
    /// - Custom resolution strategies
    pub fn with_language_resolver(mut self, resolver: LanguageResolver) -> Self {
        self.language_resolver = Some(resolver);
        self
    }

    /// Injects a custom framework detector.
    ///
    /// Useful for:
    /// - Testing framework detection
    /// - Custom framework detection logic
    pub fn with_framework_detector(mut self, detector: FrameworkDetector) -> Self {
        self.framework_detector = Some(detector);
        self
    }

    /// Inject a result tracker for recording detection history.
    ///
    /// If not provided, tracking is disabled (zero overhead).
    pub fn with_result_tracker(mut self, tracker: Arc<ResultTracker>) -> Self {
        self.result_tracker = Some(tracker);
        self
    }

    /// Builds the `DetectionEngine` with configured or default components.
    ///
    /// ## Component Dependencies
    ///
    /// Components have dependencies on each other:
    /// - `ConfidenceScorer` needs `PatternMatcher`
    /// - `ScanningEngine` needs `PatternMatcher` and `FileSystemCache`
    ///
    /// The builder handles these dependencies automatically:
    /// - If you provide a custom `PatternMatcher`, it's shared with all components
    /// - If you don't provide one, a default is created and shared
    pub fn build(self) -> DetectionEngine {
        let languages: Vec<Arc<ProjectIndicator>> =
            self.languages.into_iter().map(Arc::new).collect();

        // Use provided components or create defaults
        let pattern_matcher = self
            .pattern_matcher
            .unwrap_or_else(|| Arc::new(PatternMatcher::new()));

        let cache_manager = self.cache_manager.unwrap_or_default();

        let confidence_scorer = self
            .confidence_scorer
            .unwrap_or_else(|| ConfidenceScorer::with_pattern_matcher(pattern_matcher.clone()));

        let language_resolver = self.language_resolver.unwrap_or_default();

        let framework_detector = self.framework_detector.unwrap_or_default();

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
            let metadata = DetectionMetadata {
                duration_micros: detection_start.elapsed().as_micros() as u64,
                from_cache: false,
                cached_at: None,
                pattern_cache_hits: 0, // Pattern matcher cache not directly accessible
                pattern_cache_misses: 0,
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
            .scan_matching_files(&scan_path)
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
        let metadata = DetectionMetadata {
            duration_micros: detection_start.elapsed().as_micros() as u64,
            from_cache: false,
            cached_at: None,
            pattern_cache_hits: 0,
            pattern_cache_misses: 0,
            fs_cache_hits: fs_hits as usize,
            fs_cache_misses: fs_misses as usize,
        };
        self.record_detection(&result, path, metadata)?;

        Ok(result)
    }

    /// Detect project with persistent caching support.
    ///
    /// This method populates the cache with relevant files before detection,
    /// enabling faster subsequent detections for the same project structure.
    pub fn detect_cached(
        &mut self,
        path: &Path,
        cache: &DetectionCache,
    ) -> Result<DetectionResult> {
        // PERFORMANCE OPTIMIZATION: Check cache FIRST before doing expensive dynamic_relevant work
        // This avoids iterating through all languages/frameworks on cache hits
        let cached_result = cache.get_with_metadata(path)?;

        if let Some((cached, created_at)) = cached_result {
            // Result is from cache - record snapshot if tracking is enabled
            // Convert SystemTime to Unix timestamp
            let cached_at_timestamp = created_at
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs());

            let metadata = DetectionMetadata {
                duration_micros: CACHED_DETECTION_DURATION_MICROS,
                from_cache: true,
                cached_at: cached_at_timestamp,
                pattern_cache_hits: 0,
                pattern_cache_misses: 0,
                fs_cache_hits: 0,
                fs_cache_misses: 0,
            };
            self.record_detection(&cached, path, metadata)?;

            return Ok(cached);
        }

        // Cache miss - prepare dynamic relevant files before fresh detection
        cache.clear_dynamic_relevant();

        for lang in &self.languages {
            for pat in &lang.files {
                if !pat.contains('*') {
                    cache.add_dynamic_relevant(pat.clone());
                }
            }
            for fw in &lang.frameworks {
                match &fw.detection {
                    DetectionType::NodeEcosystem { .. } => cache.add_dynamic_relevant(PACKAGE_JSON),
                    DetectionType::RustEcosystem { .. } => cache.add_dynamic_relevant(CARGO_TOML),
                    DetectionType::GoEcosystem { .. } => cache.add_dynamic_relevant(GO_MOD),
                    DetectionType::PythonEcosystem { .. } => {
                        cache.add_dynamic_relevant(PYPROJECT_TOML)
                    }
                    DetectionType::PHPEcosystem { .. } => cache.add_dynamic_relevant(COMPOSER_JSON),
                    DetectionType::RubyEcosystem { .. } => cache.add_dynamic_relevant(GEMFILE),
                    DetectionType::JavaEcosystem { .. } => {}
                    DetectionType::DotNetEcosystem { .. } => {}
                    DetectionType::ScalaEcosystem { .. } => cache.add_dynamic_relevant(BUILD_SBT),
                    DetectionType::DartEcosystem { .. } => cache.add_dynamic_relevant(PUBSPEC_YAML),
                    _ => {}
                }
            }
        }

        // Not cached, do fresh detection
        let result = self.detect(path)?;

        // Store result in cache for future lookups
        cache.put(path, result.clone())?;

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
