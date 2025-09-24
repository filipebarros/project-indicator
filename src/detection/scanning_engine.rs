use crate::detection::pattern_matching::PatternMatcher;
use crate::detection::pattern_processor::PatternProcessor;
use crate::performance::FileSystemCache;
use crate::types::{MatchedFile, ProjectIndicator};
use crate::Result;
use ignore::WalkBuilder;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
    fs_cache: FileSystemCache,
    unique_patterns: Vec<String>,
    languages: Vec<Arc<ProjectIndicator>>,
    max_depth: usize,
}

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub max_depth: usize,
    pub timeout: Duration,
    pub enable_parallel: bool,
    pub early_termination_threshold: usize,
    pub max_matches: Option<usize>,
    pub batch_size: usize,
    pub enable_memory_efficient_mode: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            timeout: Duration::from_millis(500),
            enable_parallel: true,
            early_termination_threshold: 2,
            max_matches: Some(100),
            batch_size: 50,
            enable_memory_efficient_mode: false,
        }
    }
}

impl ScanningEngine {
    pub fn new(
        pattern_matcher: Arc<PatternMatcher>,
        patterns: Vec<String>,
        languages: Vec<Arc<ProjectIndicator>>,
        max_depth: usize,
    ) -> Self {
        let pattern_processor =
            PatternProcessor::new(pattern_matcher, patterns.clone(), languages.clone());

        Self {
            pattern_processor,
            fs_cache: FileSystemCache::default(),
            unique_patterns: patterns,
            languages,
            max_depth,
        }
    }

    pub fn scan_files(&self, path: &Path, config: &ScanConfig) -> Result<Vec<MatchedFile>> {
        if let Some(priority_matches) = self.scan_priority_files(path)? {
            return Ok(priority_matches);
        }

        let estimated_size = self.estimate_directory_size(path);

        if estimated_size < 50 && !config.enable_parallel {
            return self.scan_sequential(path, config);
        }

        self.scan_with_optimizations(path, config)
    }

    fn scan_sequential(&self, path: &Path, config: &ScanConfig) -> Result<Vec<MatchedFile>> {
        let mut matched_files = Vec::new();
        let mut strong_evidence_count = 0;

        let walker = WalkBuilder::new(path)
            .max_depth(Some(self.max_depth))
            .follow_links(false)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .hidden(false)
            .parents(true)
            .standard_filters(true)
            .build();

        for result in walker {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                if let Some(filename) = entry.file_name().to_str() {
                    if self.pattern_processor.should_scan_file(filename) {
                        if self.pattern_processor.is_strong_evidence(filename) {
                            strong_evidence_count += 1;
                            if strong_evidence_count >= config.early_termination_threshold {
                                break;
                            }
                        }

                        if let Some(file_match) = self
                            .pattern_processor
                            .match_file_against_patterns(entry.path(), path, &self.unique_patterns)
                        {
                            matched_files.push(file_match);

                            if matched_files.len() >= 10 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(matched_files)
    }

    fn scan_priority_files(&self, path: &Path) -> Result<Option<Vec<MatchedFile>>> {
        let mut matches = Vec::new();

        for filename in self.pattern_processor.get_high_priority_files() {
            let file_path = path.join(filename);
            if self.fs_cache.exists(&file_path) {
                if let Ok(relative_path) = file_path.strip_prefix(path) {
                    if let Some(relative_str) = relative_path.to_str() {
                        let matched_file =
                            MatchedFile::new(filename.clone(), relative_str.to_string());
                        matches.push(matched_file);
                    }
                }
            }
        }

        if !matches.is_empty() {
            return Ok(Some(matches));
        }

        Ok(None)
    }

    fn scan_with_optimizations(
        &self,
        path: &Path,
        config: &ScanConfig,
    ) -> Result<Vec<MatchedFile>> {
        let start_time = Instant::now();
        let should_stop = Arc::new(AtomicBool::new(false));
        let matched_files = Arc::new(Mutex::new(Vec::new()));
        let strong_evidence_count = Arc::new(AtomicUsize::new(0));

        let estimated_size = self.estimate_directory_size(path);
        let adaptive_timeout = self.calculate_adaptive_timeout(estimated_size, config.timeout);
        let max_scan_time = adaptive_timeout;

        WalkBuilder::new(path)
            .max_depth(Some(self.max_depth))
            .follow_links(false)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .hidden(false)
            .parents(true)
            .standard_filters(true)
            .build_parallel()
            .run(|| {
                let should_stop = should_stop.clone();
                let matched_files = matched_files.clone();
                let base_path = path.to_path_buf();
                let patterns = self.unique_patterns.clone();
                let strong_evidence_count = strong_evidence_count.clone();
                let engine = self.clone();

                Box::new(move |result| {
                    if start_time.elapsed() > max_scan_time {
                        return ignore::WalkState::Quit;
                    }

                    if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }

                    let entry = match result {
                        Ok(entry) => entry,
                        Err(_) => return ignore::WalkState::Continue,
                    };

                    if entry.file_type().is_some_and(|ft| ft.is_file()) {
                        if let Some(filename) = entry.file_name().to_str() {
                            if engine.pattern_processor.should_scan_file(filename) {
                                if engine.pattern_processor.is_strong_evidence(filename) {
                                    let current_count = strong_evidence_count
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    if current_count >= config.early_termination_threshold {
                                        should_stop
                                            .store(true, std::sync::atomic::Ordering::Relaxed);
                                        return ignore::WalkState::Quit;
                                    }
                                }

                                if let Some(file_match) =
                                    engine.pattern_processor.match_file_against_patterns(
                                        entry.path(),
                                        &base_path,
                                        &patterns,
                                    )
                                {
                                    engine.collect_match_and_check_termination(
                                        file_match,
                                        &matched_files,
                                        &should_stop,
                                        config,
                                    );
                                }
                            }
                        }
                    }

                    if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
                        ignore::WalkState::Quit
                    } else {
                        ignore::WalkState::Continue
                    }
                })
            });

        let matched_files = matched_files
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire lock on matched files"))?;
        Ok(matched_files.iter().cloned().collect())
    }

    fn collect_match_and_check_termination(
        &self,
        file_match: MatchedFile,
        matched_files: &Arc<Mutex<Vec<MatchedFile>>>,
        should_stop: &Arc<AtomicBool>,
        config: &ScanConfig,
    ) {
        if let Ok(mut files) = matched_files.lock() {
            files.push(file_match);

            if files.len() >= 10 {
                should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }

            if let Some(max_matches) = config.max_matches {
                if files.len() >= max_matches {
                    should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    }

    fn estimate_directory_size(&self, path: &Path) -> usize {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.take(100).flatten() {
                if entry.file_type().is_ok_and(|ft| ft.is_file()) {
                    count += 1;
                } else if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                    count += 10;
                }
            }
        }

        if count >= 100 {
            1000
        } else {
            count.max(1)
        }
    }

    fn calculate_adaptive_timeout(
        &self,
        estimated_size: usize,
        base_timeout: Duration,
    ) -> Duration {
        let multiplier = match estimated_size {
            0..=50 => 1.0,
            51..=200 => 2.0,
            _ => 3.0,
        };

        Duration::from_millis((base_timeout.as_millis() as f64 * multiplier) as u64)
    }

    pub fn get_pattern_importance(&self, pattern: &str) -> f32 {
        self.pattern_processor
            .get_pattern_importance(pattern, &self.languages)
    }

    pub fn clear_caches(&mut self) {
        self.fs_cache.clear();
    }

    pub fn get_performance_stats(&self) -> crate::performance::CacheStats {
        self.fs_cache.stats()
    }
}
