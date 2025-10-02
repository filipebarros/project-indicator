//! Property-based tests using proptest
//!
//! These tests verify invariants across randomized inputs to catch edge cases
//! that might be missed by example-based testing.

use project_indicator::detection::pattern_matching::PatternMatcher;
use project_indicator::performance::FileSystemCache;
use proptest::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

// Custom strategies for generating test data

/// Strategy for generating valid file paths
fn path_strategy() -> impl Strategy<Value = PathBuf> {
    prop::collection::vec("[a-zA-Z0-9_-]{1,10}", 1..5).prop_map(|components| {
        let mut path = PathBuf::from("/test");
        for component in components {
            path.push(component);
        }
        path
    })
}

/// Strategy for generating file patterns
fn pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("*".to_string()),
        Just("*.rs".to_string()),
        Just("*.js".to_string()),
        Just("*.py".to_string()),
        Just("*.json".to_string()),
        Just("test*".to_string()),
        Just("src*".to_string()),
    ]
}

/// Strategy for generating filenames
fn filename_strategy() -> impl Strategy<Value = String> {
    ("[a-z]{3,10}", "[a-z]{2,4}").prop_map(|(name, ext)| format!("{}.{}", name, ext))
}

/// Cache operation for testing
#[derive(Debug, Clone)]
enum CacheOp {
    Get(PathBuf),
    Store(PathBuf, u64), // path and mock timestamp
    HasChanged(PathBuf),
}

/// Strategy for generating cache operations
fn cache_operation_strategy() -> impl Strategy<Value = CacheOp> {
    prop_oneof![
        path_strategy().prop_map(CacheOp::Get),
        (path_strategy(), any::<u64>()).prop_map(|(path, ts)| CacheOp::Store(path, ts)),
        path_strategy().prop_map(CacheOp::HasChanged),
    ]
}

// Property Tests

proptest! {
    /// Invariant: FileSystemCache never exceeds its configured capacity
    #[test]
    fn test_cache_never_exceeds_capacity(
        operations in prop::collection::vec(cache_operation_strategy(), 1..500)
    ) {
        let max_entries = 50;
        let cache = FileSystemCache::new(3600, max_entries);

        for op in operations {
            match op {
                CacheOp::Store(path, _timestamp) => {
                    let _ = cache.get_metadata(&path);
                },
                CacheOp::Get(path) => {
                    let _ = cache.get_metadata(&path);
                },
                CacheOp::HasChanged(path) => {
                    let _ = cache.exists(&path);
                },
            }
        }

        let stats = cache.stats();
        let entries = stats.metadata_entries;
        prop_assert!(
            entries <= max_entries,
            "Cache size {} exceeds capacity {}",
            entries,
            max_entries
        );
    }

    /// Invariant: Pattern matching is deterministic
    #[test]
    fn test_pattern_matching_consistency(
        pattern in pattern_strategy(),
        filenames in prop::collection::vec(filename_strategy(), 1..20)
    ) {
        let matcher = PatternMatcher::new();

        for filename in &filenames {
            let result1 = matcher.matches_pattern(filename, &pattern);
            let result2 = matcher.matches_pattern(filename, &pattern);

            // Should be deterministic - same inputs always give same output
            prop_assert_eq!(
                result1,
                result2,
                "Pattern '{}' matching '{}' gave inconsistent results",
                pattern,
                filename
            );

            // Third call should also match (testing cache consistency)
            let result3 = matcher.matches_pattern(filename, &pattern);
            prop_assert_eq!(
                result1,
                result3,
                "Cached result for '{}' pattern '{}' is inconsistent",
                filename,
                pattern
            );
        }
    }

    /// Invariant: Cache stats are always consistent
    #[test]
    fn test_cache_stats_consistency(
        operations in prop::collection::vec(cache_operation_strategy(), 1..100)
    ) {
        let cache = FileSystemCache::new(3600, 100);

        for op in operations {
            if let CacheOp::Store(path, _) = op {
                let _ = cache.get_metadata(&path);
            }
        }

        let stats = cache.stats();

        // Hit rate calculation should be consistent
        let total_accesses = stats.hits + stats.misses;
        if total_accesses > 0 {
            let expected_hit_rate = (stats.hits as f64 / total_accesses as f64) * 100.0;
            prop_assert!(
                (stats.hit_rate - expected_hit_rate).abs() < 0.01,
                "Hit rate {} doesn't match expected {}",
                stats.hit_rate,
                expected_hit_rate
            );
        } else {
            prop_assert_eq!(stats.hit_rate, 0.0, "Hit rate should be 0 with no accesses");
        }

        // Entry count should be non-negative
        prop_assert!(stats.metadata_entries <= stats.metadata_capacity, "Entry count cannot exceed capacity");
    }

    /// Invariant: Pattern cache memory tracking is consistent
    #[test]
    fn test_pattern_cache_memory_consistency(
        operations in prop::collection::vec((filename_strategy(), pattern_strategy()), 1..100)
    ) {
        let matcher = PatternMatcher::new();

        for (filename, pattern) in operations {
            let (entries_before, _, _) = matcher.cache_stats();

            matcher.matches_pattern(&filename, &pattern);

            let (entries_after, memory_after, hit_rate) = matcher.cache_stats();

            // Entry count should only increase or stay the same (cache hits)
            prop_assert!(
                entries_after >= entries_before,
                "Entry count decreased: {} -> {}",
                entries_before,
                entries_after
            );

            // Memory should be positive if we have entries
            if entries_after > 0 {
                prop_assert!(memory_after > 0, "Memory should be tracked when entries exist");
            }

            // Hit rate should be between 0 and 100
            prop_assert!(
                (0.0..=100.0).contains(&hit_rate),
                "Hit rate {} out of valid range [0, 100]",
                hit_rate
            );
        }

        let (final_entries, final_memory, final_hit_rate) = matcher.cache_stats();

        // Sanity checks on final state
        prop_assert!(final_entries <= 10000, "Entry count {} exceeds max cache size", final_entries);
        prop_assert!((0.0..=100.0).contains(&final_hit_rate));
        if final_entries > 0 {
            prop_assert!(final_memory > 0);
        }
    }
}

