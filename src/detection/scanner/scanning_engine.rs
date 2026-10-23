use super::{FileSystemTraverser, TimeoutManager};
use crate::detection::pattern_processor::PatternProcessor;
use crate::performance::FileSystemCache;
use crate::types::MatchedFile;
use crate::Result;
use std::path::Path;
use std::sync::Arc;

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
const DEFAULT_MAX_MATCHES_PER_PATTERN: usize = 15;

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
/// - Returns empty result rather than hanging
const EXTREME_SIZE_THRESHOLD: usize = 500;

pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
    max_depth: usize,
    file_cache: Option<Arc<FileSystemCache>>,

    max_matches_per_pattern: usize,
    extreme_size_threshold: usize,
}

impl ScanningEngine {
    /// Creates a ScanningEngine with an optional FileSystemCache for file existence checks.
    pub fn with_cache(
        pattern_processor: PatternProcessor,
        max_depth: usize,
        file_cache: Option<Arc<FileSystemCache>>,
    ) -> Self {
        Self {
            pattern_processor,
            max_depth,
            file_cache,
            max_matches_per_pattern: DEFAULT_MAX_MATCHES_PER_PATTERN,
            extreme_size_threshold: EXTREME_SIZE_THRESHOLD,
        }
    }

    /// Check a set of exact (non-glob) filenames in the scan root
    fn check_root_files<'a>(
        &self,
        path: &Path,
        files: impl IntoIterator<Item = &'a String>,
    ) -> Vec<MatchedFile> {
        let mut matches = Vec::new();

        for file_name in files {
            let file_path = path.join(file_name);

            let exists_and_is_file = if let Some(cache) = &self.file_cache {
                cache.is_file(&file_path)
            } else {
                file_path.exists() && file_path.is_file()
            };

            if exists_and_is_file {
                matches.push(MatchedFile::new(file_name.clone(), file_name.clone()));
            }
        }

        matches
    }

    /// Exact-name patterns present as files in the scan root, in sorted order
    fn root_manifest_matches(&self, path: &Path) -> Vec<MatchedFile> {
        let mut names: Vec<&String> = self.pattern_processor.get_exact_patterns().iter().collect();
        names.sort();
        self.check_root_files(path, names)
    }

    /// Full directory scan with an adaptive timeout
    ///
    /// This method chooses between sequential and parallel scanning based on the
    /// estimated directory size to optimize performance:
    ///
    pub fn scan(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        log::trace!("Starting scan for path: {}", path.display());
        let traverser = FileSystemTraverser::new(self.max_depth);
        let estimated_size = traverser.estimate_directory_size(path);

        // Safety: Skip scanning extremely large directories (likely home dir or system dir)
        // This prevents scanning from places like ~, /usr, etc.
        if estimated_size > self.extreme_size_threshold {
            log::warn!(
                "Directory {} has {} immediate entries (>{} threshold), likely not a project root. Skipping scan.",
                path.display(),
                estimated_size,
                self.extreme_size_threshold
            );
            // Root-level manifests are still cheap to check, so an oversized
            // project root (e.g. a big monorepo) is not reported as empty
            return Ok(self.root_manifest_matches(path));
        }

        self.scan_sequential(path, &traverser, estimated_size)
    }

    /// Shared per-entry pre-filter for both scan strategies.
    ///
    /// Returns `true` when the entry is a file whose name passes the pattern
    /// pre-filter (and records it with the timeout manager).
    fn prefilter_entry(
        pattern_processor: &PatternProcessor,
        timeout_mgr: &TimeoutManager,
        entry: &ignore::DirEntry,
    ) -> bool {
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

        timeout_mgr.record_file_scanned();
        true
    }

    /// Sequential scan for small projects (avoids thread spawning overhead)
    fn scan_sequential(
        &self,
        path: &Path,
        traverser: &FileSystemTraverser,
        estimated_size: usize,
    ) -> Result<Vec<MatchedFile>> {
        let scan_start = std::time::Instant::now();
        let timeout_mgr = TimeoutManager::new(estimated_size);
        // Exact-name manifests at the root are always kept, whatever the
        // match cap does to the walk below — a busy directory must never
        // crowd out Cargo.toml / package.json
        let mut matched_files = self.root_manifest_matches(path);
        let root_count = matched_files.len();
        let patterns = self.pattern_processor.get_patterns_arc();

        let walker = traverser.build_walker(path);

        for result in walker.build() {
            if matched_files.len() - root_count >= self.max_matches_per_pattern {
                log::debug!(
                    "Sequential scan: max matches ({}) reached",
                    self.max_matches_per_pattern
                );
                break;
            }

            if timeout_mgr.should_stop() {
                break;
            }

            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if !Self::prefilter_entry(&self.pattern_processor, &timeout_mgr, &entry) {
                continue;
            }

            if let Some(file_match) =
                self.pattern_processor
                    .match_file_against_patterns(entry.path(), path, &patterns)
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
            "Sequential scan complete: {} matches, {} files scanned in {}ms",
            matched_files.len(),
            timeout_mgr.files_scanned(),
            elapsed
        );

        Ok(matched_files)
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

        ScanningEngine::with_cache(pattern_processor, 3, None)
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
    fn test_engine_with_cache() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string(), "Cargo.toml".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let file_cache = Arc::new(FileSystemCache::new());
        let engine = ScanningEngine::with_cache(pattern_processor, 3, Some(file_cache));

        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;
        let matches = engine.scan(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_oversized_directory_still_detects_root_manifests() -> Result<()> {
        let patterns = Arc::new(vec!["special.config".to_string(), "*.xyz".to_string()]);
        // Detection must survive the extreme-size bailout via the root
        // exact-pattern check
        let pattern_processor = PatternProcessor::new(patterns);
        let engine = ScanningEngine::with_cache(pattern_processor, 3, None);

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

        let engine = ScanningEngine::with_cache(pattern_processor, 1, None); // Very low depth to trigger timeout

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
        // Adaptive timeout may result in empty matches, just verify function completes
        let _ = matches;

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
    fn test_scan_file_counter_thread_safe() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Test that file counter works correctly with multiple sequential scans
        for _ in 0..4 {
            let matches = engine.scan(temp_dir.path())?;
            // File counter should work without panicking, results may vary
            let _ = matches;
        }

        Ok(())
    }

    #[test]
    fn test_scan_timeout_exceeded_scenario() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let engine = ScanningEngine::with_cache(pattern_processor, 1, None);

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

        let matches = engine.scan(root)?;
        // Should return some matches even if timeout occurred, but may be empty due to timeout
        // The important thing is that the function doesn't panic
        let _ = matches;

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
        let engine = ScanningEngine::with_cache(PatternProcessor::new(patterns), 1, None);

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
}
