//! Filename-vs-glob pattern matching used across the detection pipeline.
//!
//! Matching is a direct, allocation-free string scan. Don't memoize it: a
//! cache hit benchmarked ~6× slower than computing the match.

/// Match `text` against a glob where `*` matches any run of characters.
/// A single `*` (`*.rs`, `test*`, `a*b`) is one prefix/suffix comparison;
/// prefix and suffix must not overlap in the text.
pub fn matches_pattern(text: &str, pattern: &str) -> bool {
    let Some((prefix, rest)) = pattern.split_once('*') else {
        return text == pattern;
    };
    if !rest.contains('*') {
        return text.len() >= prefix.len() + rest.len()
            && text.starts_with(prefix)
            && text.ends_with(rest);
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let first_part = parts[0];
    let mut search_start = 0;
    if !first_part.is_empty() {
        if !text.starts_with(first_part) {
            return false;
        }
        search_start = first_part.len();
    }

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

        match text[search_start..].find(part) {
            Some(pos) => search_start += pos + part.len(),
            None => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern_multibyte_prefix_does_not_panic() {
        // A byte-sliced prefix check would split the second ü and panic
        assert!(!matches_pattern("tüüxyz", "test*"));
        assert!(matches_pattern("türkçe.rs", "tü*"));
    }

    #[test]
    fn test_matches_pattern_dot_prefix_with_second_wildcard() {
        // A trailing "*.ext" must not swallow patterns with more wildcards
        assert!(matches_pattern("foo.test.js", "*.test.*"));
        assert!(!matches_pattern("footest.js", "*.test.*"));
    }

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("package.json", "package.json"));
        assert!(matches_pattern("", ""));
        assert!(!matches_pattern("package.json", "Cargo.toml"));
        assert!(!matches_pattern("test", ""));
        assert!(!matches_pattern("", "test"));
    }

    #[test]
    fn test_matches_pattern_prefix() {
        assert!(matches_pattern("test.rs", "test*"));
        assert!(matches_pattern("test", "test*"));
        assert!(matches_pattern("testing123", "test*"));
        assert!(!matches_pattern("mytest", "test*"));
        assert!(!matches_pattern("", "test*"));
    }

    #[test]
    fn test_matches_pattern_suffix() {
        assert!(matches_pattern("main.rs", "*.rs"));
        assert!(matches_pattern(".rs", "*.rs"));
        assert!(matches_pattern("very.long.file.rs", "*.rs"));
        assert!(!matches_pattern("rs", "*.rs"));
        assert!(!matches_pattern("main.toml", "*.rs"));
        assert!(!matches_pattern("", "*.rs"));
    }

    #[test]
    fn test_matches_pattern_overlapping_patterns() {
        // Glob semantics: prefix and suffix must not overlap in the text
        // ("test" is not "test" + "" + "t")
        assert!(!matches_pattern("test", "test*t"));
        assert!(!matches_pattern("hello", "hello*o"));
        assert!(!matches_pattern("abc", "ab*bc"));
        assert!(!matches_pattern("abcd", "abc*cd"));
        assert!(!matches_pattern("test", "test*st"));
        assert!(!matches_pattern("test", "test*test"));
        assert!(!matches_pattern("aa", "aa*aa"));
        assert!(matches_pattern("testt", "test*t"));
        assert!(matches_pattern("aaaa", "aa*aa"));

        assert!(!matches_pattern("abc", "abc*def"));
        assert!(!matches_pattern("hello", "hell*world"));

        assert!(!matches_pattern("ab", "abc*abc"));
        assert!(matches_pattern("abcabc", "abc*abc"));
    }

    #[test]
    fn test_matches_pattern_edge_cases() {
        assert!(matches_pattern("a", "*a*"));
        assert!(matches_pattern("a", "a*"));
        assert!(matches_pattern("a", "*a"));
        assert!(!matches_pattern("", "a*"));
        assert!(!matches_pattern("", "*a"));

        assert!(!matches_pattern("Test.RS", "*.rs"));
        assert!(matches_pattern("Test.RS", "*.RS"));
    }
}
