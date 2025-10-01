use crate::types::ProjectIndicator;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Handles pattern extraction from languages and regex compilation.
///
/// This module is responsible for:
/// - Extracting unique file patterns from language definitions
/// - Compiling wildcard patterns into regex for efficient matching
/// - Maintaining a cache of compiled regex patterns
pub struct PatternCompiler {
    unique_patterns: Arc<Vec<String>>,
    pattern_cache: HashMap<String, Regex>,
}

impl PatternCompiler {
    /// Creates a new PatternCompiler from a list of languages.
    ///
    /// Extracts all unique file patterns and precompiles regex patterns
    /// for wildcard patterns (containing '*').
    pub fn new(languages: &[Arc<ProjectIndicator>]) -> Self {
        let mut all_patterns = HashSet::new();

        // Extract all unique patterns from all languages
        for language in languages {
            for pattern in &language.files {
                all_patterns.insert(pattern.clone());
            }
        }

        let unique_patterns: Vec<String> = all_patterns.into_iter().collect();
        let mut pattern_cache = HashMap::new();

        // Precompile regex patterns for wildcard patterns
        for pattern in &unique_patterns {
            if pattern.contains('*') || pattern.contains('?') {
                if let Some(regex_pattern) = crate::patterns::pattern_to_regex(pattern) {
                    if let Ok(regex) = Regex::new(&regex_pattern) {
                        pattern_cache.insert(pattern.clone(), regex);
                    } else {
                        log::warn!("Failed to compile regex for pattern: {}", pattern);
                    }
                } else {
                    log::warn!("Failed to convert pattern to regex: {}", pattern);
                }
            }
        }

        Self {
            unique_patterns: Arc::new(unique_patterns),
            pattern_cache,
        }
    }

    /// Returns the unique patterns as an Arc for efficient sharing.
    pub fn unique_patterns(&self) -> Arc<Vec<String>> {
        self.unique_patterns.clone()
    }

    /// Gets the number of unique patterns.
    pub fn pattern_count(&self) -> usize {
        self.unique_patterns.len()
    }

    /// Gets the number of compiled regex patterns.
    pub fn compiled_regex_count(&self) -> usize {
        self.pattern_cache.len()
    }

    /// Checks if a pattern has been compiled to regex.
    pub fn has_compiled_pattern(&self, pattern: &str) -> bool {
        self.pattern_cache.contains_key(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProjectIndicator;

    fn create_test_language(name: &str, patterns: Vec<&str>) -> Arc<ProjectIndicator> {
        Arc::new(ProjectIndicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        ))
    }

    #[test]
    fn test_pattern_compiler_creation() {
        let languages = vec![
            create_test_language("Rust", vec!["*.rs", "Cargo.toml", "src/**/*.rs"]),
            create_test_language("JavaScript", vec!["*.js", "package.json", "**/*.test.js"]),
        ];

        let compiler = PatternCompiler::new(&languages);

        assert_eq!(compiler.pattern_count(), 6);
        // Simple wildcard patterns (*.rs) are NOT compiled to regex (handled by simple_wildcard_match)
        assert!(!compiler.has_compiled_pattern("*.rs"));
        // Complex patterns with multiple wildcards ARE compiled to regex
        assert!(compiler.has_compiled_pattern("src/**/*.rs"));
        assert!(compiler.has_compiled_pattern("**/*.test.js"));
        // Exact patterns should not be compiled
        assert!(!compiler.has_compiled_pattern("Cargo.toml"));
    }

    #[test]
    fn test_pattern_deduplication() {
        let languages = vec![
            create_test_language("TypeScript", vec!["*.ts", "package.json"]),
            create_test_language("JavaScript", vec!["*.js", "package.json"]),
        ];

        let compiler = PatternCompiler::new(&languages);

        // package.json should only appear once
        assert_eq!(compiler.pattern_count(), 3); // *.ts, *.js, package.json
    }

    #[test]
    fn test_wildcard_pattern_compilation() {
        let languages = vec![create_test_language(
            "Rust",
            vec!["*.rs", "Cargo.toml", "src/**/*.rs"],
        )];

        let compiler = PatternCompiler::new(&languages);

        // Simple patterns (*.rs) are NOT compiled - handled by simple_wildcard_match
        assert!(!compiler.has_compiled_pattern("*.rs"));
        assert!(!compiler.has_compiled_pattern("Cargo.toml"));
        // Complex patterns with multiple wildcards ARE compiled
        assert!(compiler.has_compiled_pattern("src/**/*.rs"));
    }

    #[test]
    fn test_empty_languages() {
        let languages: Vec<Arc<ProjectIndicator>> = vec![];
        let compiler = PatternCompiler::new(&languages);

        assert_eq!(compiler.pattern_count(), 0);
        assert_eq!(compiler.compiled_regex_count(), 0);
    }
}
