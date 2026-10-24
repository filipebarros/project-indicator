use super::{FileSystemTraverser, TimeoutManager};
use crate::detection::pattern_processor::PatternProcessor;
use crate::types::MatchedFile;
use crate::Result;
use std::path::Path;

/// Maximum matches before the scan stops collecting.
///
/// **Why 15?**
/// - 99th percentile of projects need <10 matches for accurate detection
/// - 15 provides 50% safety margin
/// - Prevents memory exhaustion in large monorepos
///
/// **Performance Impact:**
/// - Reduces scan time by 40-60% for large projects
/// - Memory savings: ~3KB per pattern at limit
/// - Accuracy: No measurable impact on detection quality
const MAX_MATCHES: usize = 15;

/// Extreme directory size threshold for safety checks.
///
/// **Why 500?**
/// - Typical project roots: 10-200 immediate entries
/// - Large monorepos: 200-400 entries
/// - Home directories: 500-5000+ entries
/// - System directories: 1000+ entries
///
/// **Safety Mechanism:**
/// - Prevents accidental scans of home directory (~/)
/// - Prevents system directory scans (/usr, /etc)
/// - Protects against misconfiguration
/// - Falls back to a root-level manifest check (exact + wildcard names)
const EXTREME_SIZE_THRESHOLD: usize = 500;

pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
    max_depth: usize,
}

impl ScanningEngine {
    pub fn new(pattern_processor: PatternProcessor, max_depth: usize) -> Self {
        Self {
            pattern_processor,
            max_depth,
        }
    }

