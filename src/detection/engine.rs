use crate::constants::{
    EARLY_TERMINATION, EARLY_TERMINATION_MSG, FRAMEWORK_DETECTION_SKIPPED,
    FRAMEWORK_DETECTION_SKIPPED_MSG,
};
use crate::detection::cache::{CachedDetection, DetectionCache};
use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::detection::file_scanner::FileScanner;
use crate::detection::framework_detector::FrameworkDetector;
use crate::detection::language_resolver::LanguageResolver;
use crate::detection::parsed_file_cache::ParsedFileCache;
use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::root_indicators::RootIndicatorEngine;
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionConfig, DetectionEvidence, DetectionResult, DetectionType, MatchedFile,
    ProjectIndicator,
};
use crate::Result;
use anyhow::Context;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct DetectionEngine {
    languages: Vec<Arc<ProjectIndicator>>,
    pattern_cache: HashMap<String, Regex>,
    unique_patterns: Vec<String>,
    config: DetectionConfig,

    file_existence_cache: FileSystemCache,
    parsed_file_cache: ParsedFileCache,

    confidence_scorer: ConfidenceScorer,
    language_resolver: LanguageResolver,
    framework_detector: FrameworkDetector,
    file_scanner: FileScanner,
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
        let shared_pattern_matcher = Arc::new(PatternMatcher::new());

        let mut engine = Self {
            languages: languages.clone(),
            pattern_cache: HashMap::new(),
            unique_patterns: Vec::new(),
            config: detection_config.clone(),
            file_existence_cache: FileSystemCache::new(300, 10000),
            parsed_file_cache: ParsedFileCache::new(),
            confidence_scorer: ConfidenceScorer::with_pattern_matcher(
                shared_pattern_matcher.clone(),
            ),
            language_resolver: LanguageResolver::new(),
            framework_detector: FrameworkDetector::new(),
            file_scanner: FileScanner::new(Vec::new()),
            root_indicator_engine: RootIndicatorEngine::from_arc_languages(
                languages.clone(),
                detection_config,
            ),
        };
        engine.precompute_patterns();
        engine
    }

    fn precompute_patterns(&mut self) {
        let mut all_patterns = std::collections::HashSet::new();

        for language in &self.languages {
            for pattern in &language.files {
                all_patterns.insert(pattern.clone());
            }
        }

        self.unique_patterns = all_patterns.into_iter().collect();

        self.file_scanner = FileScanner::with_languages_and_depth(
            self.unique_patterns.clone(),
            self.languages.clone(),
            self.config.max_depth,
        );

        for pattern in &self.unique_patterns {
            if pattern.contains('*') {
                if let Some(regex_pattern) = crate::patterns::pattern_to_regex(pattern) {
                    if let Ok(regex) = regex::Regex::new(&regex_pattern) {
                        self.pattern_cache.insert(pattern.clone(), regex);
                    }
                }
            }
        }
    }

    pub fn cached_file_exists(&self, path: &Path) -> bool {
        self.file_existence_cache.exists(path)
    }

    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        let mut evidence = DetectionEvidence::new();
        let detection_start = std::time::Instant::now();

        if let Some(early_result) = self
            .root_indicator_engine
            .detect_with_early_termination(path, &self.file_existence_cache)
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
                            path,
                            language,
                            &mut evidence,
                            &self.file_existence_cache,
                            &self.parsed_file_cache,
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
            .file_scanner
            .scan_matching_files(path)
            .with_context(|| format!("Failed to scan files in path: {}", path.display()))?;

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
                    path,
                    &language,
                    &mut evidence,
                    &self.file_existence_cache,
                    &self.parsed_file_cache,
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

    pub fn languages_by_priority(&self) -> Vec<&Arc<ProjectIndicator>> {
        let mut languages: Vec<&Arc<ProjectIndicator>> = self.languages.iter().collect();
        languages.sort_by_key(|lang| lang.priority);
        languages
    }

    pub fn find_language(&self, name: &str) -> Option<&Arc<ProjectIndicator>> {
        self.languages
            .iter()
            .find(|lang| lang.name.eq_ignore_ascii_case(name))
    }

    pub fn all_file_patterns(&self) -> Vec<&String> {
        self.languages.iter().flat_map(|lang| &lang.files).collect()
    }

    pub fn batch_collect_files(
        &self,
        base_path: &Path,
        patterns: &[String],
    ) -> Result<Vec<MatchedFile>> {
        let pattern_refs: Vec<&String> = patterns.iter().collect();
        self.file_scanner
            .batch_collect_files(base_path, &pattern_refs)
    }

    pub fn get_performance_stats(&self) -> crate::performance::CacheStats {
        self.file_scanner.get_performance_stats()
    }

    pub fn clear_caches(&mut self) {
        self.file_scanner.clear_caches();
        self.file_existence_cache.clear();
        self.parsed_file_cache.clear();
    }

    pub fn get_root_indicator_stats(
        &self,
    ) -> crate::detection::root_indicators::RootIndicatorStats {
        self.root_indicator_engine.get_stats()
    }
}

