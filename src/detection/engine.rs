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
use crate::types::{
    DetectionConfig, DetectionEvidence, DetectionResult, DetectionType, ProjectIndicator,
};
use crate::Result;
use anyhow::Context;
use std::path::Path;
use std::sync::Arc;

/// Main detection engine for identifying project languages and frameworks.
///
/// The DetectionEngine coordinates multiple specialized components to analyze
/// a project directory and determine its primary language and frameworks.
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
}

impl DetectionEngine {
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self::with_config(languages, DetectionConfig::default())
    }

    pub fn with_config(
        languages: Vec<ProjectIndicator>,
        detection_config: DetectionConfig,
    ) -> Self {
        let languages: Vec<Arc<ProjectIndicator>> = languages.into_iter().map(Arc::new).collect();

        // Create a single shared PatternMatcher instance for all components
        let shared_pattern_matcher = Arc::new(PatternMatcher::new());

        // Create specialized components
        let pattern_compiler = PatternCompiler::new(&languages);
        let cache_manager = FileSystemCacheManager::new();

        // Share file existence cache with ScanningEngine for performance
        // This enables ~7-8x speedup for repeated scans (e.g., shell prompts)
        let file_cache = cache_manager.file_existence_cache();

        let scanning_engine = ScanningEngine::with_cache(
            PatternProcessor::new(
                shared_pattern_matcher.clone(),
                pattern_compiler.unique_patterns(),
                languages.clone(),
            ),
            detection_config.max_depth,
            Some(file_cache),
        );

        Self {
            languages: languages.clone(),
            config: detection_config.clone(),
            cache_manager,
            confidence_scorer: ConfidenceScorer::with_pattern_matcher(
                shared_pattern_matcher.clone(),
            ),
            language_resolver: LanguageResolver::new(),
            framework_detector: FrameworkDetector::new(),
            scanning_engine,
            root_indicator_engine: RootIndicatorEngine::from_arc_languages(
                languages,
                detection_config,
            ),
        }
    }

    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        let mut evidence = DetectionEvidence::new();
        let detection_start = std::time::Instant::now();

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

        Ok(DetectionResult::new_with_evidence(
            Some(language),
            frameworks,
            confidence,
            evidence,
        ))
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

        self.detect(path)
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
        let engine = DetectionEngine::new(languages);

        assert_eq!(engine.languages.len(), 1);
        assert_eq!(engine.languages[0].name, "Rust");
        Ok(())
    }

    #[test]
    fn test_detection_engine_with_config() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let config = DetectionConfig::default();
        let engine = DetectionEngine::with_config(languages, config);

        assert_eq!(engine.languages.len(), 1);
        // Pattern compilation is handled internally during initialization
        Ok(())
    }

    #[test]
    fn test_detect_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngine::new(languages);
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
        let engine = DetectionEngine::new(languages);
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.language.is_none());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }
}
