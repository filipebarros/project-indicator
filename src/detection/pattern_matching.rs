use crate::patterns::simple_wildcard_match;
use dashmap::DashMap;
use std::sync::Arc;

const MAX_PATTERN_CACHE_ENTRIES: usize = 10000;

pub struct PatternMatcher {
    cache: Arc<DashMap<(String, String), bool>>,
    max_entries: usize,
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            max_entries: MAX_PATTERN_CACHE_ENTRIES,
        }
    }

    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    pub fn cache_stats(&self) -> usize {
        self.cache.len()
    }

    pub fn matches_pattern(&self, file_name: &str, pattern: &str) -> bool {
        let cache_key = (file_name.to_string(), pattern.to_string());

        if let Some(cached_result) = self.cache.get(&cache_key) {
            return *cached_result;
        }

        let result = if pattern.contains('*') {
            self.optimized_wildcard_match(file_name, pattern)
        } else {
            file_name == pattern
        };

        if self.cache.len() >= self.max_entries {
            self.evict_entries();
        }

        self.cache.insert(cache_key, result);

        result
    }

    fn evict_entries(&self) {
        let target_size = (self.max_entries as f64 * 0.75) as usize;
        let to_remove = self.cache.len().saturating_sub(target_size);

        for (removed, entry) in self.cache.iter().enumerate() {
            if removed >= to_remove {
                break;
            }
            let key = entry.key().clone();
            drop(entry);
            self.cache.remove(&key);
        }
    }

    fn optimized_wildcard_match(&self, file_name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(ext) = pattern.strip_prefix("*.") {
            return self.simd_extension_match(file_name, ext);
        }

        if let Some(prefix) = pattern.strip_suffix("*") {
            if !prefix.contains('*') {
                return self.simd_prefix_match(file_name, prefix);
            }
        }

        simple_wildcard_match(file_name, pattern)
    }

    fn simd_extension_match(&self, file_name: &str, extension: &str) -> bool {
        if file_name.len() <= extension.len() {
            return false;
        }

        let start_pos = file_name.len() - extension.len() - 1;
        if start_pos >= file_name.len() || file_name.as_bytes()[start_pos] != b'.' {
            return false;
        }

        file_name[start_pos + 1..] == *extension
    }

    fn simd_prefix_match(&self, file_name: &str, prefix: &str) -> bool {
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
    fn test_simd_pattern_matching() -> Result<(), Box<dyn std::error::Error>> {
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
    fn test_performance_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = create_matcher();

        let short_file = "a.rs";
        let long_file = "very_long_filename_that_should_trigger_simd_optimization.rs";
        let short_pattern = "*.rs";
        let complex_pattern = "*very*long*optimization*";

        assert!(matcher.matches_pattern(short_file, short_pattern));
        assert!(matcher.matches_pattern(long_file, short_pattern));
        assert!(matcher.matches_pattern(long_file, complex_pattern));
        assert!(!matcher.matches_pattern(short_file, complex_pattern));
        Ok(())
    }
}
