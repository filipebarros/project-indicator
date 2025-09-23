//! Shared pattern matching utilities (glob-like)

/// Convert a glob pattern to a regex pattern string
/// Returns None for exact matches (no wildcards), where simple equality is faster
/// Optimized to avoid creating expensive regex for simple patterns
pub fn pattern_to_regex(pattern: &str) -> Option<String> {
    if !pattern.contains('*') {
        return None; // Use simple equality for exact matches
    }

    // Count wildcards to determine complexity
    let wildcard_count = pattern.matches('*').count();

    // For very simple patterns (prefix* or *suffix), we use simple_wildcard_match instead
    if wildcard_count == 1 && (pattern.starts_with('*') || pattern.ends_with('*')) {
        return None; // Let simple_wildcard_match handle this
    }

    // For complex patterns with multiple wildcards, create regex
    if wildcard_count > 1 {
        let escaped = regex::escape(pattern);
        let regex_pattern = escaped.replace(r"\*", ".*");
        Some(format!("^{}$", regex_pattern))
    } else {
        None // Single wildcard patterns handled by simple_wildcard_match
    }
}

/// Simple wildcard matching for patterns with '*'
/// Handles common cases efficiently without full regex engine
/// Optimized for performance with early returns
pub fn simple_wildcard_match(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text == pattern;
    }

    // Fast path for very common patterns
    if pattern == "*" {
        return true; // Matches everything
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    // Optimized handling for single wildcard patterns
    if parts.len() == 2 {
        let prefix = parts[0];
        let suffix = parts[1];

        // Fast paths for common cases
        if prefix.is_empty() {
            return text.ends_with(suffix); // Pattern: *suffix
        }
        if suffix.is_empty() {
            return text.starts_with(prefix); // Pattern: prefix*
        }

        // Pattern: prefix*suffix
        return text.len() >= prefix.len() + suffix.len()
            && text.starts_with(prefix)
            && text.ends_with(suffix);
    }

    // For more complex patterns, ensure all non-empty segments appear in order
    let non_empty_parts: Vec<&str> = parts.into_iter().filter(|p| !p.is_empty()).collect();

    if non_empty_parts.is_empty() {
        return true; // Pattern is all wildcards
    }

    let mut start_idx = 0usize;
    for part in non_empty_parts {
        if let Some(pos) = text[start_idx..].find(part) {
            start_idx += pos + part.len();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_to_regex_exact_matches() {
        // Exact matches should return None (use simple equality)
        assert_eq!(pattern_to_regex("package.json"), None);
        assert_eq!(pattern_to_regex("Cargo.toml"), None);
        assert_eq!(pattern_to_regex("README.md"), None);
        assert_eq!(pattern_to_regex(""), None);
    }

    #[test]
    fn test_pattern_to_regex_simple_wildcards() {
        // Simple patterns should return None (use simple_wildcard_match)
        assert_eq!(pattern_to_regex("*.rs"), None); // suffix wildcard
        assert_eq!(pattern_to_regex("test*"), None); // prefix wildcard
        assert_eq!(pattern_to_regex("*"), None); // match all
    }

    #[test]
    fn test_pattern_to_regex_complex_patterns() {
        // Complex patterns should generate regex
        let result = pattern_to_regex("*.test.*");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "^.*\\.test\\..*$");

        let result = pattern_to_regex("test*.*.log");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "^test.*\\..*\\.log$");

        let result = pattern_to_regex("*foo*bar*");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "^.*foo.*bar.*$");
    }

    #[test]
    fn test_pattern_to_regex_middle_wildcards() {
        // Single wildcard in middle should return None
        assert_eq!(pattern_to_regex("test*.log"), None);
        assert_eq!(pattern_to_regex("src*main"), None);
    }

    #[test]
    fn test_simple_wildcard_match_exact() {
        // Exact matches
        assert!(simple_wildcard_match("package.json", "package.json"));
        assert!(simple_wildcard_match("", ""));
        assert!(!simple_wildcard_match("package.json", "Cargo.toml"));
        assert!(!simple_wildcard_match("test", ""));
        assert!(!simple_wildcard_match("", "test"));
    }

    #[test]
    fn test_simple_wildcard_match_all() {
        // Match everything pattern
        assert!(simple_wildcard_match("anything", "*"));
        assert!(simple_wildcard_match("", "*"));
        assert!(simple_wildcard_match("very.long.filename.with.dots", "*"));
    }

    #[test]
    fn test_simple_wildcard_match_prefix() {
        // Prefix patterns (prefix*)
        assert!(simple_wildcard_match("test.rs", "test*"));
        assert!(simple_wildcard_match("test", "test*"));
        assert!(simple_wildcard_match("testing123", "test*"));
        assert!(!simple_wildcard_match("mytest", "test*"));
        assert!(!simple_wildcard_match("", "test*"));
    }

    #[test]
    fn test_simple_wildcard_match_suffix() {
        // Suffix patterns (*suffix)
        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match(".rs", "*.rs"));
        assert!(simple_wildcard_match("very.long.file.rs", "*.rs"));
        assert!(!simple_wildcard_match("rs", "*.rs"));
        assert!(!simple_wildcard_match("main.toml", "*.rs"));
        assert!(!simple_wildcard_match("", "*.rs"));
    }

    #[test]
    fn test_simple_wildcard_match_prefix_suffix() {
        // Prefix and suffix patterns (prefix*suffix)
        assert!(simple_wildcard_match("test.rs", "test*.rs"));
        assert!(simple_wildcard_match("test_main.rs", "test*.rs"));
        assert!(simple_wildcard_match("test.rs", "test*.rs"));
        assert!(!simple_wildcard_match("main.rs", "test*.rs"));
        assert!(!simple_wildcard_match("test.toml", "test*.rs"));
        assert!(!simple_wildcard_match("test", "test*.rs")); // missing suffix
        assert!(!simple_wildcard_match(".rs", "test*.rs")); // missing prefix

        // Edge case: prefix + suffix longer than text
        assert!(!simple_wildcard_match("t.r", "test*.rs"));
    }

    #[test]
    fn test_simple_wildcard_match_multiple_wildcards() {
        // Multiple wildcards
        assert!(simple_wildcard_match("src/main.rs", "src*main*"));
        assert!(simple_wildcard_match("src/test/main.c", "src*main*"));
        assert!(simple_wildcard_match("src_main_file", "src*main*"));
        assert!(!simple_wildcard_match("main/src", "src*main*")); // wrong order
        assert!(!simple_wildcard_match("source/main", "src*main*")); // missing 'src'

        // Complex pattern
        assert!(simple_wildcard_match("test.spec.js", "test*spec*"));
        assert!(simple_wildcard_match("test_unit_spec_file", "test*spec*"));
        assert!(!simple_wildcard_match("spec.test.js", "test*spec*")); // wrong order
    }

    #[test]
    fn test_simple_wildcard_match_all_wildcards() {
        // Pattern with only wildcards
        assert!(simple_wildcard_match("anything", "**"));
        assert!(simple_wildcard_match("", "**"));
        assert!(simple_wildcard_match("test", "***"));
    }

    #[test]
    fn test_simple_wildcard_match_empty_segments() {
        // Patterns with empty segments between wildcards
        assert!(simple_wildcard_match("test123", "*test*"));
        assert!(simple_wildcard_match("prefixtest", "*test*"));
        assert!(simple_wildcard_match("testsuffix", "*test*"));
        assert!(simple_wildcard_match("prefixtestsuffix", "*test*"));
    }

    #[test]
    fn test_simple_wildcard_match_edge_cases() {
        // Edge cases
        assert!(simple_wildcard_match("a", "*a*"));
        assert!(simple_wildcard_match("a", "a*"));
        assert!(simple_wildcard_match("a", "*a"));
        assert!(!simple_wildcard_match("", "a*"));
        assert!(!simple_wildcard_match("", "*a"));

        // Case sensitivity
        assert!(!simple_wildcard_match("Test.RS", "*.rs"));
        assert!(simple_wildcard_match("Test.RS", "*.RS"));
    }

    #[test]
    fn test_pattern_optimization_integration() {
        // Test that the optimization logic works correctly together

        // These should use simple_wildcard_match (pattern_to_regex returns None)
        let simple_patterns = ["*.rs", "test*", "*", "test*.log"];
        for pattern in &simple_patterns {
            assert_eq!(
                pattern_to_regex(pattern),
                None,
                "Pattern {} should use simple matching",
                pattern
            );
        }

        // These should use regex (pattern_to_regex returns Some)
        let complex_patterns = ["*.test.*", "*foo*bar*baz*"];
        for pattern in &complex_patterns {
            assert!(
                pattern_to_regex(pattern).is_some(),
                "Pattern {} should use regex",
                pattern
            );
        }

        // Verify the simple patterns work correctly
        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match("test_file", "test*"));
        assert!(simple_wildcard_match("anything", "*"));
        assert!(simple_wildcard_match("test_main.log", "test*.log"));
    }

    #[test]
    fn test_real_world_patterns() {
        // Test patterns commonly used in project detection

        // TypeScript/JavaScript patterns
        assert!(simple_wildcard_match("main.ts", "*.ts"));
        assert!(simple_wildcard_match("component.tsx", "*.tsx"));
        assert!(simple_wildcard_match("app.js", "*.js"));
        assert!(simple_wildcard_match("module.mjs", "*.mjs"));

        // Rust patterns
        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match("lib.rs", "*.rs"));

        // Python patterns
        assert!(simple_wildcard_match("main.py", "*.py"));
        assert!(simple_wildcard_match("__init__.py", "*.py"));

        // Go patterns
        assert!(simple_wildcard_match("main.go", "*.go"));

        // C/C++ patterns
        assert!(simple_wildcard_match("main.c", "*.c"));
        assert!(simple_wildcard_match("header.h", "*.h"));
        assert!(simple_wildcard_match("source.cpp", "*.cpp"));

        // Test files
        assert!(simple_wildcard_match("test_main.py", "test_*"));
        assert!(simple_wildcard_match("main_test.go", "*_test.go"));
        // This pattern has multiple segments: "" + "spec" + "js"
        // It should work with our complex pattern logic
        assert!(simple_wildcard_match("test.spec.js", "*.spec.js"));
        assert!(simple_wildcard_match("component.spec.js", "*.spec.js"));
        assert!(!simple_wildcard_match("spec.js", "*.spec.js")); // Missing the middle part
    }
}