impl CachedDetection for DetectionEngine {
    fn detect_cached(&mut self, path: &Path, cache: &DetectionCache) -> Result<DetectionResult> {
        cache.clear_dynamic_relevant();

        for lang in &self.languages {
            for pat in &lang.files {
                if !pat.contains('*') {
                    cache.add_dynamic_relevant(pat.clone());
                }
            }
            for fw in &lang.frameworks {
                match &fw.detection {
                    DetectionType::NodeEcosystem { .. } => {
                        cache.add_dynamic_relevant("package.json")
                    }
                    DetectionType::RustEcosystem { .. } => cache.add_dynamic_relevant("Cargo.toml"),
                    DetectionType::GoEcosystem { .. } => cache.add_dynamic_relevant("go.mod"),
                    DetectionType::PythonEcosystem { .. } => {
                        cache.add_dynamic_relevant("pyproject.toml")
                    }
                    DetectionType::PHPEcosystem { .. } => {
                        cache.add_dynamic_relevant("composer.json")
                    }
                    DetectionType::RubyEcosystem { .. } => cache.add_dynamic_relevant("Gemfile"),
                    DetectionType::JavaEcosystem { .. } => {}
                    DetectionType::DotNetEcosystem { .. } => {}
                    DetectionType::ScalaEcosystem { .. } => cache.add_dynamic_relevant("build.sbt"),
                    DetectionType::DartEcosystem { .. } => {
                        cache.add_dynamic_relevant("pubspec.yaml")
                    }
                    _ => {}
                }
            }
        }

        self.detect(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use crate::detection::matchers::test_helpers::helpers::create_test_language;

    fn create_test_directory() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"")?;
        fs::write(root.join("main.rs"), "fn main() {}")?;

        let src_dir = root.join("src");
        fs::create_dir(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "// lib")?;

        Ok(temp_dir)
    }

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
        assert!(!engine.unique_patterns.is_empty());
        Ok(())
    }

    #[test]
    fn test_detect_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngine::new(languages);
        let temp_dir = create_test_directory()?;

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
        let temp_dir = create_test_directory()?;

        let result = engine.detect(temp_dir.path())?;

        assert!(result.language.is_none());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }

    #[test]
    fn test_languages_by_priority() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language("Low Priority", vec!["*.low"]),
            create_test_language("High Priority", vec!["*.high"]),
        ];
        let mut lang1 = languages[0].clone();
        let mut lang2 = languages[1].clone();
        lang1.priority = 10;
        lang2.priority = 1;

        let engine = DetectionEngine::new(vec![lang1, lang2]);
        let sorted = engine.languages_by_priority();

        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].priority, 1);
        assert_eq!(sorted[1].priority, 10);
        Ok(())
    }

    #[test]
    fn test_find_language() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language("Rust", vec!["*.rs"]),
            create_test_language("JavaScript", vec!["*.js"]),
        ];
        let engine = DetectionEngine::new(languages);

        assert!(engine.find_language("rust").is_some());
        assert!(engine.find_language("Rust").is_some());
        assert!(engine.find_language("RUST").is_some());
        assert!(engine.find_language("nonexistent").is_none());
        Ok(())
    }

    #[test]
    fn test_all_file_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language("Rust", vec!["Cargo.toml", "*.rs"]),
            create_test_language("JavaScript", vec!["package.json", "*.js"]),
        ];
        let engine = DetectionEngine::new(languages);

        let patterns = engine.all_file_patterns();
        assert_eq!(patterns.len(), 4);
        assert!(patterns.iter().any(|p| *p == "Cargo.toml"));
        assert!(patterns.iter().any(|p| *p == "*.rs"));
        assert!(patterns.iter().any(|p| *p == "package.json"));
        assert!(patterns.iter().any(|p| *p == "*.js"));
        Ok(())
    }

    #[test]
    fn test_batch_collect_files() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["Cargo.toml", "*.rs"])];
        let engine = DetectionEngine::new(languages);
        let temp_dir = create_test_directory()?;

        let patterns = vec!["Cargo.toml".to_string(), "*.rs".to_string()];
        let files = engine.batch_collect_files(temp_dir.path(), &patterns)?;

        assert!(!files.is_empty());
        let filenames: Vec<&str> = files.iter().map(|f| f.filename.as_str()).collect();
        assert!(filenames.contains(&"Cargo.toml"));
        Ok(())
    }

    #[test]
    fn test_performance_stats() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["*.rs"])];
        let engine = DetectionEngine::new(languages);

        let stats = engine.get_performance_stats();
        assert_eq!(stats.metadata_entries, 0);
        assert_eq!(stats.metadata_capacity, 10000);
        Ok(())
    }

    #[test]
    fn test_clear_caches() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language("Rust", vec!["*.rs"])];
        let mut engine = DetectionEngine::new(languages);

        engine.clear_caches();

        let stats = engine.get_performance_stats();
        assert_eq!(stats.metadata_entries, 0);
        Ok(())
    }
}
