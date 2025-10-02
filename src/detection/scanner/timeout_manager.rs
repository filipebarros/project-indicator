//! Timeout and early termination management
//!
//! This module provides adaptive timeout calculation and early termination
//! heuristics for efficient file scanning.
//!
//! # Adaptive Timeouts
//!
//! Timeouts scale with directory size:
//! - 0-50 files: 100ms
//! - 51-200 files: 300ms
//! - 201-1000 files: 500ms
//! - 1000+ files: 1000ms
//!
//! # Early Termination
//!
//! Scanning stops early when:
//! - 3+ high-priority files found (strong evidence)
//! - 15+ total files scanned (sufficient sample)
//!
//! All operations are thread-safe using atomic types.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct TimeoutManager {
    start_time: Instant,
    timeout_duration: Duration,
    files_scanned: Arc<AtomicUsize>,
    should_terminate: Arc<AtomicBool>,
}

impl TimeoutManager {
    pub fn new(estimated_file_count: usize) -> Self {
        let timeout_duration = Self::calculate_adaptive_timeout(estimated_file_count);

        Self {
            start_time: Instant::now(),
            timeout_duration,
            files_scanned: Arc::new(AtomicUsize::new(0)),
            should_terminate: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Calculate timeout based on estimated directory size
    fn calculate_adaptive_timeout(file_count: usize) -> Duration {
        match file_count {
            0..=50 => Duration::from_millis(100),
            51..=200 => Duration::from_millis(300),
            201..=1000 => Duration::from_millis(500),
            _ => Duration::from_millis(1000),
        }
    }

    /// Check if scanning should stop (timeout or early termination)
    pub fn should_stop(&self) -> bool {
        self.should_terminate.load(Ordering::Relaxed)
            || self.start_time.elapsed() >= self.timeout_duration
    }

    /// Record that a file was scanned
    pub fn record_file_scanned(&self) {
        self.files_scanned.fetch_add(1, Ordering::Relaxed);
    }

    /// Check early termination heuristics and set termination flag if met
    /// Returns true if early termination was triggered
    pub fn check_early_termination(&self, high_priority_count: usize) -> bool {
        let scanned = self.files_scanned.load(Ordering::Relaxed);

        // Strong evidence threshold: 3+ high-priority files
        if high_priority_count >= 3 {
            log::trace!(
                "Early termination: strong evidence (high_priority={}, threshold=3)",
                high_priority_count
            );
            self.should_terminate.store(true, Ordering::Relaxed);
            return true;
        }

        // File count threshold: 15+ files scanned
        if scanned >= 15 {
            log::trace!(
                "Early termination: file count limit (scanned={}, threshold=15)",
                scanned
            );
            self.should_terminate.store(true, Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn files_scanned(&self) -> usize {
        self.files_scanned.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_adaptive_timeout_small_dir() {
        let mgr = TimeoutManager::new(25);
        assert_eq!(mgr.timeout_duration, Duration::from_millis(100));
    }

    #[test]
    fn test_adaptive_timeout_medium_dir() {
        let mgr = TimeoutManager::new(100);
        assert_eq!(mgr.timeout_duration, Duration::from_millis(300));
    }

    #[test]
    fn test_adaptive_timeout_large_dir() {
        let mgr = TimeoutManager::new(500);
        assert_eq!(mgr.timeout_duration, Duration::from_millis(500));
    }

    #[test]
    fn test_adaptive_timeout_very_large_dir() {
        let mgr = TimeoutManager::new(2000);
        assert_eq!(mgr.timeout_duration, Duration::from_millis(1000));
    }

    #[test]
    fn test_early_termination_strong_evidence() {
        let mgr = TimeoutManager::new(100);
        assert!(!mgr.should_stop());

        let terminated = mgr.check_early_termination(3);
        assert!(terminated);
        assert!(mgr.should_stop());
    }

    #[test]
    fn test_early_termination_file_count() {
        let mgr = TimeoutManager::new(100);

        for _ in 0..15 {
            mgr.record_file_scanned();
        }

        let terminated = mgr.check_early_termination(0);
        assert!(terminated);
        assert!(mgr.should_stop());
    }

    #[test]
    fn test_timeout_not_exceeded_immediately() {
        let mgr = TimeoutManager::new(100);
        assert!(!mgr.should_stop());
    }

    #[test]
    fn test_timeout_exceeded() {
        let mgr = TimeoutManager::new(0); // 100ms timeout
        thread::sleep(Duration::from_millis(150));
        assert!(mgr.should_stop());
    }

    #[test]
    fn test_file_counter_thread_safe() {
        let mgr = Arc::new(TimeoutManager::new(100));
        let mut handles = vec![];

        for _ in 0..10 {
            let mgr_clone = Arc::clone(&mgr);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    mgr_clone.record_file_scanned();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.join() {
                panic!("Thread panicked: {:?}", e);
            }
        }

        assert_eq!(mgr.files_scanned(), 100);
    }
}
