use crate::types::ProjectIndicator;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Marker regex for simple wildcard patterns that use optimized matching
fn create_simple_pattern_marker() -> Regex {
    // This regex will never match anything - it's just a marker
    // The actual matching will be done by the optimized algorithms
    // The pattern "^$" is guaranteed to compile successfully
    match Regex::new("^$") {
        Ok(regex) => regex,
        Err(_) => {
            // This should never happen as "^$" is a valid regex pattern
            // If it does, we have a serious issue with the regex crate
            panic!("Critical error: Marker regex pattern failed to compile - this indicates a bug in the regex crate");
        }
    }
}

/// Marker regex for exact match patterns
fn create_exact_match_marker() -> Regex {
    // This regex will never match anything - it's just a marker
    // The actual matching will be done by direct string comparison
    // The pattern "^$" is guaranteed to compile successfully
    match Regex::new("^$") {
        Ok(regex) => regex,
        Err(_) => {
            // This should never happen as "^$" is a valid regex pattern
            // If it does, we have a serious issue with the regex crate
            panic!("Critical error: Marker regex pattern failed to compile - this indicates a bug in the regex crate");
        }
    }
}

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
    pub fn new(languages: &[Arc<ProjectIndicator>]) -> Self {
        let total_patterns_estimate: usize = languages.iter().map(|lang| lang.files.len()).sum();
        let mut all_patterns = HashSet::with_capacity(total_patterns_estimate);

        for language in languages {
            for pattern in &language.files {
                all_patterns.insert(pattern.clone());
            }
        }

        let unique_patterns: Vec<String> = all_patterns.into_iter().collect();
        let mut pattern_cache = HashMap::with_capacity(unique_patterns.len());

        for pattern in &unique_patterns {
            // Handle all patterns in one place - choose the best matching strategy
            if pattern.contains('*') || pattern.contains('?') {
                // Try regex compilation first for complex patterns
                if let Some(regex_pattern) = crate::patterns::pattern_to_regex(pattern) {
                    if let Ok(regex) = Regex::new(&regex_pattern) {
                        pattern_cache.insert(pattern.clone(), regex);
                    } else {
                        log::warn!("Failed to compile regex for pattern: {}", pattern);
                    }
                } else {
                    // Simple wildcard patterns - we'll handle them with optimized matching
                    // but we still want to track them in our pattern cache for consistency
                    // We'll use a special marker to indicate this is a simple pattern
                    pattern_cache.insert(pattern.clone(), create_simple_pattern_marker());
                }
            } else {
                // Exact match patterns - also track them for consistency
                pattern_cache.insert(pattern.clone(), create_exact_match_marker());
            }
        }

        Self {
            unique_patterns: Arc::new(unique_patterns),
            pattern_cache,
        }
    }

    pub fn unique_patterns(&self) -> Arc<Vec<String>> {
        self.unique_patterns.clone()
    }

    pub fn pattern_count(&self) -> usize {
        self.unique_patterns.len()
    }

    pub fn compiled_regex_count(&self) -> usize {
        self.pattern_cache.len()
    }

    pub fn has_compiled_pattern(&self, pattern: &str) -> bool {
        self.pattern_cache.contains_key(pattern)
    }

    /// Check if a pattern is a complex regex pattern
    pub fn is_regex_pattern(&self, pattern: &str) -> bool {
        if let Some(regex) = self.pattern_cache.get(pattern) {
            // If the regex is not a marker, it's a real regex pattern
            regex.as_str() != "^$"
        } else {
            false
        }
    }

    /// Check if a pattern is a simple wildcard pattern (uses optimized matching)
    pub fn is_simple_pattern(&self, pattern: &str) -> bool {
        if let Some(regex) = self.pattern_cache.get(pattern) {
            // If the regex is the simple pattern marker, it's a simple pattern
            regex.as_str() == "^$" && (pattern.contains('*') || pattern.contains('?'))
        } else {
            false
        }
    }

    /// Check if a pattern is an exact match pattern
    pub fn is_exact_pattern(&self, pattern: &str) -> bool {
        if let Some(regex) = self.pattern_cache.get(pattern) {
            // If the regex is the exact match marker, it's an exact pattern
            regex.as_str() == "^$" && !pattern.contains('*') && !pattern.contains('?')
        } else {
            false
        }
    }

    /// Get the compiled regex for a pattern (only works for regex patterns)
    pub fn get_compiled_regex(&self, pattern: &str) -> Option<&Regex> {
        if self.is_regex_pattern(pattern) {
            self.pattern_cache.get(pattern)
        } else {
            None
        }
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
        assert!(compiler.has_compiled_pattern("*.rs"));
        assert!(compiler.has_compiled_pattern("src/**/*.rs"));
        assert!(compiler.has_compiled_pattern("**/*.test.js"));
        assert!(compiler.has_compiled_pattern("Cargo.toml"));

        // Check pattern types
        assert!(compiler.is_simple_pattern("*.rs"));
        assert!(compiler.is_regex_pattern("src/**/*.rs"));
        assert!(compiler.is_regex_pattern("**/*.test.js"));
        assert!(compiler.is_exact_pattern("Cargo.toml"));
    }

    #[test]
    fn test_pattern_deduplication() {
        let languages = vec![
            create_test_language("TypeScript", vec!["*.ts", "package.json"]),
            create_test_language("JavaScript", vec!["*.js", "package.json"]),
        ];

        let compiler = PatternCompiler::new(&languages);

        assert_eq!(compiler.pattern_count(), 3);
    }

    #[test]
    fn test_wildcard_pattern_compilation() {
        let languages = vec![create_test_language(
            "Rust",
            vec!["*.rs", "Cargo.toml", "*.test.*"],
        )];

        let compiler = PatternCompiler::new(&languages);

        // All patterns should be tracked in the compiler
        assert!(compiler.has_compiled_pattern("*.rs"));
        assert!(compiler.has_compiled_pattern("Cargo.toml"));
        assert!(compiler.has_compiled_pattern("*.test.*"));

        // Check pattern types
        assert!(compiler.is_simple_pattern("*.rs"));
        assert!(compiler.is_exact_pattern("Cargo.toml"));
        assert!(compiler.is_regex_pattern("*.test.*"));

        // Verify we can get the regex for complex patterns
        assert!(compiler.get_compiled_regex("*.test.*").is_some());
        assert!(compiler.get_compiled_regex("*.rs").is_none());
        assert!(compiler.get_compiled_regex("Cargo.toml").is_none());
    }

    #[test]
    fn test_empty_languages() {
        let languages: Vec<Arc<ProjectIndicator>> = vec![];
        let compiler = PatternCompiler::new(&languages);

        assert_eq!(compiler.pattern_count(), 0);
        assert_eq!(compiler.compiled_regex_count(), 0);
    }
}
