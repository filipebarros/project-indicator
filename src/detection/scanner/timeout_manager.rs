//! Scan timeout that scales with the size of the scan root: 100ms up to 50
//! entries, 300ms up to 200, 500ms above (roots over 500 entries are never
//! walked, see `EXTREME_SIZE_THRESHOLD`).

use std::time::{Duration, Instant};

pub struct TimeoutManager {
    start_time: Instant,
    timeout_duration: Duration,
}

impl TimeoutManager {
    pub fn new(estimated_file_count: usize) -> Self {
        let timeout_duration = Self::calculate_adaptive_timeout(estimated_file_count);

        Self {
            start_time: Instant::now(),
            timeout_duration,
        }
    }

    /// Fixed timeout, for tests that need to force an expiry
    #[cfg(test)]
    pub fn with_timeout(timeout_duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            timeout_duration,
        }
    }

    fn calculate_adaptive_timeout(file_count: usize) -> Duration {
        match file_count {
            0..=50 => Duration::from_millis(100),
            51..=200 => Duration::from_millis(300),
            _ => Duration::from_millis(500),
        }
    }

    pub fn should_stop(&self) -> bool {
        self.start_time.elapsed() >= self.timeout_duration
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
}
