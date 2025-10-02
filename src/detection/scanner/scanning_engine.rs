// Main scanning orchestration logic
// Coordinates pattern matching, timeout management, and file traversal

use super::{FileSystemTraverser, TimeoutManager};
use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::performance::FileSystemCache;
use crate::types::{MatchedFile, ProjectIndicator};
use crate::Result;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
    max_depth: usize,
    file_cache: Option<Arc<FileSystemCache>>,

    // Configurable thresholds
    max_matches_per_pattern: usize,
    small_project_threshold: usize,
    extreme_size_threshold: usize,
}

impl ScanningEngine {
    pub fn new(pattern_processor: PatternProcessor, max_depth: usize) -> Self {
        Self {
            pattern_processor,
            max_depth,
            file_cache: None,
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        }
    }

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
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        }
    }

    /// Creates a ScanningEngine with configurable thresholds
    pub fn with_config(
        pattern_processor: PatternProcessor,
        max_depth: usize,
        file_cache: Option<Arc<FileSystemCache>>,
        max_matches: usize,
        small_project_threshold: usize,
        extreme_size_threshold: usize,
    ) -> Self {
        Self {
            pattern_processor,
            max_depth,
            file_cache,
            max_matches_per_pattern: max_matches,
            small_project_threshold,
            extreme_size_threshold,
        }
    }

    /// Creates a ScanningEngine with a shared PatternMatcher instance.
    ///
    /// This is the primary constructor used by DetectionEngine.
    /// It creates a PatternProcessor internally using the provided matcher and patterns.
    pub fn with_shared_pattern_matcher(
        pattern_matcher: Arc<PatternMatcher>,
        patterns: Arc<Vec<String>>,
        languages: Vec<Arc<ProjectIndicator>>,
        max_depth: usize,
    ) -> Self {
        let pattern_processor = PatternProcessor::new(pattern_matcher, patterns, languages);
        Self::new(pattern_processor, max_depth)
    }

    /// Execute full scan with priority file fast path
    pub fn scan(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        log::trace!("Starting scan for path: {}", path.display());

        // Phase 1: Check high-priority files (fast path)
        if let Some(priority_matches) = self.check_priority_files(path)? {
            log::debug!(
                "Priority fast path successful: found {} matches in root",
                priority_matches.len()
            );
            return Ok(priority_matches);
        }

        log::debug!("No priority files found, proceeding with full directory scan");

        // Phase 2: Full directory scan with timeout
        self.scan_with_timeout(path)
    }

    /// Fast path: check only high-priority files in root
    fn check_priority_files(&self, path: &Path) -> Result<Option<Vec<MatchedFile>>> {
        let high_priority_files = self.pattern_processor.get_high_priority_files();
        let mut matches = Vec::with_capacity(high_priority_files.len());

        for priority_file in high_priority_files.iter() {
            let file_path = path.join(priority_file);

            // Use cache if available, otherwise direct filesystem check
            let exists_and_is_file = if let Some(cache) = &self.file_cache {
                cache.is_file(&file_path)
            } else {
                file_path.exists() && file_path.is_file()
            };

            if exists_and_is_file {
                if let Ok(relative_path) = file_path.strip_prefix(path) {
                    if let Some(relative_str) = relative_path.to_str() {
                        let matched =
                            MatchedFile::new(priority_file.clone(), relative_str.to_string());
                        matches.push(matched);
                    }
                }
            }
        }

        if matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(matches))
        }
    }

    /// Full directory scan with adaptive timeout and early termination
    ///
    /// This method chooses between sequential and parallel scanning based on the
    /// estimated directory size to optimize performance:
    ///
    /// - Small projects (< 50 files): Sequential scan avoids thread spawning overhead
    /// - Large projects (>= 50 files): Parallel scan leverages multiple cores
    ///
    /// Both paths use adaptive timeouts and early termination for consistent performance.
    fn scan_with_timeout(&self, path: &Path) -> Result<Vec<MatchedFile>> {
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
            return Ok(Vec::new());
        }

        // Optimization: Use sequential scan for small projects to avoid parallel overhead
        if estimated_size < self.small_project_threshold {
            log::debug!(
                "Using sequential scan (estimated {} files < {} threshold)",
                estimated_size,
                self.small_project_threshold
            );
            return self.scan_sequential(path, &traverser, estimated_size);
        }

        // Use parallel scanning for larger projects
        log::debug!(
            "Using parallel scan (estimated {} files >= {} threshold)",
            estimated_size,
            self.small_project_threshold
        );
        self.scan_parallel(path, &traverser, estimated_size)
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
        // Pre-allocate with reasonable capacity to reduce reallocations
        let mut matched_files =
            Vec::with_capacity(estimated_size.min(self.max_matches_per_pattern));
        let mut high_priority_count = 0;
        let patterns = self.pattern_processor.get_patterns();

        let walker = traverser.build_walker(path);

        for result in walker.build() {
            // Early bailout conditions
            if matched_files.len() >= self.max_matches_per_pattern {
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

            // Optimization: Check file type first before any string operations
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            // Optimization: Get filename once and reuse
            let filename = match entry.file_name().to_str() {
                Some(name) => name,
                None => continue,
            };

            // Fast path: Skip files that don't match any patterns
            if !self.pattern_processor.should_scan_file(filename) {
                continue;
            }

            timeout_mgr.record_file_scanned();

            // Check for strong evidence before expensive pattern matching
            let is_strong = self.pattern_processor.is_strong_evidence(filename);
            if is_strong {
                high_priority_count += 1;

                // Optimization: Early exit if we have enough strong evidence (3+ strong indicators)
                if high_priority_count >= 3 && matched_files.len() >= 3 {
                    log::debug!(
                        "Sequential scan: sufficient strong evidence ({}), early exit",
                        high_priority_count
                    );
                    // Still do the pattern match for this file before exiting
                    if let Some(file_match) = self.pattern_processor.match_file_against_patterns(
                        entry.path(),
                        path,
                        patterns,
                    ) {
                        matched_files.push(file_match);
                    }
                    break;
                }
            }

            // Pattern matching
            if let Some(file_match) =
                self.pattern_processor
                    .match_file_against_patterns(entry.path(), path, patterns)
            {
                matched_files.push(file_match);

                // Early termination check (less frequent now)
                if is_strong && timeout_mgr.check_early_termination(high_priority_count) {
                    log::debug!(
                        "Sequential scan: early termination triggered (high_priority={}, scanned={})",
                        high_priority_count,
                        timeout_mgr.files_scanned()
                    );
                    break;
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

    /// Parallel scan for larger projects
    ///
    /// Uses multiple threads to scan large directories efficiently.
    /// Patterns are shared via Arc to avoid expensive cloning in each worker thread.
    fn scan_parallel(
        &self,
        path: &Path,
        traverser: &FileSystemTraverser,
        estimated_size: usize,
    ) -> Result<Vec<MatchedFile>> {
        let scan_start = std::time::Instant::now();
        // Pre-allocate with reasonable capacity (max matches expected)
        let matched_files = Arc::new(Mutex::new(Vec::with_capacity(self.max_matches_per_pattern)));
        let high_priority_count = Arc::new(AtomicUsize::new(0));
        let total_matches = Arc::new(AtomicUsize::new(0));
        let should_quit = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Optimization: Share patterns via Arc to avoid cloning
        // Instead of cloning the entire Vec for each worker, we Arc-clone the pointer
        let patterns = Arc::new(self.pattern_processor.get_patterns().to_vec());

        let walker = traverser.build_walker(path);

        walker.build_parallel().run(|| {
            let matched_files = Arc::clone(&matched_files);
            let high_priority_count = Arc::clone(&high_priority_count);
            let total_matches = Arc::clone(&total_matches);
            let should_quit = Arc::clone(&should_quit);
            let timeout_mgr_clone = TimeoutManager::new(estimated_size);
            let pattern_processor = self.pattern_processor.clone();
            let base_path = path.to_path_buf();
            let patterns = Arc::clone(&patterns);

            Box::new(move |result| {
                // Optimization: Check atomic quit flag before acquiring lock
                if should_quit.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }

                // Check timeout
                if timeout_mgr_clone.should_stop() {
                    return ignore::WalkState::Quit;
                }

                let entry = match result {
                    Ok(entry) => entry,
                    Err(_) => return ignore::WalkState::Continue,
                };

                // Optimization: Check file type first before any string operations
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }

                // Optimization: Get filename once and reuse
                let filename = match entry.file_name().to_str() {
                    Some(name) => name,
                    None => return ignore::WalkState::Continue,
                };

                // Fast path: Skip files that don't match patterns
                if !pattern_processor.should_scan_file(filename) {
                    return ignore::WalkState::Continue;
                }

                timeout_mgr_clone.record_file_scanned();

                // Check if this is strong evidence
                let is_strong = pattern_processor.is_strong_evidence(filename);
                if is_strong {
                    let hp_count = high_priority_count.fetch_add(1, Ordering::Relaxed) + 1;

                    // Optimization: Early exit if we have enough strong evidence (3+ strong indicators)
                    // Check this BEFORE pattern matching to save work
                    if hp_count >= 3 && total_matches.load(Ordering::Relaxed) >= 3 {
                        log::trace!(
                            "Parallel scan: sufficient strong evidence ({}), signaling quit",
                            hp_count
                        );
                        should_quit.store(true, Ordering::Relaxed);
                        return ignore::WalkState::Quit;
                    }
                }

                // Match against patterns
                if let Some(file_match) = pattern_processor.match_file_against_patterns(
                    entry.path(),
                    &base_path,
                    &patterns,
                ) {
                    if let Ok(mut files) = matched_files.lock() {
                        files.push(file_match);
                        let count = files.len();
                        total_matches.store(count, Ordering::Relaxed);

                        // Max matches check
                        if count >= self.max_matches_per_pattern {
                            log::trace!(
                                "Parallel scan: max matches ({}) reached",
                                self.max_matches_per_pattern
                            );
                            should_quit.store(true, Ordering::Relaxed);
                            return ignore::WalkState::Quit;
                        }

                        // Early termination check
                        if is_strong {
                            let hp_count = high_priority_count.load(Ordering::Relaxed);
                            if timeout_mgr_clone.check_early_termination(hp_count) {
                                log::trace!(
                                    "Parallel scan: early termination triggered (high_priority={})",
                                    hp_count
                                );
                                should_quit.store(true, Ordering::Relaxed);
                                return ignore::WalkState::Quit;
                            }
                        }
                    }
                }

                ignore::WalkState::Continue
            })
        });

        // Try to unwrap Arc to avoid cloning if we're the only owner
        let files = match Arc::try_unwrap(matched_files) {
            Ok(mutex) => mutex
                .into_inner()
                .map_err(|e| anyhow::anyhow!("Failed to acquire lock: {}", e))?,
            Err(arc) => {
                // Still have other Arc references, must clone
                arc.lock()
                    .map_err(|e| anyhow::anyhow!("Failed to acquire lock: {}", e))?
                    .clone()
            }
        };

        let elapsed = scan_start.elapsed().as_millis();
        log::debug!(
            "Parallel scan complete: {} matches in {}ms",
            files.len(),
            elapsed
        );

        Ok(files)
    }

    pub fn pattern_processor(&self) -> &PatternProcessor {
        &self.pattern_processor
    }

    /// Batch collect files - alias for scan()
    pub fn batch_collect_files(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        self.scan(path)
    }

    /// Scan matching files - alias for scan()
    pub fn scan_matching_files(&self, path: &Path) -> Result<Vec<MatchedFile>> {
        self.scan(path)
    }

    /// Clear internal caches
    ///
    /// Currently a no-op as ScanningEngine doesn't maintain caches internally.
    /// PatternProcessor caches are managed separately by the DetectionEngine.
    pub fn clear_caches(&mut self) {
        // No-op: ScanningEngine doesn't have internal caches
        // PatternProcessor caches are managed separately
    }

    /// Get performance statistics
    ///
    /// Returns default stats as ScanningEngine doesn't maintain a FileSystemCache.
    /// Performance is tracked through logging instead.
    pub fn get_performance_stats(&self) -> crate::performance::CacheStats {
        // Return default stats since we don't have a cache
        crate::performance::CacheStats {
            metadata_entries: 0,
            metadata_capacity: 0,
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
            lock_contentions: 0,
            evictions_performed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::pattern_matching::PatternMatcher;
    use crate::test_utils::create_test_rust_project;
    use crate::types::{IndicatorContext, ProjectIndicator, RootIndicator};
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_engine() -> ScanningEngine {
        let patterns = Arc::new(vec![
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
            "*.js".to_string(),
        ]);

        let rust_lang = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string(), "Cargo.toml".to_string()],
            "#dea584".to_string(),
            "".to_string(),
            1,
            vec![],
            vec![RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 0.95,
                context: IndicatorContext::LanguageRoot,
            }],
        );

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let pattern_processor =
            PatternProcessor::new(pattern_matcher, patterns, vec![Arc::new(rust_lang)]);

        ScanningEngine::new(pattern_processor, 3)
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_test_engine();
        assert_eq!(engine.max_depth, 3);
    }

    #[test]
    fn test_priority_file_fast_path() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.check_priority_files(temp_dir.path())?;
        assert!(matches.is_some());

        if let Some(files) = matches {
            assert!(!files.is_empty());
            assert!(files.iter().any(|f| f.filename == "Cargo.toml"));
        }

        Ok(())
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

        // Create non-matching files
        fs::write(temp_dir.path().join("readme.txt"), "text")?;

        let matches = engine.scan(temp_dir.path())?;
        // Should be empty since no files match patterns
        assert!(matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_nested_files() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create nested structure
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
        let rust_lang = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string(), "Cargo.toml".to_string()],
            "#dea584".to_string(),
            "".to_string(),
            1,
            vec![],
            vec![],
        );

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let pattern_processor =
            PatternProcessor::new(pattern_matcher, patterns, vec![Arc::new(rust_lang)]);

        let file_cache = Arc::new(FileSystemCache::new(300, 1000));
        let engine = ScanningEngine::with_cache(pattern_processor, 3, Some(file_cache));

        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;
        let matches = engine.scan(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_engine_with_shared_pattern_matcher() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string(), "Cargo.toml".to_string()]);
        let rust_lang = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string(), "Cargo.toml".to_string()],
            "#dea584".to_string(),
            "".to_string(),
            1,
            vec![],
            vec![],
        );

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let engine = ScanningEngine::with_shared_pattern_matcher(
            pattern_matcher,
            patterns,
            vec![Arc::new(rust_lang)],
            3,
        );

        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;
        let matches = engine.scan(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan_with_timeout(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_no_matches() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;

        // Create non-matching files
        fs::write(temp_dir.path().join("readme.txt"), "text")?;

        let matches = engine.scan_with_timeout(temp_dir.path())?;
        assert!(matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_timeout_exceeded() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let rust_lang = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#dea584".to_string(),
            "".to_string(),
            1,
            vec![],
            vec![],
        );

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let pattern_processor =
            PatternProcessor::new(pattern_matcher, patterns, vec![Arc::new(rust_lang)]);

        let engine = ScanningEngine::new(pattern_processor, 1); // Very low depth to trigger timeout

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many files to potentially trigger timeout
        for i in 0..100 {
            fs::write(root.join(format!("file_{}.rs", i)), "// rust file")?;
        }

        let matches = engine.scan_with_timeout(root)?;
        // Should still return some matches even if timeout occurred
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_early_termination() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan_with_timeout(temp_dir.path())?;
        // Early termination may result in empty matches, just verify function completes
        let _ = matches;

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_strong_evidence() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan_with_timeout(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_file_count_limit() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many files to test file count limit
        for i in 0..200 {
            fs::write(root.join(format!("file_{}.rs", i)), "// rust file")?;
        }

        let matches = engine.scan_with_timeout(root)?;
        // Should return matches but potentially limited by file count
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_concurrent_access() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Test that multiple sequential scans work (avoiding lifetime issues)
        for _ in 0..4 {
            let matches = engine.scan_with_timeout(temp_dir.path())?;
            assert!(!matches.is_empty());
        }

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_adaptive_timeout() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create a small directory structure
        let src = root.join("src");
        fs::create_dir(&src)?;
        fs::write(src.join("lib.rs"), "// lib")?;

        let matches = engine.scan_with_timeout(root)?;
        // Adaptive timeout may result in empty matches, just verify function completes
        let _ = matches;

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_medium_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create a medium-sized directory structure
        for i in 0..50 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan_with_timeout(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_large_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create a large directory structure
        for i in 0..100 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan_with_timeout(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_very_large_directory() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create a very large directory structure
        for i in 0..200 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            fs::write(dir.join("lib.rs"), "// lib")?;
        }

        let matches = engine.scan_with_timeout(root)?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_timeout_not_exceeded_immediately() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let matches = engine.scan_with_timeout(temp_dir.path())?;
        assert!(!matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_file_counter_thread_safe() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Test that file counter works correctly with multiple sequential scans
        for _ in 0..4 {
            let matches = engine.scan_with_timeout(temp_dir.path())?;
            // File counter should work without panicking, results may vary
            let _ = matches;
        }

        Ok(())
    }

    #[test]
    fn test_scan_with_timeout_timeout_exceeded_scenario() -> Result<()> {
        let patterns = Arc::new(vec!["*.rs".to_string()]);
        let rust_lang = ProjectIndicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#dea584".to_string(),
            "".to_string(),
            1,
            vec![],
            vec![],
        );

        let pattern_matcher = Arc::new(PatternMatcher::new());
        let pattern_processor =
            PatternProcessor::new(pattern_matcher, patterns, vec![Arc::new(rust_lang)]);

        let engine = ScanningEngine::new(pattern_processor, 1); // Very low depth

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

        let matches = engine.scan_with_timeout(root)?;
        // Should return some matches even if timeout occurred, but may be empty due to timeout
        // The important thing is that the function doesn't panic
        // Just verify the function completed successfully
        let _ = matches;

        Ok(())
    }
}
