//! Filename-vs-glob pattern matching used across the detection pipeline.
//!
//! Matching is a direct, allocation-free string scan with fast paths for
//! extension (`*.rs`) and prefix (`test*`) shapes. (A DashMap memo once
//! wrapped `matches_pattern` and was removed after benchmarking: a cache HIT
//! cost ~6× more than computing the match, and removal cut end-to-end
//! detection time by ~70%. Don't reintroduce it without new numbers.)

/// Match a filename against a glob-ish pattern: exact, `*`, `*.ext`,
/// `prefix*`, or general wildcards via [`simple_wildcard_match`].
pub fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return file_name == pattern;
    }

    if pattern == "*" {
        return true;
    }

    if let Some(ext) = pattern.strip_prefix("*.") {
        if !ext.contains('*') {
            return extension_match(file_name, ext);
        }
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        if !prefix.contains('*') {
            // starts_with compares bytes without slicing, so multibyte
            // filenames can't panic it
            return file_name.starts_with(prefix);
        }
    }

    simple_wildcard_match(file_name, pattern)
}

/// `*.ext` fast path: filename must end with `.ext`.
fn extension_match(file_name: &str, extension: &str) -> bool {
    if file_name.len() <= extension.len() {
        return false;
    }

    let start_pos = file_name.len() - extension.len() - 1;
    if start_pos >= file_name.len() || file_name.as_bytes()[start_pos] != b'.' {
        return false;
    }

    file_name[start_pos + 1..] == *extension
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

    let last_idx = parts.len() - 1;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }

        // A non-empty final part means the pattern doesn't end with '*':
        // glob semantics require the text to END with it
        if i == last_idx {
            return text[search_start..].ends_with(part);
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
    fn test_matches_pattern_exact_and_extension() -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches_pattern("package.json", "package.json"));
        assert!(!matches_pattern("package.json", "Cargo.toml"));
        assert!(matches_pattern("main.rs", "*.rs"));
        assert!(!matches_pattern("main.js", "*.rs"));
        assert!(!matches_pattern("main", "*.rs"));
        Ok(())
    }

    #[test]
    fn test_matches_pattern_prefix_and_wildcard() -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches_pattern("test_file.rs", "test*"));
        assert!(!matches_pattern("my_test", "test*"));
        assert!(matches_pattern("anything", "*"));
        assert!(matches_pattern("", "*"));
        assert!(matches_pattern("test_config_file.json", "*config*"));
        assert!(!matches_pattern("settings.json", "*config*"));
        assert!(matches_pattern("test", "*test"));
        assert!(!matches_pattern("", "test"));
        Ok(())
    }

    #[test]
    fn test_matches_pattern_multibyte_prefix_does_not_panic() {
        // A byte-sliced prefix check would split the second ü and panic
        assert!(!matches_pattern("tüüxyz", "test*"));
        assert!(matches_pattern("türkçe.rs", "tü*"));
    }

    #[test]
    fn test_matches_pattern_dot_prefix_with_second_wildcard() {
        // The "*.ext" fast path must not swallow patterns with more wildcards
        assert!(matches_pattern("foo.test.js", "*.test.*"));
        assert!(!matches_pattern("footest.js", "*.test.*"));
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
        assert!(simple_wildcard_match("main.rs", "*.rs"));
        assert!(simple_wildcard_match("test_file", "test*"));
        assert!(simple_wildcard_match("anything", "*"));
        assert!(simple_wildcard_match("test_main.log", "test*.log"));
        assert!(simple_wildcard_match("foo.test.js", "*.test.*"));
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

    #[test]
    fn test_multi_wildcard_trailing_literal_is_anchored() {
        // Glob semantics: a pattern not ending in '*' must match to the end
        assert!(!simple_wildcard_match("a-b-c", "*a*b"));
        assert!(simple_wildcard_match("xa-b", "*a*b"));
    }
}
