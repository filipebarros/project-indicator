use criterion::{criterion_group, criterion_main, Criterion};
use project_indicator::detection::caches::ParsedFileCache;
use std::fs;
use std::hint::black_box;
use tempfile::TempDir;

/// Benchmark JSON parsing with caching
fn benchmark_json_parsing(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("json_parsing");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Create a realistic package.json
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
            if let Err(e) = cache.get_package_json(temp_dir.path()) {
                eprintln!("JSON parsing failed: {}", e);
            }
        });
    });

    group.bench_function("json_warm_cache", |b| {
        let cache = ParsedFileCache::new();
        // Pre-warm
        if let Err(e) = cache.get_package_json(temp_dir.path()) {
            eprintln!("JSON parsing failed: {}", e);
        }

        b.iter(|| {
            if let Err(e) = cache.get_package_json(temp_dir.path()) {
                eprintln!("JSON parsing failed: {}", e);
            }
        });
    });

    group.bench_function("json_repeated_access", |b| {
        let cache = ParsedFileCache::new();

        b.iter(|| {
            for _ in 0..10 {
                if let Err(e) = cache.get_package_json(temp_dir.path()) {
                    eprintln!("JSON parsing failed: {}", e);
                }
            }
        });
    });

    group.finish();
    Ok(())
}

/// Benchmark TOML parsing with caching
fn benchmark_toml_parsing(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("toml_parsing");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Create a realistic Cargo.toml
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
            if let Err(e) = cache.get_cargo_toml(temp_dir.path()) {
                eprintln!("TOML parsing failed: {}", e);
            }
        });
    });

    group.bench_function("toml_warm_cache", |b| {
        let cache = ParsedFileCache::new();
        // Pre-warm
        if let Err(e) = cache.get_cargo_toml(temp_dir.path()) {
            eprintln!("TOML parsing failed: {}", e);
        }

        b.iter(|| {
            if let Err(e) = cache.get_cargo_toml(temp_dir.path()) {
                eprintln!("TOML parsing failed: {}", e);
            }
        });
    });

    group.bench_function("toml_repeated_access", |b| {
        let cache = ParsedFileCache::new();

        b.iter(|| {
            for _ in 0..10 {
                if let Err(e) = cache.get_cargo_toml(temp_dir.path()) {
                    eprintln!("TOML parsing failed: {}", e);
                }
            }
        });
    });

    group.finish();
    Ok(())
}

/// Benchmark mixed file types
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

        b.iter(|| {
            if let Err(e) = cache.get_package_json(temp_dir.path()) {
                eprintln!("JSON parsing failed: {}", e);
            }
            if let Err(e) = cache.get_cargo_toml(temp_dir.path()) {
                eprintln!("TOML parsing failed: {}", e);
            }
            if let Err(e) = cache.get_pyproject_toml(temp_dir.path()) {
                eprintln!("PyProject TOML parsing failed: {}", e);
            }
        });
    });

    Ok(())
}

/// Benchmark parse failure handling
fn benchmark_parse_failures(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Create invalid JSON
    fs::write(temp_dir.path().join("bad.json"), r#"{"invalid json"#)
        .map_err(|e| format!("Failed to write bad.json: {}", e))?;

    // Create invalid TOML
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
        // First attempt to cache the failure
        let _ = cache.get_json_value(temp_dir.path().join("bad.json"));

        b.iter(|| {
            // Subsequent attempts should be fast
            let _ = black_box(cache.get_json_value(temp_dir.path().join("bad.json")));
        });
    });

    Ok(())
}

/// Benchmark cache statistics overhead
fn benchmark_cache_stats(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(temp_dir.path().join("package.json"), r#"{"name": "test"}"#)
        .map_err(|e| format!("Failed to write package.json: {}", e))?;

    let cache = ParsedFileCache::new();
    // Pre-populate
    for _ in 0..50 {
        if let Err(e) = cache.get_package_json(temp_dir.path()) {
            eprintln!("JSON parsing failed: {}", e);
        }
    }

    c.bench_function("cache_stats_call", |b| {
        b.iter(|| {
            black_box(cache.stats());
        });
    });

    Ok(())
}

// Wrapper functions for criterion
fn benchmark_json_parsing_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_json_parsing(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_toml_parsing_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_toml_parsing(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_mixed_access_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_mixed_access(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_parse_failures_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_parse_failures(c) {
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
    benchmark_json_parsing_wrapper,
    benchmark_toml_parsing_wrapper,
    benchmark_mixed_access_wrapper,
    benchmark_parse_failures_wrapper,
    benchmark_cache_stats_wrapper
);
criterion_main!(benches);
