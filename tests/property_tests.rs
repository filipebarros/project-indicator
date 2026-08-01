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
    Store(PathBuf),
    HasChanged(PathBuf),
}

/// Strategy for generating cache operations
fn cache_operation_strategy() -> impl Strategy<Value = CacheOp> {
    prop_oneof![
        path_strategy().prop_map(CacheOp::Get),
        path_strategy().prop_map(CacheOp::Store),
        path_strategy().prop_map(CacheOp::HasChanged),
    ]
}

// Property Tests

proptest! {
    /// Invariant: FileSystemCache holds at most one entry per distinct path
    #[test]
    fn test_cache_bounded_by_distinct_paths(
        operations in prop::collection::vec(cache_operation_strategy(), 1..500)
    ) {
        let cache = FileSystemCache::new();
        let mut distinct_paths = std::collections::HashSet::new();

        for op in operations {
            match op {
                CacheOp::Store(path) | CacheOp::Get(path) => {
                    let _ = cache.get_metadata(&path);
                    distinct_paths.insert(path);
                },
                CacheOp::HasChanged(path) => {
                    let _ = cache.exists(&path);
                    distinct_paths.insert(path);
                },
            }
        }

        let stats = cache.stats();
        prop_assert!(
            stats.metadata_entries <= distinct_paths.len(),
            "Cache size {} exceeds distinct paths touched {}",
            stats.metadata_entries,
            distinct_paths.len()
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
        let cache = FileSystemCache::new();

        for op in operations {
            if let CacheOp::Store(path) = op {
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

    }

    /// Invariant: Pattern cache bookkeeping is consistent
    #[test]
    fn test_pattern_cache_bookkeeping_consistency(
        operations in prop::collection::vec((filename_strategy(), pattern_strategy()), 1..100)
    ) {
        let matcher = PatternMatcher::new();
        let mut total_lookups = 0usize;

        for (filename, pattern) in operations {
            let (entries_before, _) = matcher.cache_stats();

            matcher.matches_pattern(&filename, &pattern);
            total_lookups += 1;

            let (entries_after, hit_rate) = matcher.cache_stats();

            // Entry count should only increase or stay the same (cache hits)
            prop_assert!(
                entries_after >= entries_before,
                "Entry count decreased: {} -> {}",
                entries_before,
                entries_after
            );

            // Hit rate should be between 0 and 100
            prop_assert!(
                (0.0..=100.0).contains(&hit_rate),
                "Hit rate {} out of valid range [0, 100]",
                hit_rate
            );
        }

        let (final_entries, final_hit_rate) = matcher.cache_stats();
        let (hits, misses) = matcher.hit_miss_counts();

        // Every lookup was either a hit or a miss; entries equal misses
        // because each miss memoizes exactly one new pair
        prop_assert_eq!(hits + misses, total_lookups);
        prop_assert_eq!(final_entries, misses);
        prop_assert!((0.0..=100.0).contains(&final_hit_rate));
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
        let cache = Arc::new(FileSystemCache::new());
        let total_ops: usize = operations.iter().map(|thread_ops| thread_ops.len()).sum();
        let mut handles = vec![];

        // Spawn thread for each operation set
        for thread_ops in operations {
            let cache_clone: Arc<FileSystemCache> = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for op in thread_ops {
                    match op {
                        CacheOp::Store(path) | CacheOp::Get(path) => {
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
            stats.metadata_entries <= total_ops,
            "Cache size {} exceeds total operations {} after concurrent access",
            stats.metadata_entries,
            total_ops
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
