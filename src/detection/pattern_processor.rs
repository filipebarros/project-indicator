use crate::patterns::matches_pattern;
use crate::types::MatchedFile;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct PatternProcessor {
    unique_patterns: Arc<Vec<String>>,
    extension_filter: Arc<HashSet<String>>,
    exact_patterns: Arc<HashSet<String>>,
}

impl PatternProcessor {
    pub fn new(patterns: Arc<Vec<String>>) -> Self {
        let mut extension_filter = HashSet::new();
        let mut exact_patterns = HashSet::new();

        for pattern in patterns.iter() {
            if pattern.contains('*') || pattern.contains('?') {
                if let Some(ext) = pattern.strip_prefix("*.") {
                    extension_filter.insert(ext.to_string());
                }
            } else {
                exact_patterns.insert(pattern.clone());
            }
        }

        Self {
            unique_patterns: patterns,
            extension_filter: Arc::new(extension_filter),
            exact_patterns: Arc::new(exact_patterns),
        }
    }

    pub fn should_scan_file(&self, filename: &str) -> bool {
        if self.exact_patterns.contains(filename) {
            return true;
        }

        if let Some(ext) = filename.split('.').next_back() {
            if self.extension_filter.contains(ext) {
                return true;
            }
        }

        let should_scan = self
            .unique_patterns
            .iter()
            .any(|pattern| matches_pattern(filename, pattern));

        should_scan
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
                        if matches_pattern(filename, pattern) {
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

    /// Returns an Arc clone of the patterns vector for efficient sharing across threads.
    ///
    /// Use this method when you need to share the patterns across multiple threads
    /// (e.g., in parallel scanning) to avoid cloning the entire vector. The Arc clone
    /// is just a pointer copy with reference counting, making it very cheap.
    pub fn get_patterns_arc(&self) -> Arc<Vec<String>> {
        Arc::clone(&self.unique_patterns)
    }

    pub fn get_exact_patterns(&self) -> &HashSet<String> {
        &self.exact_patterns
    }
}
