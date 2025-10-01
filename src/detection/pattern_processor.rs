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
    extension_cache: Arc<HashMap<String, bool>>,
    exact_cache: Arc<HashMap<String, bool>>,
}

impl PatternProcessor {
    pub fn new(
        pattern_matcher: Arc<PatternMatcher>,
        patterns: Vec<String>,
        languages: Vec<Arc<ProjectIndicator>>,
    ) -> Self {
        let mut extension_filter = HashSet::new();
        let mut exact_patterns = HashSet::new();
        let mut high_priority_files = HashSet::new();
        let mut extension_cache = HashMap::new();
        let mut exact_cache = HashMap::new();

        for pattern in &patterns {
            if pattern.contains('*') || pattern.contains('?') {
                if let Some(ext) = pattern.strip_prefix("*.") {
                    extension_filter.insert(ext.to_string());
                }
            } else {
                exact_patterns.insert(pattern.clone());
            }
        }

        for language in &languages {
            if language.priority <= 2 {
                for file_pattern in &language.files {
                    if !file_pattern.contains('*') && !file_pattern.contains('?') {
                        high_priority_files.insert(file_pattern.clone());
                    }
                }
            }
        }

        for ext in &extension_filter {
            extension_cache.insert(ext.clone(), true);
        }

        for filename in &exact_patterns {
            exact_cache.insert(filename.clone(), true);
        }

        Self {
            pattern_matcher,
            unique_patterns: patterns,
            extension_filter,
            exact_patterns,
            high_priority_files: Arc::new(high_priority_files),
            extension_cache: Arc::new(extension_cache),
            exact_cache: Arc::new(exact_cache),
        }
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

    pub fn get_patterns(&self) -> &[String] {
        &self.unique_patterns
    }

    pub fn get_high_priority_files(&self) -> &HashSet<String> {
        &self.high_priority_files
    }
}
