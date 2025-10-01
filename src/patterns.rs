pub fn pattern_to_regex(pattern: &str) -> Option<String> {
    if pattern.is_empty() {
        return None;
    }

    if pattern.len() > 1024 {
        return None;
    }

    if pattern
        .chars()
        .any(|c| c.is_control() && c != '\t' && c != '\n')
    {
        return None;
    }

    if !pattern.contains('*') {
        return None;
    }

    let wildcard_count = pattern.matches('*').count();

    if wildcard_count == 1 && (pattern.starts_with('*') || pattern.ends_with('*')) {
        return None;
    }

    if wildcard_count > 1 {
        let escaped = regex::escape(pattern);
        let regex_pattern = escaped.replace(r"\*", ".*");
        let final_pattern = format!("^{}$", regex_pattern);

        if final_pattern.len() > 2048 {
            return None;
        }

        Some(final_pattern)
    } else {
        None
    }
}
pub fn simple_wildcard_match(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text == pattern;
    }

    if pattern == "*" {
        return true;
    }

    if let Some(star_pos) = pattern.find('*') {
        if !pattern[star_pos + 1..].contains('*') {
            let prefix = &pattern[..star_pos];
            let suffix = &pattern[star_pos + 1..];

            if prefix.is_empty() {
                return text.ends_with(suffix);
            }
            if suffix.is_empty() {
                return text.starts_with(prefix);
            }

            let prefix_len = prefix.len();
            let suffix_len = suffix.len();
            let text_len = text.len();

            if !text.starts_with(prefix) || !text.ends_with(suffix) {
                return false;
            }

            let prefix_end = prefix_len;
            let suffix_start = text_len - suffix_len;

            if prefix_end <= suffix_start {
                return true;
            }

            let overlap_len = prefix_end - suffix_start;
            if overlap_len <= suffix_len && overlap_len <= prefix_len {
                let prefix_overlap = &prefix[prefix_len - overlap_len..];
                let suffix_overlap = &suffix[..overlap_len];
                return prefix_overlap == suffix_overlap;
            }

            return false;
        }
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let first_part = parts[0];
    let mut search_start = if first_part.is_empty() {
        0
    } else {
        if !text.starts_with(first_part) {
            return false;
        }
        first_part.len()
    };

    for part in &parts[1..] {
        if part.is_empty() {
            continue;
        }

        if let Some(pos) = text[search_start..].find(part) {
            search_start += pos + part.len();
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
    fn test_pattern_to_regex_exact_matches() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(pattern_to_regex("package.json"), None);
        assert_eq!(pattern_to_regex("Cargo.toml"), None);
        assert_eq!(pattern_to_regex("README.md"), None);
        assert_eq!(pattern_to_regex(""), None);
        Ok(())
    }

    #[test]
    fn test_pattern_to_regex_input_validation() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(pattern_to_regex(""), None);

        let long_pattern = "a".repeat(1025);
        assert_eq!(pattern_to_regex(&long_pattern), None);

        let valid_long_pattern = format!(
            "{}*{}*{}",
            "a".repeat(300),
            "b".repeat(300),
            "c".repeat(300)
        );
        assert!(pattern_to_regex(&valid_long_pattern).is_some());

        assert_eq!(pattern_to_regex("test\x00*file"), None);
        assert_eq!(pattern_to_regex("test\x01*file"), None);
        assert_eq!(pattern_to_regex("test\x1F*file"), None);

        assert!(pattern_to_regex("test\t*file*bar").is_some());
        assert!(pattern_to_regex("test\n*file*bar").is_some());

        let pattern_causing_long_regex = format!("{}*{}", "a".repeat(500), "b".repeat(500));
        let result = pattern_to_regex(&pattern_causing_long_regex);
        if let Some(regex_str) = result {
            assert!(regex_str.len() <= 2048);
        }
        Ok(())
    }

    #[test]
    fn test_pattern_to_regex_simple_wildcards() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(pattern_to_regex("*.rs"), None);
        assert_eq!(pattern_to_regex("test*"), None);
        assert_eq!(pattern_to_regex("*"), None);
        Ok(())
    }

    #[test]
    fn test_pattern_to_regex_complex_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let result = pattern_to_regex("*.test.*");
        assert!(result.is_some());
        assert_eq!(
            result.ok_or("Failed to get regex for *.test.*")?,
            "^.*\\.test\\..*$"
        );

        let result = pattern_to_regex("test*.*.log");
        assert!(result.is_some());
        assert_eq!(
            result.ok_or("Failed to get regex for test*.*.log")?,
            "^test.*\\..*\\.log$"
        );

        let result = pattern_to_regex("*foo*bar*");
        assert!(result.is_some());
        assert_eq!(
            result.ok_or("Failed to get regex for *foo*bar*")?,
            "^.*foo.*bar.*$"
        );
        Ok(())
    }

    #[test]
    fn test_pattern_to_regex_middle_wildcards() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(pattern_to_regex("test*.log"), None);
        assert_eq!(pattern_to_regex("src*main"), None);
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_exact() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("package.json", "package.json"));
        assert!(simple_wildcard_match("", ""));
        assert!(!simple_wildcard_match("package.json", "Cargo.toml"));
        assert!(!simple_wildcard_match("test", ""));
        assert!(!simple_wildcard_match("", "test"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_all() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("anything", "*"));
        assert!(simple_wildcard_match("", "*"));
        assert!(simple_wildcard_match("very.long.filename.with.dots", "*"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_prefix() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("test.rs", "test*"));
        assert!(simple_wildcard_match("test", "test*"));
        assert!(simple_wildcard_match("testing123", "test*"));
        assert!(!simple_wildcard_match("mytest", "test*"));
        assert!(!simple_wildcard_match("", "test*"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_suffix() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match(".rs", "*.rs"));
        assert!(simple_wildcard_match("very.long.file.rs", "*.rs"));
        assert!(!simple_wildcard_match("rs", "*.rs"));
        assert!(!simple_wildcard_match("main.toml", "*.rs"));
        assert!(!simple_wildcard_match("", "*.rs"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_prefix_suffix() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("test.rs", "test*.rs"));
        assert!(simple_wildcard_match("test_main.rs", "test*.rs"));
        assert!(simple_wildcard_match("test.rs", "test*.rs"));
        assert!(!simple_wildcard_match("main.rs", "test*.rs"));
        assert!(!simple_wildcard_match("test.toml", "test*.rs"));
        assert!(!simple_wildcard_match("test", "test*.rs"));
        assert!(!simple_wildcard_match(".rs", "test*.rs"));

        assert!(!simple_wildcard_match("t.r", "test*.rs"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_overlapping_patterns() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("test", "test*t"));
        assert!(simple_wildcard_match("hello", "hello*o"));
        assert!(simple_wildcard_match("abc", "ab*bc"));
        assert!(simple_wildcard_match("abcd", "abc*cd"));

        assert!(simple_wildcard_match("test", "test*st"));
        assert!(!simple_wildcard_match("abc", "abc*def"));
        assert!(!simple_wildcard_match("hello", "hell*world"));

        assert!(simple_wildcard_match("test", "test*test"));
        assert!(simple_wildcard_match("aa", "aa*aa"));

        assert!(!simple_wildcard_match("ab", "abc*abc"));
        assert!(simple_wildcard_match("abcabc", "abc*abc"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_multiple_wildcards() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("src/main.rs", "src*main*"));
        assert!(simple_wildcard_match("src/test/main.c", "src*main*"));
        assert!(simple_wildcard_match("src_main_file", "src*main*"));
        assert!(!simple_wildcard_match("main/src", "src*main*"));
        assert!(!simple_wildcard_match("source/main", "src*main*"));

        assert!(simple_wildcard_match("test.spec.js", "test*spec*"));
        assert!(simple_wildcard_match("test_unit_spec_file", "test*spec*"));
        assert!(!simple_wildcard_match("spec.test.js", "test*spec*"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_all_wildcards() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("anything", "**"));
        assert!(simple_wildcard_match("", "**"));
        assert!(simple_wildcard_match("test", "***"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_empty_segments() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("test123", "*test*"));
        assert!(simple_wildcard_match("prefixtest", "*test*"));
        assert!(simple_wildcard_match("testsuffix", "*test*"));
        assert!(simple_wildcard_match("prefixtestsuffix", "*test*"));
        Ok(())
    }

    #[test]
    fn test_simple_wildcard_match_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("a", "*a*"));
        assert!(simple_wildcard_match("a", "a*"));
        assert!(simple_wildcard_match("a", "*a"));
        assert!(!simple_wildcard_match("", "a*"));
        assert!(!simple_wildcard_match("", "*a"));

        assert!(!simple_wildcard_match("Test.RS", "*.rs"));
        assert!(simple_wildcard_match("Test.RS", "*.RS"));
        Ok(())
    }

    #[test]
    fn test_pattern_optimization_integration() -> Result<(), Box<dyn std::error::Error>> {
        let simple_patterns = ["*.rs", "test*", "*", "test*.log"];
        for pattern in &simple_patterns {
            assert_eq!(
                pattern_to_regex(pattern),
                None,
                "Pattern {} should use simple matching",
                pattern
            );
        }

        let complex_patterns = ["*.test.*", "*foo*bar*baz*"];
        for pattern in &complex_patterns {
            assert!(
                pattern_to_regex(pattern).is_some(),
                "Pattern {} should use regex",
                pattern
            );
        }

        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match("test_file", "test*"));
        assert!(simple_wildcard_match("anything", "*"));
        assert!(simple_wildcard_match("test_main.log", "test*.log"));
        Ok(())
    }

    #[test]
    fn test_real_world_patterns() -> Result<(), Box<dyn std::error::Error>> {
        assert!(simple_wildcard_match("main.ts", "*.ts"));
        assert!(simple_wildcard_match("component.tsx", "*.tsx"));
        assert!(simple_wildcard_match("app.js", "*.js"));
        assert!(simple_wildcard_match("module.mjs", "*.mjs"));

        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match("lib.rs", "*.rs"));

        assert!(simple_wildcard_match("main.py", "*.py"));
        assert!(simple_wildcard_match("__init__.py", "*.py"));

        assert!(simple_wildcard_match("main.go", "*.go"));

        assert!(simple_wildcard_match("main.c", "*.c"));
        assert!(simple_wildcard_match("header.h", "*.h"));
        assert!(simple_wildcard_match("source.cpp", "*.cpp"));

        assert!(simple_wildcard_match("test_main.py", "test_*"));
        assert!(simple_wildcard_match("main_test.go", "*_test.go"));
        assert!(simple_wildcard_match("test.spec.js", "*.spec.js"));
        assert!(simple_wildcard_match("component.spec.js", "*.spec.js"));
        assert!(!simple_wildcard_match("spec.js", "*.spec.js"));
        Ok(())
    }
}
