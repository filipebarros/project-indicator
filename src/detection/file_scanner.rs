use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::detection::scanning_engine::{ScanConfig, ScanningEngine};
use crate::types::{MatchedFile, ProjectIndicator};
use crate::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub struct FileScanner {
    scanning_engine: ScanningEngine,
    pattern_processor: PatternProcessor,
    languages: Vec<Arc<ProjectIndicator>>,
    max_depth: usize,
}

impl FileScanner {
    pub fn new(patterns: Vec<String>) -> Self {
        Self::with_max_depth(patterns, 3)
    }

    pub fn with_max_depth(patterns: Vec<String>, max_depth: usize) -> Self {
        Self::with_languages_and_depth(patterns, Vec::new(), max_depth)
    }

    pub fn with_languages(patterns: Vec<String>, languages: Vec<Arc<ProjectIndicator>>) -> Self {
        Self::with_languages_and_depth(patterns, languages, 3)
    }

    pub fn with_languages_and_depth(
        patterns: Vec<String>,
        languages: Vec<Arc<ProjectIndicator>>,
        max_depth: usize,
    ) -> Self {
        let pattern_matcher = Arc::new(PatternMatcher::new());

        let scanning_engine = ScanningEngine::new(
            pattern_matcher.clone(),
            patterns.clone(),
            languages.clone(),
            max_depth,
        );

        let pattern_processor = PatternProcessor::new(pattern_matcher, patterns, languages.clone());

        Self {
            scanning_engine,
            pattern_processor,
            languages,
            max_depth,
        }
    }

    pub fn scan_matching_files(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        let config = ScanConfig {
            max_depth: self.max_depth,
            timeout: Duration::from_millis(500),
            enable_parallel: true,
            early_termination_threshold: 2,
            max_matches: Some(100),
            batch_size: 50,
            enable_memory_efficient_mode: false,
        };

        self.scanning_engine.scan_files(path, &config)
    }

    pub fn batch_collect_files(
        &self,
        path: &Path,
        _patterns: &[&String],
    ) -> Result<Vec<MatchedFile>> {
        let config = ScanConfig {
            max_depth: self.max_depth,
            timeout: Duration::from_millis(500),
            enable_parallel: true,
            early_termination_threshold: 2,
            max_matches: Some(100),
            batch_size: 50,
            enable_memory_efficient_mode: false,
        };

        self.scanning_engine.scan_files(path, &config)
    }

    pub fn get_pattern_importance(&self, pattern: &str) -> f32 {
        self.pattern_processor
            .get_pattern_importance(pattern, &self.languages)
    }

    pub fn should_scan_file(&self, filename: &str) -> bool {
        self.pattern_processor.should_scan_file(filename)
    }

    pub fn is_strong_evidence(&self, filename: &str) -> bool {
        self.pattern_processor.is_strong_evidence(filename)
    }

    pub fn clear_caches(&mut self) {
        self.scanning_engine.clear_caches();
    }

    pub fn get_performance_stats(&self) -> crate::performance::CacheStats {
        self.scanning_engine.get_performance_stats()
    }

    pub fn get_unique_patterns(&self) -> &[String] {
        self.pattern_processor.get_patterns()
    }

    pub fn get_high_priority_files(&self) -> &std::collections::HashSet<String> {
        self.pattern_processor.get_high_priority_files()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_scanner() -> FileScanner {
        use crate::types::{IndicatorContext, ProjectIndicator, RootIndicator};

        let rust_language = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string(), "Cargo.toml".to_string()],
            "#DEA584".to_string(),
            "🦀".to_string(),
            1,
            vec![],
            vec![RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 0.95,
                context: IndicatorContext::LanguageRoot,
            }],
        );

        FileScanner::with_languages(
            vec![
                "*.rs".to_string(),
                "Cargo.toml".to_string(),
                "package.json".to_string(),
                "*.js".to_string(),
            ],
            vec![Arc::new(rust_language)],
        )
    }

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
    fn test_file_scanner_creation() -> Result<(), Box<dyn std::error::Error>> {
        let scanner = FileScanner::new(vec!["*.rs".to_string()]);
        assert!(!scanner.get_unique_patterns().is_empty());
        Ok(())
    }

    #[test]
    fn test_scan_matching_files() -> Result<(), Box<dyn std::error::Error>> {
        let scanner = create_test_scanner();
        let temp_dir = create_test_directory()?;

        let files = scanner.scan_matching_files(temp_dir.path())?;

        assert!(!files.is_empty(), "Should find matching files");

        let filenames: Vec<&str> = files.iter().map(|f| f.filename.as_str()).collect();
        assert!(filenames.contains(&"Cargo.toml"));
        Ok(())
    }

    #[test]
    fn test_empty_directory_scan() -> Result<(), Box<dyn std::error::Error>> {
        let scanner = create_test_scanner();
        let temp_dir = TempDir::new()?;

        let files = scanner.scan_matching_files(temp_dir.path())?;
        assert!(files.is_empty(), "Empty directory should return no files");
        Ok(())
    }

    #[test]
    fn test_clear_caches() -> Result<(), Box<dyn std::error::Error>> {
        let mut scanner = create_test_scanner();
        scanner.clear_caches();
        Ok(())
    }
}
