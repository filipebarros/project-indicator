use crate::detection::pattern_matching::PatternMatcher;
use crate::types::{MatchedFile, ProjectIndicator};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct PatternProcessor {
    pattern_matcher: Arc<PatternMatcher>,
    unique_patterns: Vec<String>,
    extension_filter: HashSet<String>,
    exact_patterns: HashSet<String>,
    high_priority_files: Arc<HashSet<String>>,

    compiled_patterns: Arc<HashMap<String, bool>>,

    extension_cache: Arc<HashMap<String, bool>>,
    exact_cache: Arc<HashMap<String, bool>>,
}

impl PatternProcessor {
    pub fn new(
        pattern_matcher: Arc<PatternMatcher>,
        patterns: Vec<String>,
        languages: Vec<Arc<ProjectIndicator>>,
    ) -> Self {
        let mut processor = Self {
            pattern_matcher,
            unique_patterns: patterns,
            extension_filter: HashSet::new(),
            exact_patterns: HashSet::new(),
            high_priority_files: Arc::new(HashSet::new()),
            compiled_patterns: Arc::new(HashMap::new()),
            extension_cache: Arc::new(HashMap::new()),
            exact_cache: Arc::new(HashMap::new()),
        };

        processor.precompute_patterns();
        processor.precompute_high_priority_files(languages);
        processor.precompute_caches();
        processor
    }

    pub fn should_scan_file(&self, filename: &str) -> bool {
        if let Some(&should_scan) = self.exact_cache.get(filename) {
            return should_scan;
        }

        if self.exact_patterns.contains(filename) {
            return true;
        }

        if let Some(ext) = filename.split('.').next_back() {
            if let Some(&should_scan) = self.extension_cache.get(ext) {
                if should_scan {
                    return true;
                }
            } else if self.extension_filter.contains(ext) {
                return true;
            }
        }

        let should_scan = self
            .unique_patterns
            .iter()
            .any(|pattern| self.pattern_matcher.matches_pattern(filename, pattern));

        should_scan
    }

    pub fn is_strong_evidence(&self, filename: &str) -> bool {
        self.high_priority_files.contains(filename)
    }

    pub fn match_file_against_patterns(
        &self,
        file_path: &Path,
        base_path: &Path,
        patterns: &[String],
    ) -> Option<MatchedFile> {
        if let Ok(relative_path) = file_path.strip_prefix(base_path) {
            if let Some(relative_str) = relative_path.to_str() {
                if let Some(filename) = relative_path.file_name().and_then(|n| n.to_str()) {
                    for pattern in patterns {
                        if self.pattern_matcher.matches_pattern(filename, pattern) {
                            return Some(MatchedFile::new(
                                filename.to_string(),
                                relative_str.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_pattern_importance(
        &self,
        pattern: &str,
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        for language in languages {
            for root_indicator in &language.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return root_indicator.weight;
                }
            }
            for framework in &language.frameworks {
                for root_indicator in &framework.root_indicators {
                    if self
                        .pattern_matcher
                        .matches_pattern(pattern, &root_indicator.pattern)
                    {
                        return root_indicator.weight;
                    }
                }
            }
        }

        for language in languages {
            for file_pattern in &language.files {
                if self.pattern_matcher.matches_pattern(pattern, file_pattern) {
                    let base_importance = 1.0 - (language.priority as f32 - 1.0) * 0.05;
                    return base_importance.max(0.5);
                }
            }
        }

        0.7
    }

    fn precompute_patterns(&mut self) {
        let mut extensions = HashSet::new();
        let mut exact = HashSet::new();

        for pattern in &self.unique_patterns {
            if pattern.contains('*') || pattern.contains('?') {
                if let Some(ext) = pattern.strip_prefix("*.") {
                    extensions.insert(ext.to_string());
                }
            } else {
                exact.insert(pattern.clone());
            }
        }

        self.extension_filter = extensions;
        self.exact_patterns = exact;
    }

    fn precompute_high_priority_files(&mut self, languages: Vec<Arc<ProjectIndicator>>) {
        let mut high_priority_files = HashSet::new();

        for language in &languages {
            if language.priority <= 2 {
                for file_pattern in &language.files {
                    if !file_pattern.contains('*') && !file_pattern.contains('?') {
                        high_priority_files.insert(file_pattern.clone());
                    }
                }
            }
        }

        self.high_priority_files = Arc::new(high_priority_files);
    }

    fn precompute_caches(&mut self) {
        let mut compiled_patterns = HashMap::new();
        let mut extension_cache = HashMap::new();
        let mut exact_cache = HashMap::new();

        for pattern in &self.unique_patterns {
            let has_wildcards = pattern.contains('*') || pattern.contains('?');
            compiled_patterns.insert(pattern.clone(), has_wildcards);
        }

        for ext in &self.extension_filter {
            extension_cache.insert(ext.clone(), true);
        }

        for filename in &self.exact_patterns {
            exact_cache.insert(filename.clone(), true);
        }

        self.compiled_patterns = Arc::new(compiled_patterns);
        self.extension_cache = Arc::new(extension_cache);
        self.exact_cache = Arc::new(exact_cache);
    }

    pub fn get_patterns(&self) -> &[String] {
        &self.unique_patterns
    }

    pub fn get_high_priority_files(&self) -> &HashSet<String> {
        &self.high_priority_files
    }
}
