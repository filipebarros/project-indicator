#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::{Config, DetectionEngine};
use std::hint::black_box;
use tempfile::TempDir;
fn create_test_project(
    name: &str,
    file_count: usize,
) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let base = temp_dir.path();

    match name {
        "rust" => {
            std::fs::write(
                base.join("Cargo.toml"),
                r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#,
            )?;
            std::fs::create_dir_all(base.join("src"))?;

            for i in 0..file_count {
                std::fs::write(
                    base.join("src").join(format!("lib_{}.rs", i)),
                    format!("// Rust file {}\npub fn test() {{}}", i),
                )?;
            }
        }
        "javascript" => {
            std::fs::write(
                base.join("package.json"),
                r#"
{
  "name": "test",
  "dependencies": {
    "react": "^18.0.0",
    "typescript": "^4.0.0"
  }
}
"#,
            )?;
            std::fs::create_dir_all(base.join("src"))?;

            for i in 0..file_count {
                std::fs::write(
                    base.join("src").join(format!("component_{}.js", i)),
                    format!(
                        "// JavaScript file {}\nexport default function Component() {{}}",
                        i
                    ),
                )?;
            }
        }
        _ => {
            for i in 0..file_count {
                std::fs::write(
                    base.join(format!("file_{}.txt", i)),
                    format!("Generic file {}", i),
                )?;
            }
        }
    }

    Ok(temp_dir)
}
fn bench_detection_basic(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_performance");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let small_rust = create_test_project("rust", 10).unwrap();
    let medium_rust = create_test_project("rust", 50).unwrap();
    let large_rust = create_test_project("rust", 200).unwrap();

    group.bench_with_input(
        BenchmarkId::new("rust_project", "small_10_files"),
        small_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.bench_with_input(
        BenchmarkId::new("rust_project", "medium_50_files"),
        medium_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.bench_with_input(
        BenchmarkId::new("rust_project", "large_200_files"),
        large_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.finish();
}
fn bench_project_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("project_types");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let rust_project = create_test_project("rust", 30).unwrap();
    let js_project = create_test_project("javascript", 30).unwrap();

    group.bench_with_input(
        BenchmarkId::new("project_type", "rust"),
        rust_project.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.bench_with_input(
        BenchmarkId::new("project_type", "javascript"),
        js_project.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.finish();
}
fn bench_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_test_project("rust", 100).unwrap();

    let _ = engine.detect(project.path()).unwrap();

    group.bench_function("cached_detection", |b| {
        b.iter(|| engine.detect(black_box(project.path())))
    });

    group.finish();
}
fn bench_filesystem_cache(c: &mut Criterion) {
    use project_indicator::performance::FileSystemCache;

    let mut group = c.benchmark_group("filesystem_cache");
    let cache = FileSystemCache::default();
    let test_file = std::env::temp_dir().join("benchmark_test_file.txt");
    std::fs::write(&test_file, "test content").unwrap();

    group.bench_function("cache_hit", |b| {
        let _ = cache.get_metadata(&test_file);

        b.iter(|| cache.get_metadata(black_box(&test_file)))
    });

    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            cache.clear();
            cache.get_metadata(black_box(&test_file))
        })
    });

    std::fs::remove_file(&test_file).ok();
    group.finish();
}

criterion_group!(
    benches,
    bench_detection_basic,
    bench_project_types,
    bench_cache_performance,
    bench_filesystem_cache
);
criterion_main!(benches);
