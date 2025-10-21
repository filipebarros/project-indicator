use criterion::{criterion_group, criterion_main, Criterion};
use project_indicator::tracking::{DetectionSnapshot, PathCache};
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn create_test_snapshot() -> DetectionSnapshot {
    use project_indicator::tracking::{
        CacheInfo, DetectionSnapshot, EvidenceSnapshot, FrameworkResult, LanguageResult,
    };

    DetectionSnapshot {
        snapshot_id: DetectionSnapshot::new_id(),
        timestamp: 1234567890,
        path: "/test/project/path".to_string(),
        path_hash: DetectionSnapshot::hash_path("/test/project/path"),
        language: Some(LanguageResult {
            name: Arc::from("Rust"),
            confidence: 0.95,
            sample_files: vec![
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
                "Cargo.toml".to_string(),
            ],
        }),
        frameworks: vec![
            FrameworkResult {
                name: Arc::from("Tokio"),
                confidence: 0.9,
                detected_via: vec!["Cargo.toml".to_string()],
            },
            FrameworkResult {
                name: Arc::from("Serde"),
                confidence: 0.85,
                detected_via: vec!["Cargo.toml".to_string()],
            },
        ],
        confidence: 0.95,
        evidence: EvidenceSnapshot {
            sample_matched_files: vec![],
            root_indicators: vec!["Cargo.toml".to_string(), ".git".to_string()],
            early_termination: None,
            total_files_matched: 42,
        },
        cache_info: CacheInfo {
            detection_from_cache: false,
            cached_at: None,
            pattern_cache_hits: 150,
            pattern_cache_misses: 20,
            fs_cache_hits: 80,
            fs_cache_misses: 15,
        },
        duration_micros: 3500,
        files_scanned: 42,
    }
}

fn bench_path_canonicalization_with_cache(c: &mut Criterion) {
    let cache = PathCache::new();
    let path = PathBuf::from(".");

    c.bench_function("path_canonicalization_with_cache_hit", |b| {
        // Prime the cache
        cache.get_canonical(&path);

        b.iter(|| {
            let result = cache.get_canonical(black_box(&path));
            black_box(result);
        })
    });

    c.bench_function("path_canonicalization_cache_miss", |b| {
        let cache = PathCache::new();
        let path = PathBuf::from("./src/main.rs");

        b.iter(|| {
            cache.clear(); // Ensure cache miss
            let result = cache.get_canonical(black_box(&path));
            black_box(result);
        })
    });
}

fn bench_path_canonicalization_without_cache(c: &mut Criterion) {
    let path = PathBuf::from(".");

    c.bench_function("path_canonicalization_direct", |b| {
        b.iter(|| {
            let result = black_box(&path)
                .canonicalize()
                .unwrap_or_else(|_| path.clone())
                .to_string_lossy()
                .to_string();
            black_box(result);
        })
    });
}

fn bench_snapshot_serialization(c: &mut Criterion) {
    let snapshot = create_test_snapshot();

    c.bench_function("snapshot_serialize_to_buffer", |b| {
        b.iter(|| {
            let buffer = black_box(&snapshot)
                .serialize_to_buffer()
                .unwrap_or_else(|e| panic!("Benchmark setup failed - serialize_to_buffer: {}", e));
            black_box(buffer);
        })
    });

    c.bench_function("snapshot_serialize_direct", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&snapshot))
                .unwrap_or_else(|e| panic!("Benchmark setup failed - serde_json: {}", e));
            black_box(json);
        })
    });
}

fn bench_snapshot_clone(c: &mut Criterion) {
    let snapshot = create_test_snapshot();

    c.bench_function("snapshot_clone_with_arc", |b| {
        b.iter(|| {
            let cloned = black_box(&snapshot).clone();
            black_box(cloned);
        })
    });
}

fn bench_full_tracking_workflow(c: &mut Criterion) {
    use project_indicator::tracking::ResultTracker;

    c.bench_function("full_tracking_write_snapshot", |b| {
        let temp_dir = TempDir::new()
            .unwrap_or_else(|e| panic!("Benchmark setup failed - TempDir::new: {}", e));
        let tracker = ResultTracker::with_path(temp_dir.path().to_path_buf())
            .unwrap_or_else(|e| panic!("Benchmark setup failed - ResultTracker::with_path: {}", e));

        b.iter(|| {
            let snapshot = create_test_snapshot();
            tracker
                .record(black_box(snapshot))
                .unwrap_or_else(|e| panic!("Benchmark iteration failed - record: {}", e));
        });

        // Ensure cleanup
        tracker.flush();
    });
}

criterion_group!(
    benches,
    bench_path_canonicalization_with_cache,
    bench_path_canonicalization_without_cache,
    bench_snapshot_serialization,
    bench_snapshot_clone,
    bench_full_tracking_workflow
);
criterion_main!(benches);
