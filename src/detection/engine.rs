use crate::detection::caches::ParsedFileCache;
use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::detection::framework_detector;
use crate::detection::indicator_resolver::IndicatorResolver;
use crate::detection::pattern_processor::PatternProcessor;
use crate::detection::root_indicators::RootIndicatorEngine;
use crate::detection::scanner::ScanningEngine;
use crate::performance::FileSystemCache;
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

/// Full-scan scores below this are ambiguous enough for a decisive root
/// indicator to override them (see `detect()`); the same boundary as the
/// confident tier in `IndicatorResolver`.
const ROOT_MATCH_OVERRIDE_AMBIGUITY_CEILING: f32 = 0.5;

/// Root-indicator certainty required to override an ambiguous full-scan
/// result. Above what the data-driven `weight × base_priority` fallback can
/// reach for shared Node root indicators (lockfiles top out at 0.72), so only
/// the structurally validated certainties in `calculate_indicator_certainty`
/// qualify (Cargo.toml/go.mod 0.95, pyproject/tsconfig 0.90, TypeScript
/// dependency 0.85) — not a tie over a shared lockfile.
const ROOT_MATCH_OVERRIDE_CERTAINTY: f32 = 0.8;

/// Detects the project type and frameworks of one directory: root discovery,
/// scan, indicator resolution, confidence, then framework detection.
///
/// ```rust
/// # use project_indicator::detection::engine::DetectionEngine;
/// let engine = DetectionEngine::new(vec![/* indicators */], vec![]);
/// ```
pub struct DetectionEngine {
    indicators: Vec<Arc<Indicator>>,
    frameworks: Arc<Vec<Framework>>,

    file_cache: FileSystemCache,
    parsed_cache: ParsedFileCache,
    confidence_scorer: ConfidenceScorer,
    indicator_resolver: IndicatorResolver,
    scanning_engine: ScanningEngine,
    root_indicator_engine: RootIndicatorEngine,
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
        let indicators: Vec<Arc<Indicator>> = indicators.into_iter().map(Arc::new).collect();
        let frameworks = Arc::new(frameworks);

        let file_cache = FileSystemCache::default();
        let parsed_cache = ParsedFileCache::new();
        let confidence_scorer = ConfidenceScorer::with_catalog(frameworks.clone());
        let indicator_resolver = IndicatorResolver::with_catalog(frameworks.clone());

        // Deduplicated set of every file pattern the indicators reference
        let unique_patterns: Arc<Vec<String>> = Arc::new(
            indicators
                .iter()
                .flat_map(|i| i.files.iter().cloned())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect(),
        );

        let scanning_engine = ScanningEngine::new(
            PatternProcessor::new(unique_patterns),
            detection_config.max_depth,
        );

        let root_indicator_engine =
            RootIndicatorEngine::from_parts(indicators.clone(), detection_config);

        Self {
            indicators,
            frameworks,
            file_cache,
            parsed_cache,
            confidence_scorer,
            indicator_resolver,
            scanning_engine,
            root_indicator_engine,
        }
    }

    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        let mut evidence = DetectionEvidence::new();

        if self.root_indicator_engine.is_boundary_directory(path) {
            log::debug!(
                "Refusing to scan from boundary directory: {}",
                path.display()
            );
            return Ok(no_match(evidence));
        }

        let mut root_indicator_match = None;
        let scan_path = if let Some((root_path, root_indicator)) = self
            .root_indicator_engine
            .find_project_root(path, &self.file_cache)
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
                root_indicator.pattern.clone(),
                root_indicator.certainty,
            ));

            root_indicator_match = Some(root_indicator);
            root_path
        } else {
            log::debug!("No project root found via upward traversal, scanning from current path");
            path.to_path_buf()
        };

        let scan_start = std::time::Instant::now();

        let outcome = self
            .scanning_engine
            .scan_with_status(&scan_path)
            .with_context(|| format!("Failed to scan files in path: {}", scan_path.display()))?;
        let detailed_files = outcome.matches;
        evidence.scan_truncated = outcome.truncated;
        evidence.watched_dirs = outcome.watched_dirs;

        evidence.set_scan_metrics(
            detailed_files.len(),
            scan_start.elapsed().as_millis() as u64,
        );

        if detailed_files.is_empty() {
            return Ok(no_match(evidence));
        }

        let detected_indicator = self
            .indicator_resolver
            .detect_indicator_with_conflict_resolution_and_evidence(
                &self.indicators,
                &detailed_files,
                &mut evidence,
            );

        let Some(mut indicator) = detected_indicator else {
            return Ok(no_match(evidence));
        };

        let mut confidence = self
            .confidence_scorer
            .calculate_indicator_score_with_evidence(
                &indicator,
                &detailed_files,
                &mut evidence,
                &self.indicators,
            );

        // The full scan only reasons about file existence, so it can't see
        // content-aware evidence (e.g. a "typescript" devDependency with no
        // tsconfig.json yet). find_project_root already computed a
        // content-aware certainty for this same root to locate scan_path;
        // when the full scan is ambiguous and that certainty decisively
        // disagrees, trust it instead of discarding it.
        if let Some(root_match) = &root_indicator_match {
            if root_match.indicator.name != indicator.name
                && confidence < ROOT_MATCH_OVERRIDE_AMBIGUITY_CEILING
                && root_match.certainty >= ROOT_MATCH_OVERRIDE_CERTAINTY
            {
                log::debug!(
                    "Overriding ambiguous full-scan indicator '{}' ({:.3}) with root-indicator match '{}' ({:.3})",
                    indicator.name,
                    confidence,
                    root_match.indicator.name,
                    root_match.certainty
                );
                evidence.add_confidence_factor(crate::types::ConfidenceFactor::new(
                    "root_indicator_override".to_string(),
                    root_match.certainty,
                    1.0,
                    format!(
                        "Overrode ambiguous full-scan result ({}) with content-aware root indicator match ({})",
                        indicator.name, root_match.indicator.name
                    ),
                ));
                indicator = root_match.indicator.clone();
                confidence = root_match.certainty;
            }
        }

        // Framework detection is not gated on confidence: a framework
        // dependency in a manifest is evidence in its own right
        let frameworks = framework_detector::detect_frameworks_with_evidence(
            &scan_path,
            &indicator,
            &self.frameworks,
            &mut evidence,
            &self.file_cache,
            &self.parsed_cache,
        );

        evidence.read_files = self
            .parsed_cache
            .read_paths()
            .iter()
            .filter_map(|path| path.strip_prefix(&scan_path).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .collect();

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

        Ok(DetectionResult::new_with_evidence(
            Some(indicator),
            frameworks,
            floored,
            evidence,
        ))
    }
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
