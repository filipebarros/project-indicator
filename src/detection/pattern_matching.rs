//! Pattern matching with memoization for efficient file pattern evaluation.
//!
//! # Ownership Model
//!
//! `PatternMatcher` is designed to be shared across multiple components via `Arc<PatternMatcher>`.
//!
//! - **Owner**: `DetectionEngine` creates a single `PatternMatcher` instance
//! - **Shared with**:
//!   - `ConfidenceScorer` - for language confidence calculations
//!   - `ScanningEngine` -> `PatternProcessor` - for file discovery and filtering
//!
//! The memo lives for a single detection run (one CLI invocation). The number
//! of distinct pattern × filename pairs touched in one run is naturally
//! bounded by the scan, so there is no eviction.
//!
//! ```rust
//! use project_indicator::detection::pattern_matching::PatternMatcher;
//! use std::sync::Arc;
//!
//! let shared_pattern_matcher = Arc::new(PatternMatcher::new());
//!
//! let matcher_clone = shared_pattern_matcher.clone();
//! assert!(matcher_clone.matches_pattern("src/main.rs", "*.rs"));
//! ```

use crate::patterns::simple_wildcard_match;
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Thread-safe pattern matcher with per-run memoization.
///
/// Designed to be wrapped in `Arc` and shared across detection components.
///
/// Cache structure: `DashMap<pattern, DashMap<filename, bool>>`. The nested
/// map allows `&str` lookups on both levels, so cache hits allocate nothing.
pub struct PatternMatcher {
    cache: DashMap<String, DashMap<String, bool>>,
    cache_hits: AtomicUsize,
    cache_misses: AtomicUsize,
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
        }
    }

    /// Returns (entry_count, hit_rate)
    pub fn cache_stats(&self) -> (usize, f64) {
        let entries = self.cache.iter().map(|entry| entry.value().len()).sum();
        let (hits, misses) = self.hit_miss_counts();
        let total_accesses = hits + misses;
        let hit_rate = if total_accesses > 0 {
            (hits as f64 / total_accesses as f64) * 100.0
        } else {
            0.0
        };
        (entries, hit_rate)
    }

    /// Returns raw (hits, misses) counter values
    pub fn hit_miss_counts(&self) -> (usize, usize) {
        (
            self.cache_hits.load(Ordering::Relaxed),
            self.cache_misses.load(Ordering::Relaxed),
        )
    }

    pub fn matches_pattern(&self, file_name: &str, pattern: &str) -> bool {
        // Check cache: pattern -> filename -> result
        // Using &str for lookup avoids allocating on cache hits
        if let Some(entry) = self.cache.get(pattern) {
            if let Some(result) = entry.get(file_name) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return *result;
            }
            // Pattern exists but filename doesn't - compute result and insert
            drop(entry); // Release lock before computing
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        let result = if pattern.contains('*') {
            self.optimized_wildcard_match(file_name, pattern)
        } else {
            file_name == pattern
        };

        self.cache
            .entry(pattern.to_string())
            .or_default()
            .insert(file_name.to_string(), result);

        result
    }

    fn optimized_wildcard_match(&self, file_name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(ext) = pattern.strip_prefix("*.") {
            return self.optimized_extension_match(file_name, ext);
        }

        if let Some(prefix) = pattern.strip_suffix("*") {
            if !prefix.contains('*') {
                return self.optimized_prefix_match(file_name, prefix);
            }
        }

        simple_wildcard_match(file_name, pattern)
    }

    /// Optimized extension matching using slice comparison.
    ///
    /// Checks if a filename ends with the given extension.
    /// Uses efficient slice comparison instead of regex or full string matching.
    fn optimized_extension_match(&self, file_name: &str, extension: &str) -> bool {
        if file_name.len() <= extension.len() {
            return false;
        }

        let start_pos = file_name.len() - extension.len() - 1;
        if start_pos >= file_name.len() || file_name.as_bytes()[start_pos] != b'.' {
            return false;
        }

        file_name[start_pos + 1..] == *extension
    }

    /// Optimized prefix matching using slice comparison.
    ///
    /// Checks if a filename starts with the given prefix.
    /// Uses efficient slice comparison for prefix patterns like "test*".
    fn optimized_prefix_match(&self, file_name: &str, prefix: &str) -> bool {
        if file_name.len() < prefix.len() {
            return false;
        }

        file_name[..prefix.len()] == *prefix
    }
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_matcher() -> PatternMatcher {
        PatternMatcher::new()
    }

    #[test]
    fn test_exact_matching() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        assert!(matcher.matches_pattern("package.json", "package.json"));
        assert!(!matcher.matches_pattern("package.json", "Cargo.toml"));
        Ok(())
    }

    #[test]
    fn test_extension_matching() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        assert!(matcher.matches_pattern("main.rs", "*.rs"));
        assert!(matcher.matches_pattern("lib.rs", "*.rs"));
        assert!(!matcher.matches_pattern("main.js", "*.rs"));
        assert!(!matcher.matches_pattern("main", "*.rs"));
        Ok(())
    }

    #[test]
    fn test_prefix_matching() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        assert!(matcher.matches_pattern("test_file.rs", "test*"));
        assert!(matcher.matches_pattern("testing", "test*"));
        assert!(!matcher.matches_pattern("my_test", "test*"));
        Ok(())
    }

    #[test]
    fn test_wildcard_all() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        assert!(matcher.matches_pattern("anything", "*"));
        assert!(matcher.matches_pattern("", "*"));
        assert!(matcher.matches_pattern("very/long/path/file.ext", "*"));
        Ok(())
    }

    #[test]
    fn test_complex_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        assert!(matcher.matches_pattern("test_config_file.json", "*config*"));
        assert!(matcher.matches_pattern("my_config.json", "*config*"));
        assert!(!matcher.matches_pattern("settings.json", "*config*"));
        Ok(())
    }

    #[test]
    fn test_optimized_pattern_matching() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();
        let filename = "very_long_filename_with_multiple_parts.extension";
        let pattern = "*long*multiple*";
        assert!(matcher.matches_pattern(filename, pattern));

        let pattern2 = "*short*missing*";
        assert!(!matcher.matches_pattern(filename, pattern2));
        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();

        assert!(!matcher.matches_pattern("", "test"));
        assert!(matcher.matches_pattern("", "*"));

        assert!(matcher.matches_pattern("a", "*"));
        assert!(matcher.matches_pattern("a", "a"));
        assert!(!matcher.matches_pattern("a", "b"));

        assert!(matcher.matches_pattern("test.rs", "*.rs"));
        assert!(matcher.matches_pattern("test", "test*"));
        assert!(matcher.matches_pattern("test", "*test"));
        Ok(())
    }

    #[test]
    fn test_cache_hit_and_miss_counting() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();

        let (entries, hit_rate) = matcher.cache_stats();
        assert_eq!(entries, 0);
        assert_eq!(hit_rate, 0.0);

        matcher.matches_pattern("test.rs", "*.rs");
        matcher.matches_pattern("main.rs", "*.rs");
        matcher.matches_pattern("package.json", "*.json");

        let (entries, hit_rate) = matcher.cache_stats();
        assert_eq!(entries, 3);
        assert_eq!(hit_rate, 0.0);

        // Repeat lookup hits the memo
        matcher.matches_pattern("test.rs", "*.rs");

        let (entries, hit_rate) = matcher.cache_stats();
        let (hits, misses) = matcher.hit_miss_counts();
        assert_eq!(entries, 3);
        assert_eq!(hits, 1);
        assert_eq!(misses, 3);
        assert!(hit_rate > 0.0);

        Ok(())
    }
}