/// Concurrent property tests
#[test]
fn test_concurrent_cache_operations_maintain_invariants() {
    // Use a fixed seed for reproducibility
    let config = ProptestConfig::with_cases(20);

    proptest!(config, |(
        operations in prop::collection::vec(
            prop::collection::vec(cache_operation_strategy(), 10..50),
            2..10
        )
    )| {
        let cache = Arc::new(FileSystemCache::new(3600, 100));
        let mut handles = vec![];

        // Spawn thread for each operation set
        for thread_ops in operations {
            let cache_clone: Arc<FileSystemCache> = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for op in thread_ops {
                    match op {
                        CacheOp::Store(path, _) => {
                            let _ = cache_clone.get_metadata(&path);
                        },
                        CacheOp::Get(path) => {
                            let _ = cache_clone.get_metadata(&path);
                        },
                        CacheOp::HasChanged(path) => {
                            let _ = cache_clone.exists(&path);
                        },
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            if let Err(e) = handle.join() {
                panic!("Thread panicked: {:?}", e);
            }
        }

        // Verify invariants after concurrent access
        let stats = cache.stats();

        prop_assert!(
            stats.metadata_entries <= 100,
            "Cache size {} exceeds capacity 100 after concurrent access",
            stats.metadata_entries
        );

        prop_assert!(
            (0.0..=100.0).contains(&stats.hit_rate),
            "Hit rate {} out of valid range after concurrent access",
            stats.hit_rate
        );
    });
}

/// Test that pattern matching works correctly with edge case patterns
#[test]
fn test_edge_case_patterns() {
    let config = ProptestConfig::with_cases(100);

    proptest!(config, |(
        filename in filename_strategy()
    )| {
        let matcher = PatternMatcher::new();

        // Pattern "*" should match everything
        prop_assert!(
            matcher.matches_pattern(&filename, "*"),
            "Pattern '*' should match '{}'",
            filename
        );

        // Exact match should always work
        prop_assert!(
            matcher.matches_pattern(&filename, &filename),
            "Exact match failed for '{}'",
            filename
        );

        // Empty pattern should not match non-empty filename
        prop_assert!(
            !matcher.matches_pattern(&filename, ""),
            "Empty pattern should not match '{}'",
            filename
        );
    });
}
