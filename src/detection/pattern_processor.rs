use crate::patterns::matches_pattern;
use crate::types::MatchedFile;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct PatternProcessor {
    /// `*.ext` patterns (ext without dots or wildcards), by extension
    extension_patterns: Arc<HashSet<String>>,
    exact_patterns: Arc<HashSet<String>>,
    /// Everything else (`test*`, `*config*`, `*.tar.gz`, …): the only
    /// patterns a non-matching file has to be checked against one by one
    glob_patterns: Arc<[String]>,
}

impl PatternProcessor {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(patterns: Arc<[String]>) -> Self {
        let mut extension_patterns = HashSet::new();
        let mut exact_patterns = HashSet::new();
        let mut glob_patterns = Vec::new();

        for pattern in patterns.iter() {
            if !pattern.contains('*') {
                exact_patterns.insert(pattern.clone());
            } else if let Some(ext) = pattern
                .strip_prefix("*.")
                .filter(|ext| !ext.contains(['*', '.']))
            {
                extension_patterns.insert(ext.to_string());
            } else {
                glob_patterns.push(pattern.clone());
            }
        }

        Self {
            extension_patterns: Arc::new(extension_patterns),
            exact_patterns: Arc::new(exact_patterns),
            glob_patterns: glob_patterns.into(),
        }
    }

    /// Whether any pattern matches `filename`. Most files match nothing, so
    /// the common case costs two hash lookups plus the few general globs.
    pub fn should_scan_file(&self, filename: &str) -> bool {
        self.exact_patterns.contains(filename)
            || filename
                .rsplit_once('.')
                .is_some_and(|(_, ext)| self.extension_patterns.contains(ext))
            || self
                .glob_patterns
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

        self.should_scan_file(filename)
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
pub(crate) fn join_components(relative_path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = relative_path
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect();
    parts.map(|p| p.join("/"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bare_extension_name_is_not_an_extension_match() {
        let processor = PatternProcessor::new(Arc::from(vec!["*.rs".to_string()]));
        assert!(!processor.should_scan_file("rs"));
        assert!(processor.should_scan_file("main.rs"));
    }

    use super::*;

    fn processor(patterns: &[&str]) -> PatternProcessor {
        PatternProcessor::new(patterns.iter().map(|s| s.to_string()).collect())
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
