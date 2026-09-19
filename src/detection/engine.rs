use crate::constants::*;
use crate::detection::caches::FileSystemCacheManager;
use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::detection::framework_detector::FrameworkDetector;
use crate::detection::indicator_resolver::IndicatorResolver;
use crate::detection::pattern_compiler::PatternCompiler;
use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::detection::root_indicators::RootIndicatorEngine;
use crate::detection::scanner::ScanningEngine;
use crate::types::{DetectionConfig, DetectionEvidence, DetectionResult, Framework, Indicator};
use crate::Result;
use anyhow::Context;
use std::path::Path;
use std::sync::Arc;

/// A matched root indicator (e.g. Cargo.toml, weight 0.95) floors confidence
/// at weight × this factor.
const ROOT_MATCH_CONFIDENCE_FACTOR: f32 = 0.9;

/// A confirmed framework dependency floors confidence here: the dependency
/// exists in a real manifest, which identifies the project near-certainly.
const FRAMEWORK_MATCH_CONFIDENCE_FLOOR: f32 = 0.75;

/// Main detection engine for identifying project indicators and frameworks.
///
/// The DetectionEngine coordinates multiple specialized components to analyze
/// a project directory and determine its primary indicator and frameworks.
///
/// ## Construction
///
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngine;
/// # use project_indicator::types::Indicator;
/// let indicators = vec![/* ... */];
/// let engine = DetectionEngine::new(indicators, vec![]);
/// ```
///
/// ## Architecture
///
/// - **PatternCompiler**: Extracts and compiles file patterns from indicator definitions
/// - **FileSystemCacheManager**: Manages file existence and parsed file caches
/// - **ScanningEngine**: Performs directory traversal and file matching with adaptive performance
/// - **IndicatorResolver**: Resolves indicator conflicts when multiple indicators detected
/// - **ConfidenceScorer**: Calculates confidence scores for indicator matches
/// - **FrameworkDetector**: Identifies frameworks within detected indicators
/// - **RootIndicatorEngine**: Fast path detection using root indicator files
///
/// ## Shared Resources
///
/// ### PatternMatcher Ownership
///
/// DetectionEngine creates a single `Arc<PatternMatcher>` instance and shares it with:
/// - `ConfidenceScorer` - for calculating indicator match confidence
/// - `ScanningEngine` (via `PatternProcessor`) - for efficient file pattern matching
///
/// This design ensures:
/// 1. **Single cache instance** - All pattern matches benefit from shared cache
/// 2. **Memory efficiency** - No duplicate matcher instances
/// 3. **Thread safety** - PatternMatcher uses DashMap for concurrent access
/// 4. **Performance** - Cache hits across entire detection pipeline
pub struct DetectionEngine {
    indicators: Vec<Arc<Indicator>>,
    frameworks: Arc<Vec<Framework>>,

    // Specialized components
    cache_manager: FileSystemCacheManager,
    confidence_scorer: ConfidenceScorer,
    indicator_resolver: IndicatorResolver,
    framework_detector: FrameworkDetector,
    scanning_engine: ScanningEngine,
    root_indicator_engine: RootIndicatorEngine,
}

impl DetectionEngine {
    /// Creates a new `DetectionEngine` with default configuration.
    pub fn new(indicators: Vec<Indicator>, frameworks: Vec<Framework>) -> Self {
        Self::with_config(indicators, frameworks, DetectionConfig::default())
    }

