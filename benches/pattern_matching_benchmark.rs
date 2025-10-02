use criterion::{criterion_group, criterion_main, Criterion};
use project_indicator::detection::pattern_matching::PatternMatcher;
use std::hint::black_box;
use std::sync::Arc;

/// Benchmark pattern matching with common patterns
fn benchmark_common_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");
    let matcher = Arc::new(PatternMatcher::new());

    // Common extension patterns
    let extensions = vec![
        ("*.rs", "src/main.rs", true),
        ("*.js", "index.js", true),
        ("*.ts", "app.ts", true),
        ("*.py", "script.py", true),
        ("*.go", "main.go", true),
        ("*.rs", "test.js", false),
    ];

    group.bench_function("extension_matching_cold", |b| {
        b.iter(|| {
            let matcher = PatternMatcher::new();
            for (pattern, filename, _) in &extensions {
                black_box(matcher.matches_pattern(filename, pattern));
            }
        });
    });

    group.bench_function("extension_matching_warm", |b| {
        // Pre-warm cache
        for (pattern, filename, _) in &extensions {
            matcher.matches_pattern(filename, pattern);
        }

        b.iter(|| {
            for (pattern, filename, _) in &extensions {
                black_box(matcher.matches_pattern(filename, pattern));
            }
        });
    });

    // Exact match patterns
    group.bench_function("exact_match", |b| {
        b.iter(|| {
            black_box(matcher.matches_pattern("Cargo.toml", "Cargo.toml"));
            black_box(matcher.matches_pattern("package.json", "package.json"));
            black_box(matcher.matches_pattern("go.mod", "go.mod"));
        });
    });

    // Prefix patterns
    group.bench_function("prefix_match", |b| {
        b.iter(|| {
            black_box(matcher.matches_pattern("test_file.rs", "test_*"));
            black_box(matcher.matches_pattern("main_app.js", "main_*"));
            black_box(matcher.matches_pattern("config_dev.json", "config_*"));
        });
    });

    group.finish();
}

/// Benchmark cache effectiveness
fn benchmark_cache_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_effectiveness");

    // Simulate realistic workload: many files against few patterns
    let filenames: Vec<String> = (0..100)
        .map(|i| format!("src/module_{}/file_{}.rs", i / 10, i))
        .collect();

    let patterns = vec!["*.rs", "*.toml", "*.json", "Cargo.*"];

    group.bench_function("realistic_workload_cold", |b| {
        b.iter(|| {
            let matcher = PatternMatcher::new();
            for filename in &filenames {
                for pattern in &patterns {
                    black_box(matcher.matches_pattern(filename, pattern));
                }
            }
        });
    });

    let matcher = Arc::new(PatternMatcher::new());
    group.bench_function("realistic_workload_warm", |b| {
        // Pre-warm cache
        for filename in &filenames {
            for pattern in &patterns {
                matcher.matches_pattern(filename, pattern);
            }
        }

        b.iter(|| {
            for filename in &filenames {
                for pattern in &patterns {
                    black_box(matcher.matches_pattern(filename, pattern));
                }
            }
        });
    });

    group.finish();
}

/// Benchmark cache eviction performance
fn benchmark_cache_eviction(c: &mut Criterion) {
    c.bench_function("cache_eviction", |b| {
        b.iter(|| {
            let matcher = PatternMatcher::new();

            // Fill cache beyond capacity (10k entries)
            for i in 0..12000 {
                let filename = format!("file_{}.txt", i);
                black_box(matcher.matches_pattern(&filename, "*.txt"));
            }

            // Verify cache was evicted
            let (cache_entries, _, _) = matcher.cache_stats();
            assert!(cache_entries <= 10000);
        });
    });
}

/// Benchmark different pattern complexity
fn benchmark_pattern_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_complexity");
    let matcher = Arc::new(PatternMatcher::new());

    // Simple patterns (optimized fast path)
    group.bench_function("simple_extension", |b| {
        b.iter(|| {
            for i in 0..100 {
                let filename = format!("file_{}.rs", i);
                black_box(matcher.matches_pattern(&filename, "*.rs"));
            }
        });
    });

    // Exact match (fastest)
    group.bench_function("exact_match", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(matcher.matches_pattern("Cargo.toml", "Cargo.toml"));
            }
        });
    });

    // Complex patterns (slower wildcard matching)
    group.bench_function("complex_pattern", |b| {
        b.iter(|| {
            for i in 0..100 {
                let filename = format!("src/test_{}.rs", i);
                black_box(matcher.matches_pattern(&filename, "*/test_*.rs"));
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent access
fn benchmark_concurrent_access(c: &mut Criterion) {
    use std::thread;

    c.bench_function("concurrent_pattern_matching", |b| {
        let matcher = Arc::new(PatternMatcher::new());

        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|thread_id| {
                    let matcher = Arc::clone(&matcher);
                    thread::spawn(move || {
                        for i in 0..50 {
                            let filename = format!("thread_{}_file_{}.rs", thread_id, i);
                            matcher.matches_pattern(&filename, "*.rs");
                        }
                    })
                })
                .collect();

            for handle in handles {
                if let Err(e) = handle.join() {
                    eprintln!("Thread panicked: {:?}", e);
                }
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_common_patterns,
    benchmark_cache_effectiveness,
    benchmark_cache_eviction,
    benchmark_pattern_complexity,
    benchmark_concurrent_access
);
criterion_main!(benches);
