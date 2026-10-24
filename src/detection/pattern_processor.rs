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
            if pattern.contains('*') {
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

        if let Some((_, ext)) = filename.rsplit_once('.') {
            if self.extension_filter.contains(ext) {
                return true;
            }
        }

        self.unique_patterns
            .iter()
            .any(|pattern| matches_pattern(filename, pattern))
    }

    /// Match `file_path` (relative to `base_path`) against the configured
    /// patterns. `relative_path` always uses `/` separators, whatever the OS,
    /// so depth and directory classification behave the same on Windows.
    pub fn match_file_against_patterns(
        &self,
        file_path: &Path,
        base_path: &Path,
    ) -> Option<MatchedFile> {
        let relative_path = file_path.strip_prefix(base_path).ok()?;
        let filename = relative_path.file_name().and_then(|n| n.to_str())?;
        let relative_str = join_components(relative_path)?;

        self.unique_patterns
            .iter()
            .any(|pattern| matches_pattern(filename, pattern))
            .then(|| MatchedFile::new(filename.to_string(), relative_str))
    }

    pub fn get_exact_patterns(&self) -> &HashSet<String> {
        &self.exact_patterns
    }
}

/// `path` relative to `base`, joined with `/` (None for non-UTF-8 components)
pub(crate) fn normalized_relative(path: &Path, base: &Path) -> Option<String> {
    join_components(path.strip_prefix(base).ok()?)
}

/// Join path components with `/` (None for non-UTF-8 components)
fn join_components(relative_path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = relative_path
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect();
    parts.map(|p| p.join("/"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_fast_path_agrees_with_full_matcher() {
        let processor = PatternProcessor::new(Arc::new(vec!["*.rs".to_string()]));
        for name in [".rs", "..rs", "a.rs", "a.b.rs", "rs.", "x.rs.bak", ".rs.rs"] {
            assert_eq!(
                processor.should_scan_file(name),
                matches_pattern(name, "*.rs"),
                "{name}"
            );
        }
    }

    #[test]
    fn test_bare_extension_name_is_not_an_extension_match() {
        let processor = PatternProcessor::new(Arc::new(vec!["*.rs".to_string()]));
        assert!(!processor.should_scan_file("rs"));
        assert!(processor.should_scan_file("main.rs"));
    }

    use super::*;

    fn processor(patterns: &[&str]) -> PatternProcessor {
        PatternProcessor::new(Arc::new(patterns.iter().map(|s| s.to_string()).collect()))
    }

    #[test]
    fn test_relative_path_uses_forward_slashes() {
        let p = processor(&["*.rs"]);
        let m = p
            .match_file_against_patterns(Path::new("/x/sub/dir/f.rs"), Path::new("/x"))
            .map(|m| (m.filename, m.relative_path));
        assert_eq!(m, Some(("f.rs".to_string(), "sub/dir/f.rs".to_string())));
    }

    #[test]
    fn test_non_matching_file_is_none() {
        let p = processor(&["*.rs"]);
        assert!(p
            .match_file_against_patterns(Path::new("/x/readme.md"), Path::new("/x"))
            .is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_backslash_paths_are_normalized() {
        let p = processor(&["package.json"]);
        let m = p
            .match_file_against_patterns(
                Path::new(r"C:\proj\node_modules\x\package.json"),
                Path::new(r"C:\proj"),
            )
            .map(|m| m.relative_path);
        assert_eq!(m, Some("node_modules/x/package.json".to_string()));
    }
}