    /// Creates a new `DetectionEngine` with custom detection configuration.
    ///
    /// A single `PatternMatcher` is created and shared across the scorer and
    /// scanner so pattern-match memoization works across the whole pipeline.
    pub fn with_config(
        indicators: Vec<Indicator>,
        frameworks: Vec<Framework>,
        detection_config: DetectionConfig,
    ) -> Self {
        let indicators: Vec<Arc<Indicator>> = indicators.into_iter().map(Arc::new).collect();
        let frameworks = Arc::new(frameworks);

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let cache_manager = FileSystemCacheManager::new();
        let confidence_scorer =
            ConfidenceScorer::with_catalog(pattern_matcher.clone(), frameworks.clone());
        let indicator_resolver = IndicatorResolver::default();
        let framework_detector = FrameworkDetector::new();

        // Create pattern compiler and scanning engine
        let pattern_compiler = PatternCompiler::new(&indicators);
        let file_cache = cache_manager.file_existence_cache();

        let scanning_engine = ScanningEngine::with_cache(
            PatternProcessor::new(
                pattern_matcher.clone(),
                pattern_compiler.unique_patterns(),
                indicators.clone(),
            ),
            detection_config.max_depth,
            Some(file_cache),
        );

        let root_indicator_engine = RootIndicatorEngine::from_parts(
            indicators.clone(),
            frameworks.clone(),
            detection_config,
        );

        Self {
            indicators,
            frameworks,
            cache_manager,
            confidence_scorer,
            indicator_resolver,
            framework_detector,
            scanning_engine,
            root_indicator_engine,
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
                    .indicator
                    .as_ref()
                    .map(|l| l.name.as_str())
                    .unwrap_or("Unknown"),
                early_result.confidence,
                detection_start.elapsed().as_millis()
            );

            // Framework detection is not gated on confidence: a framework
            // dependency in a manifest is evidence in its own right
            let frameworks = if let Some(ref indicator) = early_result.indicator {
                self.framework_detector
                    .detect_frameworks_with_evidence(
                        &scan_path,
                        indicator,
                        &self.frameworks,
                        &mut evidence,
                        &self.cache_manager.file_existence_cache(),
                        self.cache_manager.parsed_file_cache(),
                    )
                    .with_context(|| "Failed to detect frameworks")?
            } else {
                Vec::new()
            };

            let mut result = early_result;
            result.frameworks = frameworks;
            result.evidence = evidence;
            // Same framework-evidence floor as the full-scan path (the root
            // floor is implicit here: early termination is root-driven)
            if !result.frameworks.is_empty() {
                result.confidence = result.confidence.max(FRAMEWORK_MATCH_CONFIDENCE_FLOOR);
            }

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

        let detected_indicator = self
            .indicator_resolver
            .detect_indicator_with_conflict_resolution_and_evidence(
                &self.indicators,
                &detailed_files,
                &mut evidence,
            );

        let indicator = match detected_indicator {
            Some(indicator) => indicator,
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
            .calculate_indicator_score_with_evidence(
                &indicator,
                &detailed_files,
                &mut evidence,
                &self.indicators,
            );

        // Framework detection is not gated on confidence: a framework
        // dependency in a manifest is evidence in its own right
        let frameworks = self
            .framework_detector
            .detect_frameworks_with_evidence(
                &scan_path,
                &indicator,
                &self.frameworks,
                &mut evidence,
                &self.cache_manager.file_existence_cache(),
                self.cache_manager.parsed_file_cache(),
            )
            .with_context(|| "Failed to detect frameworks")?;

        // The ratio score punishes indicators with many patterns for files
        // they don't have, so a canonical project can read implausibly low.
        // Floor the displayed confidence by the strongest evidence found: a
        // matched root manifest, or a confirmed framework dependency.
        let root_floor = self
            .confidence_scorer
            .strongest_root_match(&indicator, &detailed_files)
            * ROOT_MATCH_CONFIDENCE_FACTOR;
        let framework_floor = if frameworks.is_empty() {
            0.0
        } else {
            FRAMEWORK_MATCH_CONFIDENCE_FLOOR
        };
        let floored = confidence.max(root_floor).max(framework_floor).min(1.0);
        if floored > confidence {
            evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                "confidence_floor".to_string(),
                floored,
                1.0,
                "Confidence floored by root manifest / framework evidence".to_string(),
            ));
        }

        let result =
            DetectionResult::new_with_evidence(Some(indicator), frameworks, floored, evidence);

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

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator;
    use crate::test_utils::create_test_rust_project;

    #[test]
    fn test_detection_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngine::new(indicators, vec![]);

        assert_eq!(engine.indicators.len(), 1);
        assert_eq!(engine.indicators[0].name, "Rust");
        Ok(())
    }

    #[test]
    fn test_detection_engine_with_config() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"])];
        let config = DetectionConfig::default();
        let engine = DetectionEngine::with_config(indicators, vec![], config);

        assert_eq!(engine.indicators.len(), 1);
        // Pattern compilation is handled internally during initialization
        Ok(())
    }

    #[test]
    fn test_detect_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"])];
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
    fn test_detect_no_match() -> Result<(), Box<dyn std::error::Error>> {
        let indicators = vec![create_test_indicator(
            "Python",
            vec!["*.py", "requirements.txt"],
        )];
        let engine = DetectionEngine::new(indicators, vec![]);
        let temp_dir = create_test_rust_project()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.indicator.is_none());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }
}
