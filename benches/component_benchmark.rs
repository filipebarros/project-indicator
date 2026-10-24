//! Component micro-benchmarks: pattern matching, the per-run filesystem
//! metadata cache, and the parsed-manifest cache.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::detection::caches::ParsedFileCache;
use project_indicator::patterns::matches_pattern;
use project_indicator::performance::FileSystemCache;
use std::fs;
use std::hint::black_box;
use tempfile::TempDir;

fn benchmark_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    group.bench_function("extension_matching", |b| {
        b.iter(|| {
            for i in 0..100 {
                let filename = format!("file_{}.rs", i);
                black_box(matches_pattern(&filename, "*.rs"));
            }
        });
    });

    group.bench_function("exact_match", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(matches_pattern("Cargo.toml", "Cargo.toml"));
            }
        });
    });

    group.bench_function("prefix_match", |b| {
        b.iter(|| {
            black_box(matches_pattern("test_file.rs", "test_*"));
            black_box(matches_pattern("main_app.js", "main_*"));
            black_box(matches_pattern("config_dev.json", "config_*"));
        });
    });

    group.bench_function("complex_pattern", |b| {
        b.iter(|| {
            for i in 0..100 {
                let filename = format!("src/test_{}.rs", i);
                black_box(matches_pattern(&filename, "*/test_*.rs"));
            }
        });
    });

    let filenames: Vec<String> = (0..100)
        .map(|i| format!("src/module_{}/file_{}.rs", i / 10, i))
        .collect();
    let patterns = vec!["*.rs", "*.toml", "*.json", "Cargo.*"];
    group.bench_function("realistic_workload", |b| {
        b.iter(|| {
            for filename in &filenames {
                for pattern in &patterns {
                    black_box(matches_pattern(filename, pattern));
                }
            }
        });
    });

    group.finish();
}

fn create_test_files(count: usize) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    for i in 0..count {
        let file_path = temp_dir.path().join(format!("file_{}.txt", i));
        std::fs::write(&file_path, format!("content {}", i))
            .map_err(|e| format!("Failed to write test file: {}", e))?;
    }

    Ok(temp_dir)
}

fn benchmark_cache_hits(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_hits");
    let cache = FileSystemCache::default();
    let temp_dir = create_test_files(100)?;

    for i in 0..100 {
        let path = temp_dir.path().join(format!("file_{}.txt", i));
        cache.exists(&path);
    }

    group.bench_function("sequential_100_files", |b| {
        b.iter(|| {
            for i in 0..100 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.exists(black_box(&path)));
            }
        });
    });

    group.finish();
    Ok(())
}

fn benchmark_cache_misses(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_misses");
    let temp_dir = create_test_files(100)?;

    group.bench_function("sequential_100_files", |b| {
        b.iter(|| {
            let cache = FileSystemCache::default(); // Fresh cache each time
            for i in 0..100 {
                let path = temp_dir.path().join(format!("file_{}.txt", i));
                black_box(cache.exists(black_box(&path)));
            }
        });
    });

    group.finish();
    Ok(())
}

fn benchmark_cache_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("cache_scaling");

    for count in [10, 50, 100, 500].iter() {
        let temp_dir = create_test_files(*count)?;
        let cache = FileSystemCache::default();

        for i in 0..*count {
            let path = temp_dir.path().join(format!("file_{}.txt", i));
            cache.exists(&path);
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                for i in 0..count {
                    let path = temp_dir.path().join(format!("file_{}.txt", i));
                    black_box(cache.exists(black_box(&path)));
                }
            });
        });
    }

    group.finish();
    Ok(())
}

fn benchmark_json_parsing(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("json_parsing");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let package_json = r#"{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^13.0.0",
    "typescript": "^5.0.0",
    "express": "^4.18.0",
    "axios": "^1.0.0"
  },
  "devDependencies": {
    "jest": "^29.0.0",
    "eslint": "^8.0.0",
    "prettier": "^3.0.0"
  }
}"#;

    fs::write(temp_dir.path().join("package.json"), package_json)
        .map_err(|e| format!("Failed to write package.json: {}", e))?;

    group.bench_function("json_cold_cache", |b| {
        b.iter(|| {
            let cache = ParsedFileCache::new();
            let package_json_path = temp_dir.path().join("package.json");
            if let Err(e) = cache.get_json_value(&package_json_path) {
                eprintln!("JSON parsing failed: {}", e);
            }
        });
    });

    group.bench_function("json_warm_cache", |b| {
        let cache = ParsedFileCache::new();
        let package_json_path = temp_dir.path().join("package.json");
        if let Err(e) = cache.get_json_value(&package_json_path) {
            eprintln!("JSON parsing failed: {}", e);
        }

        b.iter(|| {
            if let Err(e) = cache.get_json_value(&package_json_path) {
                eprintln!("JSON parsing failed: {}", e);
            }
        });
    });

    group.bench_function("json_repeated_access", |b| {
        let cache = ParsedFileCache::new();
        let package_json_path = temp_dir.path().join("package.json");

        b.iter(|| {
            for _ in 0..10 {
                if let Err(e) = cache.get_json_value(&package_json_path) {
                    eprintln!("JSON parsing failed: {}", e);
                }
            }
        });
    });

    group.finish();
    Ok(())
}

