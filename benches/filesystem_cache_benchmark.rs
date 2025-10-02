use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::performance::FileSystemCache;
use std::hint::black_box;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a temp directory with multiple files for testing
fn create_test_files(count: usize) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    for i in 0..count {
        let file_path = temp_dir.path().join(format!("file_{}.txt", i));
        std::fs::write(&file_path, format!("content {}", i))
            .map_err(|e| format!("Failed to write test file: {}", e))?;
    }

    Ok(temp_dir)
}

/// Benchmark cache hit performance (warm cache)
fn benchmark_cache_hits(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_hits");
    let cache = FileSystemCache::default();
    let temp_dir = create_test_files(100)?;

    // Warm up cache with all files
    for i in 0..100 {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        cache.get_metadata(&path);
    }

    group.bench_function("sequential_100_files", |b| {
        b.iter(|| {
            for i in 0..100 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.get_metadata(black_box(&path)));
            }
        });
    });

    group.finish();
    Ok(())
}

/// Benchmark cache miss performance (cold cache)
fn benchmark_cache_misses(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_misses");
    let temp_dir = create_test_files(100)?;

    group.bench_function("sequential_100_files", |b| {
        b.iter(|| {
            let cache = FileSystemCache::default(); // Fresh cache each time
            for i in 0..100 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.get_metadata(black_box(&path)));
            }
        });
    });

    group.finish();
    Ok(())
}

/// Benchmark cache performance with different file counts
fn benchmark_cache_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_scaling");

    for count in [10, 50, 100, 500].iter() {
        let temp_dir = create_test_files(*count)?;
        let cache = FileSystemCache::default();

        // Warm up cache
        for i in 0..*count {
            let path = temp_dir.path().join(format!("file_{}.txt", i));
            cache.get_metadata(&path);
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                for i in 0..count {
                    let path = temp_dir.path().join(format!("file_{}.txt", i));
                    black_box(cache.get_metadata(black_box(&path)));
                }
            });
        });
    }

    group.finish();
    Ok(())
}

/// Benchmark cache eviction behavior
fn benchmark_cache_eviction(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_files(2000)?; // More files than cache capacity
    let cache = FileSystemCache::default();

    c.bench_function("cache_eviction_2000_files", |b| {
        b.iter(|| {
            // Access files to trigger eviction
            for i in 0..2000 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.get_metadata(black_box(&path)));
            }
        });
    });

    Ok(())
}

/// Benchmark concurrent cache access
fn benchmark_concurrent_access(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    use std::thread;

    let temp_dir = create_test_files(100)?;
    let cache = Arc::new(FileSystemCache::default());

    // Warm up cache
    for i in 0..100 {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        cache.get_metadata(&path);
    }

    c.bench_function("concurrent_4_threads_100_files", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..4 {
                let cache = Arc::clone(&cache);
                let temp_path = temp_dir.path().to_path_buf();

                let handle = thread::spawn(move || {
                    for i in 0..25 {
                        let path = temp_path.join(format!("file_{}.txt", i));
                        black_box(cache.get_metadata(black_box(&path)));
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                if let Err(e) = handle.join() {
                    eprintln!("Thread panicked: {:?}", e);
                }
            }
        });
    });

    Ok(())
}

/// Benchmark is_file performance
fn benchmark_is_file_checks(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_files(100)?;
    let cache = FileSystemCache::default();

    // Warm up cache
    for i in 0..100 {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        cache.is_file(&path);
    }

    c.bench_function("is_file_100_checks", |b| {
        b.iter(|| {
            for i in 0..100 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.is_file(black_box(&path)));
            }
        });
    });

    Ok(())
}

/// Benchmark cache stats collection
fn benchmark_cache_stats(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_files(100)?;
    let cache = FileSystemCache::default();

    // Populate cache
    for i in 0..100 {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        cache.get_metadata(&path);
    }

    c.bench_function("cache_stats_collection", |b| {
        b.iter(|| {
            black_box(cache.stats());
        });
    });

    Ok(())
}

// Wrapper functions for criterion
fn benchmark_cache_hits_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_hits(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_cache_misses_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_misses(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_cache_scaling_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_scaling(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_cache_eviction_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_eviction(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_concurrent_access_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_concurrent_access(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_is_file_checks_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_is_file_checks(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_cache_stats_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_stats(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

criterion_group!(
    benches,
    benchmark_cache_hits_wrapper,
    benchmark_cache_misses_wrapper,
    benchmark_cache_scaling_wrapper,
    benchmark_cache_eviction_wrapper,
    benchmark_concurrent_access_wrapper,
    benchmark_is_file_checks_wrapper,
    benchmark_cache_stats_wrapper
);
criterion_main!(benches);