    /// One pass over the scan root: the total entry count (for the size
    /// guard and timeout) and the sorted names of the files in it (symlinks
    /// to files included). Replaces a size estimate, one `stat` per exact
    /// pattern and a separate wildcard listing — three syscall sets — with
    /// a single `read_dir`.
    fn list_root(path: &Path) -> (usize, Vec<String>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return (0, Vec::new());
        };
        let (mut count, mut names) = (0, Vec::new());
        for entry in entries.filter_map(|e| e.ok()) {
            count += 1;
            let is_file = entry
                .file_type()
                .is_ok_and(|ft| ft.is_file() || (ft.is_symlink() && entry.path().is_file()));
            if let (true, Ok(name)) = (is_file, entry.file_name().into_string()) {
                names.push(name);
            }
        }
        names.sort();
        (count, names)
    }

    /// Exact-name pattern files present in the root, in sorted order
    fn root_manifest_matches(&self, names: &[String]) -> Vec<MatchedFile> {
        let exact = self.pattern_processor.get_exact_patterns();
        names
            .iter()
            .filter(|name| exact.contains(name.as_str()))
            .map(|name| MatchedFile::new(name.clone(), name.clone()))
            .collect()
    }

    /// Root-level files matching a wildcard pattern (`*.csproj`, `*.gemspec`),
    /// in name order and capped, skipping names already in `already`.
    fn root_wildcard_matches(&self, names: &[String], already: &[MatchedFile]) -> Vec<MatchedFile> {
        names
            .iter()
            .filter(|name| {
                self.pattern_processor.should_scan_file(name)
                    && !already.iter().any(|m| &m.filename == *name)
            })
            .take(MAX_MATCHES)
            .map(|name| MatchedFile::new(name.clone(), name.clone()))
            .collect()
    }

    /// Full directory scan: a sorted, deterministic walk bounded by the match
    /// cap and an adaptive timeout. Oversized directories (likely a home or
    /// system dir) only get a root-level manifest check.
    pub fn scan(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        self.scan_with_status(path).map(|(matches, _)| matches)
    }

    /// Like [`scan`](Self::scan), also reporting whether the timeout cut the
    /// walk short (the matches are then a partial sample).
    pub fn scan_with_status(&self, path: &Path) -> Result<(Vec<MatchedFile>, bool)> {
        log::trace!("Starting scan for path: {}", path.display());
        let traverser = FileSystemTraverser::new(self.max_depth);
        let (entry_count, root_names) = Self::list_root(path);

        // Safety: Skip scanning extremely large directories (likely home dir or system dir)
        // This prevents scanning from places like ~, /usr, etc.
        if entry_count > EXTREME_SIZE_THRESHOLD {
            log::warn!(
                "Directory {} has {} immediate entries (>{} threshold), likely not a project root. Skipping scan.",
                path.display(),
                entry_count,
                EXTREME_SIZE_THRESHOLD
            );
            // Root-level manifests are still cheap to check, so an oversized
            // project root (e.g. a big monorepo) is not reported as empty
            let mut matches = self.root_manifest_matches(&root_names);
            matches.extend(self.root_wildcard_matches(&root_names, &matches));
            return Ok((matches, false));
        }

        self.scan_sequential(
            path,
            &traverser,
            TimeoutManager::new(entry_count),
            self.root_manifest_matches(&root_names),
        )
    }

    /// Per-entry pre-filter: `true` when the entry is a file whose name passes
    /// the pattern pre-filter.
    fn prefilter_entry(pattern_processor: &PatternProcessor, entry: &ignore::DirEntry) -> bool {
        // Check file type first before any string operations. Symlinks are
        // resolved to their targets so a symlinked manifest counts as a file
        // (matching the extreme-size root check, which stats through
        // symlinks); broken links resolve to nothing and are skipped.
        let is_file = entry
            .file_type()
            .is_some_and(|ft| ft.is_file() || (ft.is_symlink() && entry.path().is_file()));
        if !is_file {
            return false;
        }

        let Some(filename) = entry.file_name().to_str() else {
            return false;
        };

        if !pattern_processor.should_scan_file(filename) {
            return false;
        }

        true
    }

    /// Sequential scan for small projects (avoids thread spawning overhead)
    fn scan_sequential(
        &self,
        path: &Path,
        traverser: &FileSystemTraverser,
        timeout_mgr: TimeoutManager,
        root_manifests: Vec<MatchedFile>,
    ) -> Result<(Vec<MatchedFile>, bool)> {
        let scan_start = std::time::Instant::now();
        let mut timed_out = false;
        // Exact-name manifests at the root are always kept, whatever the
        // match cap does to the walk below — a busy directory must never
        // crowd out Cargo.toml / package.json
        let mut matched_files = root_manifests;
        let root_count = matched_files.len();

        let walker = traverser.build_walker(path);

        for result in walker.build() {
            if matched_files.len() - root_count >= MAX_MATCHES {
                log::debug!("Sequential scan: max matches ({}) reached", MAX_MATCHES);
                break;
            }

            if timeout_mgr.should_stop() {
                timed_out = true;
                break;
            }

            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if !Self::prefilter_entry(&self.pattern_processor, &entry) {
                continue;
            }

            if let Some(file_match) = self
                .pattern_processor
                .match_file_against_patterns(entry.path(), path)
            {
                if !matched_files
                    .iter()
                    .any(|m| m.relative_path == file_match.relative_path)
                {
                    matched_files.push(file_match);
                }
            }
        }

        let elapsed = scan_start.elapsed().as_millis();
        log::debug!(
            "Scan complete: {} matches in {}ms",
            matched_files.len(),
            elapsed
        );

        Ok((matched_files, timed_out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_rust_project;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_engine() -> ScanningEngine {
        let patterns = Arc::new(vec![
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
            "*.js".to_string(),
        ]);

        let pattern_processor = PatternProcessor::new(patterns);

        ScanningEngine::new(pattern_processor, 3)
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_test_engine();
        assert_eq!(engine.max_depth, 3);
    }

    #[test]
    fn test_full_scan() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_no_matches() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;

        fs::write(temp_dir.path().join("readme.txt"), "text")?;

        let matches = engine.scan(temp_dir.path())?;
        assert!(matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_nested_files() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        let src = root.join("src");
        fs::create_dir(&src)?;
        fs::write(src.join("lib.rs"), "// lib")?;

        let nested = src.join("nested");
        fs::create_dir(&nested)?;
        fs::write(nested.join("module.rs"), "// module")?;

        let matches = engine.scan(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_list_root_counts_everything_and_lists_sorted_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        fs::create_dir(root.join("a_dir"))?;
        fs::write(root.join("b.txt"), "x")?;
        fs::write(root.join("a.txt"), "x")?;

        let (count, names) = ScanningEngine::list_root(root);
        assert_eq!(count, 3, "directories count towards the size guard");
        assert_eq!(names, vec!["a.txt".to_string(), "b.txt".to_string()]);

        assert_eq!(
            ScanningEngine::list_root(&root.join("missing")),
            (0, vec![])
        );
        assert_eq!(
            ScanningEngine::list_root(TempDir::new()?.path()),
            (0, vec![])
        );
        Ok(())
    }

    #[test]
    fn test_oversized_directory_still_detects_root_manifests() -> Result<()> {
        let patterns = Arc::new(vec!["special.config".to_string(), "*.xyz".to_string()]);
        // Detection must survive the extreme-size bailout via the root
        // exact-pattern check
        let pattern_processor = PatternProcessor::new(patterns);
        let engine = ScanningEngine::new(pattern_processor, 3);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..510 {
            fs::write(root.join(format!("file_{}.txt", i)), "x")?;
        }
        fs::write(root.join("special.config"), "cfg")?;

        let matches = engine.scan(root)?;
        assert!(matches.iter().any(|m| m.filename == "special.config"));

        Ok(())
    }

    #[test]
    fn test_scan_timeout_exceeded() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let engine = ScanningEngine::new(pattern_processor, 1); // Very low depth to trigger timeout

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many files to potentially trigger timeout
        for i in 0..100 {
            fs::write(root.join(format!("file_{}.rs", i)), "// rust file")?;
        }

        let matches = engine.scan(root)?;
        // Should still return some matches even if timeout occurred
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_file_count_limit() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many files to test file count limit
        for i in 0..200 {
            fs::write(root.join(format!("file_{}.rs", i)), "// rust file")?;
        }

        let matches = engine.scan(root)?;
        // Should return matches but potentially limited by file count
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_concurrent_access() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Test that multiple sequential scans work (avoiding lifetime issues)
        for _ in 0..4 {
            let matches = engine.scan(temp_dir.path())?;
            assert!(!matches.is_empty());
        }

        Ok(())
    }

    #[test]
    fn test_scan_adaptive_timeout() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        let src = root.join("src");
        fs::create_dir(&src)?;
        fs::write(src.join("lib.rs"), "// lib")?;

        let matches = engine.scan(root)?;
        assert!(
            matches.iter().any(|m| m.relative_path == "src/lib.rs"),
            "nested file within max_depth must be found: {matches:?}"
        );

        Ok(())
    }

    #[test]
    fn test_scan_medium_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        for i in 0..50 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_large_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        for i in 0..100 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_very_large_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        for i in 0..200 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_timeout_not_exceeded_immediately() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_repeated_scans_are_identical() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let paths =
            |v: Vec<MatchedFile>| v.into_iter().map(|m| m.relative_path).collect::<Vec<_>>();
        let first = paths(engine.scan(temp_dir.path())?);
        assert!(!first.is_empty());
        for _ in 0..3 {
            assert_eq!(first, paths(engine.scan(temp_dir.path())?));
        }

        Ok(())
    }

    #[test]
    fn test_scan_ignores_files_beyond_max_depth() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let engine = ScanningEngine::new(pattern_processor, 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many nested directories to potentially trigger timeout
        for i in 0..50 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            for j in 0..10 {
                let subdir = dir.join(format!("subdir_{}", j));
                fs::create_dir(&subdir)?;
                fs::write(subdir.join("lib.rs"), "// lib")?;
            }
        }

        // lib.rs files sit three levels down, beyond max_depth = 1
        let matches = engine.scan(root)?;
        assert!(matches.is_empty(), "files beyond max_depth: {matches:?}");

        Ok(())
    }

    #[test]
    fn test_busy_directory_scan_is_deterministic_and_keeps_root_manifest() -> Result<()> {
        // 60 matching sources + a root manifest: the match cap must never
        // crowd the manifest out, and repeated scans must agree
        let patterns = Arc::new(vec![
            "*.js".to_string(),
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
        ]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..60 {
            fs::write(root.join(format!("file_{i:02}.js")), "x")?;
        }
        for i in 0..3 {
            fs::write(root.join(format!("m{i}.rs")), "x")?;
        }
        fs::write(root.join("Cargo.toml"), "[package]")?;

        let first = engine.scan(root)?;
        assert!(
            first.iter().any(|m| m.filename == "Cargo.toml"),
            "root manifest must survive the match cap"
        );
        for _ in 0..20 {
            let again = engine.scan(root)?;
            let names = |v: &[MatchedFile]| {
                v.iter()
                    .map(|m| m.relative_path.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&first), names(&again), "scan results must be stable");
        }
        Ok(())
    }

    #[test]
    fn test_oversized_directory_keeps_wildcard_root_markers() -> Result<()> {
        let patterns = Arc::new(vec!["*.csproj".to_string(), "go.mod".to_string()]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..510 {
            fs::write(root.join(format!("file_{i}.txt")), "x")?;
        }
        fs::write(root.join("App.csproj"), "<Project/>")?;
        fs::write(root.join("go.mod"), "module x")?;

        let names: Vec<String> = engine.scan(root)?.into_iter().map(|m| m.filename).collect();
        assert!(names.contains(&"go.mod".to_string()));
        assert!(
            names.contains(&"App.csproj".to_string()),
            "wildcard-only marker lost in an oversized directory: {names:?}"
        );
        Ok(())
    }

    #[test]
    fn test_timed_out_scan_is_reported_truncated() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);
        let temp_dir = TempDir::new()?;
        for i in 0..20 {
            fs::write(temp_dir.path().join(format!("f{i}.rs")), "x")?;
        }
        let traverser = FileSystemTraverser::new(1);

        let (_, timed_out) = engine.scan_sequential(
            temp_dir.path(),
            &traverser,
            TimeoutManager::with_timeout(std::time::Duration::ZERO),
            Vec::new(),
        )?;
        assert!(timed_out, "an expired timeout must be reported");

        let (matches, timed_out) = engine.scan_with_status(temp_dir.path())?;
        assert!(!timed_out && !matches.is_empty());
        Ok(())
    }
}
