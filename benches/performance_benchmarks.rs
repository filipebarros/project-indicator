#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::patterns::simple_wildcard_match;
use project_indicator::{Config, DetectionEngine};
use std::hint::black_box;
use tempfile::TempDir;
fn create_benchmark_project(
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
            std::fs::create_dir_all(base.join("tests"))?;

            for i in 0..file_count {
                std::fs::write(
                    base.join("src").join(format!("lib_{}.rs", i)),
                    format!("// Rust file {}\npub fn test() {{}}", i),
                )?;
                if i < file_count / 3 {
                    std::fs::write(
                        base.join("tests").join(format!("test_{}.rs", i)),
                        format!("// Test file {}\n#[test]\nfn test() {{}}", i),
                    )?;
                }
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
            std::fs::create_dir_all(base.join("tests"))?;

            for i in 0..file_count {
                std::fs::write(
                    base.join("src").join(format!("component_{}.js", i)),
                    format!(
                        "// JavaScript file {}\nexport default function Component() {{}}",
                        i
                    ),
                )?;
                std::fs::write(
                    base.join("src").join(format!("util_{}.ts", i)),
                    format!("// TypeScript file {}\nexport function util() {{}}", i),
                )?;
            }
        }
        "python" => {
            std::fs::write(
                base.join("pyproject.toml"),
                r#"
[tool.poetry]
name = "test"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.8"
django = "^4.0"
"#,
            )?;
            std::fs::create_dir_all(base.join("src"))?;

            for i in 0..file_count {
                std::fs::write(
                    base.join("src").join(format!("module_{}.py", i)),
                    format!("# Python file {}\ndef function():\n    pass", i),
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
    let mut group = c.benchmark_group("detection_basic");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let small_rust = create_benchmark_project("rust", 10).unwrap();
    let medium_rust = create_benchmark_project("rust", 50).unwrap();
    let large_rust = create_benchmark_project("rust", 200).unwrap();

    group.bench_with_input(
        BenchmarkId::new("rust", "small"),
        small_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.bench_with_input(
        BenchmarkId::new("rust", "medium"),
        medium_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.bench_with_input(
        BenchmarkId::new("rust", "large"),
        large_rust.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.finish();
}
fn bench_detection_project_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_project_types");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let rust_project = create_benchmark_project("rust", 50).unwrap();
    let js_project = create_benchmark_project("javascript", 50).unwrap();
    let python_project = create_benchmark_project("python", 50).unwrap();

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

    group.bench_with_input(
        BenchmarkId::new("project_type", "python"),
        python_project.path(),
        |b, path| b.iter(|| engine.detect(black_box(path))),
    );

    group.finish();
}
fn bench_incremental_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_detection");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_benchmark_project("rust", 100).unwrap();

    let _ = engine.detect(project.path()).unwrap();

    group.bench_function("cached_detection", |b| {
        b.iter(|| engine.detect(black_box(project.path())))
    });

    group.finish();
}
fn bench_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    let extensions = [
        "rs", "js", "ts", "py", "java", "go", "cpp", "h", "json", "toml", "yaml", "md",
    ];
    for (i, ext) in extensions.iter().cycle().take(1000).enumerate() {
        std::fs::write(base.join(format!("file_{}.{}", i, ext)), "content").unwrap();
    }

    group.bench_function("many_extensions", |b| {
        b.iter(|| engine.detect(black_box(base)))
    });

    group.finish();
}
fn bench_async_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("async_performance");

    let config = Config::default();
    let sync_engine = DetectionEngine::new(config.languages.clone());

    let large_project = create_benchmark_project("javascript", 500).unwrap();

    group.bench_function("sync_detection", |b| {
        b.iter(|| sync_engine.detect(black_box(large_project.path())))
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
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    group.bench_function("vec_allocation", |b| {
        b.iter(|| {
            let mut vec = Vec::<String>::with_capacity(10);
            for i in 0..10 {
                vec.push(format!("item_{}", i));
            }
            black_box(vec)
        })
    });

    group.bench_function("string_allocation", |b| {
        b.iter(|| {
            let mut s = String::with_capacity(100);
            for i in 0..10 {
                s.push_str(&format!("item_{}", i));
            }
            black_box(s)
        })
    });

    group.finish();
}
fn bench_simd_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_patterns");

    let test_files = vec![
        "very_long_filename_with_multiple_parts.js",
        "short.rs",
        "test_file_with_underscores_and_numbers_123.py",
        "CamelCaseFileName.java",
        "kebab-case-file-name.go",
        "file.with.multiple.dots.in.name.cpp",
    ];

    let patterns = vec!["*.js", "*.rs", "*test*", "*_*", "*.java", "*.*.*"];

    for pattern in &patterns {
        group.bench_function(format!("pattern_{}", pattern.replace("*", "star")), |b| {
            b.iter(|| {
                for file in &test_files {
                    black_box(simple_wildcard_match(file, pattern));
                }
            })
        });
    }

    group.finish();
}
fn bench_detection_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_pipeline");

    let config = Config::default();
    let engine = DetectionEngine::new(config.languages.clone());

    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    std::fs::write(
        base.join("Cargo.toml"),
        "[package]\nname=\"test\"\nversion=\"0.1.0\"",
    )
    .unwrap();
    std::fs::create_dir_all(base.join("src")).unwrap();
    for i in 0..20 {
        std::fs::write(
            base.join("src").join(format!("mod_{}.rs", i)),
            "// Rust code",
        )
        .unwrap();
    }

    std::fs::write(
        base.join("package.json"),
        r#"{"name":"test","dependencies":{"react":"18.0.0"}}"#,
    )
    .unwrap();
    std::fs::write(base.join("tsconfig.json"), "{}").unwrap();
    for i in 0..15 {
        std::fs::write(
            base.join(format!("component_{}.tsx", i)),
            "// React component",
        )
        .unwrap();
    }

    std::fs::write(base.join("requirements.txt"), "django\nflask").unwrap();
    for i in 0..10 {
        std::fs::write(base.join(format!("script_{}.py", i)), "# Python script").unwrap();
    }

    group.bench_function("complex_project", |b| {
        b.iter(|| engine.detect(black_box(base)))
    });

    group.finish();
}
fn bench_concurrent_detection(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("concurrent_detection");

    let config = Config::default();

    let projects: Vec<_> = (0..10)
        .map(|i| create_benchmark_project("rust", 20 + i * 5))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let project_paths: Vec<_> = projects.iter().map(|p| p.path()).collect();

    group.bench_function("sequential", |b| {
        b.iter(|| {
            for path in &project_paths {
                let engine = DetectionEngine::new(config.languages.clone());
                let _ = black_box(engine.detect(path));
            }
        })
    });

    group.bench_function("parallel", |b| {
        b.iter(|| {
            project_paths.par_iter().for_each(|path| {
                let engine = DetectionEngine::new(config.languages.clone());
                let _ = black_box(engine.detect(path));
            });
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_detection_basic,
    bench_detection_project_types,
    bench_incremental_detection,
    bench_pattern_matching,
    bench_async_performance,
    bench_filesystem_cache,
    bench_memory_allocation,
    bench_simd_patterns,
    bench_detection_pipeline,
    bench_concurrent_detection
);
criterion_main!(benches);