fn benchmark_toml_parsing(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("toml_parsing");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let cargo_toml = r#"[package]
name = "project-indicator"
version = "0.3.0"
edition = "2021"

[dependencies]
clap = { version = "4.0", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.9"
serde_json = "1.0"
ignore = "0.4"
rayon = "1.7"
regex = "1.11"
anyhow = "1.0"
thiserror = "2.0"
dirs = "6.0"
log = "0.4"
env_logger = "0.11"
dashmap = "6.0"

[dev-dependencies]
criterion = "0.7"
tempfile = "3.0"
"#;

    fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml)
        .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    group.bench_function("toml_cold_cache", |b| {
        b.iter(|| {
            let cache = ParsedFileCache::new();
            let cargo_toml_path = temp_dir.path().join("Cargo.toml");
            if let Err(e) = cache.get_toml_value(&cargo_toml_path) {
                eprintln!("TOML parsing failed: {}", e);
            }
        });
    });

    group.bench_function("toml_warm_cache", |b| {
        let cache = ParsedFileCache::new();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        if let Err(e) = cache.get_toml_value(&cargo_toml_path) {
            eprintln!("TOML parsing failed: {}", e);
        }

        b.iter(|| {
            if let Err(e) = cache.get_toml_value(&cargo_toml_path) {
                eprintln!("TOML parsing failed: {}", e);
            }
        });
    });

    group.bench_function("toml_repeated_access", |b| {
        let cache = ParsedFileCache::new();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");

        b.iter(|| {
            for _ in 0..10 {
                if let Err(e) = cache.get_toml_value(&cargo_toml_path) {
                    eprintln!("TOML parsing failed: {}", e);
                }
            }
        });
    });

    group.finish();
    Ok(())
}

fn benchmark_mixed_access(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(
        temp_dir.path().join("package.json"),
        r#"{"name": "test", "version": "1.0.0"}"#,
    )
    .map_err(|e| format!("Failed to write package.json: {}", e))?;

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "test"
version = "0.1.0"
"#,
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    fs::write(
        temp_dir.path().join("pyproject.toml"),
        r#"[tool.poetry]
name = "test"
version = "0.1.0"
"#,
    )
    .map_err(|e| format!("Failed to write pyproject.toml: {}", e))?;

    c.bench_function("mixed_files_sequential", |b| {
        let cache = ParsedFileCache::new();
        let package_json_path = temp_dir.path().join("package.json");
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let pyproject_toml_path = temp_dir.path().join("pyproject.toml");

        b.iter(|| {
            if let Err(e) = cache.get_json_value(&package_json_path) {
                eprintln!("JSON parsing failed: {}", e);
            }
            if let Err(e) = cache.get_toml_value(&cargo_toml_path) {
                eprintln!("TOML parsing failed: {}", e);
            }
            if let Err(e) = cache.get_toml_value(&pyproject_toml_path) {
                eprintln!("PyProject TOML parsing failed: {}", e);
            }
        });
    });

    Ok(())
}

fn benchmark_parse_failures(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(temp_dir.path().join("bad.json"), r#"{"invalid json"#)
        .map_err(|e| format!("Failed to write bad.json: {}", e))?;

    fs::write(temp_dir.path().join("bad.toml"), r#"[invalid toml"#)
        .map_err(|e| format!("Failed to write bad.toml: {}", e))?;

    c.bench_function("parse_failure_json_first_attempt", |b| {
        b.iter(|| {
            let cache = ParsedFileCache::new();
            let _ = cache.get_json_value(temp_dir.path().join("bad.json"));
        });
    });

    c.bench_function("parse_failure_json_cached", |b| {
        let cache = ParsedFileCache::new();
        let _ = cache.get_json_value(temp_dir.path().join("bad.json"));

        b.iter(|| {
            let _ = black_box(cache.get_json_value(temp_dir.path().join("bad.json")));
        });
    });

    Ok(())
}

/// Criterion wants `fn(&mut Criterion)`: a failing setup aborts the run
macro_rules! infallible {
    ($($wrapper:ident => $bench:ident),+ $(,)?) => {$(
        fn $wrapper(c: &mut Criterion) {
            if let Err(e) = $bench(c) {
                eprintln!("Benchmark failed: {e}");
                std::process::exit(1);
            }
        }
    )+};
}
infallible!(
    benchmark_cache_hits_wrapper => benchmark_cache_hits,
    benchmark_cache_misses_wrapper => benchmark_cache_misses,
    benchmark_cache_scaling_wrapper => benchmark_cache_scaling,
    benchmark_json_parsing_wrapper => benchmark_json_parsing,
    benchmark_toml_parsing_wrapper => benchmark_toml_parsing,
    benchmark_mixed_access_wrapper => benchmark_mixed_access,
    benchmark_parse_failures_wrapper => benchmark_parse_failures,
);

criterion_group!(
    benches,
    benchmark_pattern_matching,
    benchmark_cache_hits_wrapper,
    benchmark_cache_misses_wrapper,
    benchmark_cache_scaling_wrapper,
    benchmark_json_parsing_wrapper,
    benchmark_toml_parsing_wrapper,
    benchmark_mixed_access_wrapper,
    benchmark_parse_failures_wrapper
);
criterion_main!(benches);
