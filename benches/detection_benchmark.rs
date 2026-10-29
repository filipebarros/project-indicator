//! End-to-end detection benchmarks: full `DetectionEngine::detect` runs on
//! synthetic projects (monorepo, size scaling, upward traversal, indicator
//! strength, per-run cache effectiveness, framework/language scenarios).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::config::built_in_config;
use project_indicator::detection::engine::DetectionEngine;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_monorepo(temp_dir: &TempDir) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let base = temp_dir.path();
    let mut project_paths = Vec::new();

    let backend_node = base.join("backend/node");
    fs::create_dir_all(&backend_node)?;
    fs::write(
        backend_node.join("package.json"),
        r#"{"dependencies": {"express": "^4.18.0"}}"#,
    )?;
    fs::write(
        backend_node.join("server.js"),
        "const express = require('express');",
    )?;
    project_paths.push(backend_node);

    let backend_python = base.join("backend/python");
    fs::create_dir_all(&backend_python)?;
    fs::write(backend_python.join("requirements.txt"), "django\n")?;
    fs::write(backend_python.join("manage.py"), "#!/usr/bin/env python\n")?;
    project_paths.push(backend_python);

    let backend_rust = base.join("backend/rust");
    fs::create_dir_all(&backend_rust)?;
    fs::write(
        backend_rust.join("Cargo.toml"),
        r#"[package]
name = "backend"
version = "0.1.0"

[dependencies]
actix-web = "4.0"
"#,
    )?;
    fs::write(backend_rust.join("main.rs"), "use actix_web::*;")?;
    project_paths.push(backend_rust);

    let frontend = base.join("frontend");
    fs::create_dir_all(&frontend)?;
    fs::write(
        frontend.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "next": "^13.0.0"}}"#,
    )?;
    fs::write(frontend.join("App.tsx"), "import React from 'react';")?;
    project_paths.push(frontend);

    let mobile = base.join("mobile/android");
    fs::create_dir_all(&mobile)?;
    fs::write(
        mobile.join("build.gradle.kts"),
        r#"dependencies {
    implementation("io.ktor:ktor-server-core:2.3.5")
}"#,
    )?;
    fs::write(mobile.join("Main.kt"), "package com.example\n")?;
    project_paths.push(mobile);

    let infra = base.join("infrastructure");
    fs::create_dir_all(&infra)?;
    fs::write(infra.join("go.mod"), "module github.com/example/infra\n")?;
    fs::write(infra.join("main.go"), "package main\n")?;
    project_paths.push(infra);

    Ok(project_paths)
}

fn setup_large_project(
    temp_dir: &TempDir,
    file_count: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = temp_dir.path();

    for i in 0..file_count {
        fs::write(
            base.join(format!("file_{i:05}.rs")),
            "fn main() { println!(\"Hello\"); }",
        )?;
    }

    fs::write(
        base.join("Cargo.toml"),
        r#"[package]
name = "large_project"
version = "0.1.0"

[dependencies]
tokio = "1.0"
"#,
    )?;

    Ok(base.to_path_buf())
}

fn setup_deep_nested_project(
    temp_dir: &TempDir,
    depth: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current = temp_dir.path().to_path_buf();

    for i in 0..depth {
        current = current.join(format!("level_{}", i));
        fs::create_dir_all(&current)?;

        fs::write(
            current.join(format!("file_{}.js", i)),
            "console.log('test');",
        )?;
    }

    fs::write(
        current.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0"}}"#,
    )?;

    Ok(current)
}

fn benchmark_monorepo_detection(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("monorepo_detection");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let project_paths = setup_monorepo(&temp_dir)?;

    let config = built_in_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);

    for (idx, path) in project_paths.iter().enumerate() {
        let name = path.file_name().map_or_else(
            || format!("project_{}", idx),
            |n| n.to_string_lossy().to_string(),
        );
        group.bench_with_input(BenchmarkId::new("subproject", &name), &idx, |b, _| {
            b.iter(|| {
                if let Err(e) = engine.detect(black_box(path)) {
                    eprintln!("Detection failed: {}", e);
                }
            });
        });
    }

    group.finish();
    Ok(())
}

fn benchmark_file_count_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("file_count_scaling");

    for file_count in [50, 100, 200, 500, 5_000].iter() {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let project_path = setup_large_project(&temp_dir, *file_count)?;

        let config = built_in_config();
        let engine = DetectionEngine::new(config.indicators, config.frameworks);

        group.bench_with_input(BenchmarkId::new("files", file_count), file_count, |b, _| {
            b.iter(|| {
                if let Err(e) = engine.detect(black_box(&project_path)) {
                    eprintln!("Detection failed: {}", e);
                }
            });
        });
    }

    group.finish();
    Ok(())
}

fn benchmark_upward_traversal(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("upward_traversal");

    for depth in [5, 10, 20].iter() {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let deepest_path = setup_deep_nested_project(&temp_dir, *depth)?;

        let config = built_in_config();
        let engine = DetectionEngine::new(config.indicators, config.frameworks);

        group.bench_with_input(BenchmarkId::new("depth", depth), depth, |b, _| {
            b.iter(|| {
                if let Err(e) = engine.detect(black_box(&deepest_path)) {
                    eprintln!("Detection failed: {}", e);
                }
            });
        });
    }

    group.finish();
    Ok(())
}

fn benchmark_indicator_strength(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("indicator_strength");

    let temp_dir_strong =
        TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    fs::write(
        temp_dir_strong.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;
    fs::write(
        temp_dir_strong.path().join("Cargo.lock"),
        "# This file is automatically generated",
    )
    .map_err(|e| format!("Failed to write Cargo.lock: {}", e))?;

    let src = temp_dir_strong.path().join("src");
    fs::create_dir_all(&src).map_err(|e| format!("Failed to create src: {}", e))?;
    for i in 0..100 {
        fs::write(
            src.join(format!("file_{}.rs", i)),
            "fn main() { println!(\"test\"); }",
        )
        .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    let config = built_in_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);

    group.bench_function("strong_indicators", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(black_box(temp_dir_strong.path())) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    let temp_dir_weak = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    for i in 0..100 {
        fs::write(
            temp_dir_weak.path().join(format!("file_{}.js", i)),
            "console.log('test');",
        )
        .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    group.bench_function("weak_indicators", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(black_box(temp_dir_weak.path())) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.finish();
    Ok(())
}

/// Engine construction plus detection versus detection with an existing engine.
///
/// Filesystem and parsed-file caches are created inside every `detect` call,
/// so `repeated_detection` still pays for fresh per-run caches. The CLI also
/// constructs a fresh engine per process unless its persistent cache hits.
fn benchmark_cache_effectiveness(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n\n[dependencies]\nactix-web = \"4.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    let config = built_in_config();
    let engine = DetectionEngine::new(config.indicators.clone(), config.frameworks.clone());

    // Fresh engine per iteration (setup excluded): what one CLI run pays
    c.bench_function("cold_engine_detection", |b| {
        b.iter_batched(
            || DetectionEngine::new(config.indicators.clone(), config.frameworks.clone()),
            |engine| {
                if let Err(e) = engine.detect(black_box(temp_dir.path())) {
                    eprintln!("Detection failed: {}", e);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("repeated_detection", |b| {
        b.iter(|| {
            for _ in 0..5 {
                if let Err(e) = engine.detect(black_box(temp_dir.path())) {
                    eprintln!("Detection failed: {}", e);
                }
            }
        });
    });

    Ok(())
}

fn create_node_multi_framework_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let package_json = r#"{
  "name": "multi-framework-project",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^13.0.0",
    "express": "^4.18.0",
    "vue": "^3.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "jest": "^29.0.0"
  }
}"#;

    fs::write(temp_dir.path().join("package.json"), package_json)
        .map_err(|e| format!("Failed to write package.json: {}", e))?;

    fs::write(temp_dir.path().join("index.js"), "// Main file")
        .map_err(|e| format!("Failed to write index.js: {}", e))?;

    Ok(temp_dir)
}

fn create_rust_multi_framework_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let cargo_toml = r#"[package]
name = "rust-project"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-web = "4.0"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.6"
rocket = "0.5"
warp = "0.3"
"#;

    fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml)
        .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    fs::create_dir_all(temp_dir.path().join("src"))
        .map_err(|e| format!("Failed to create src dir: {}", e))?;
    fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}")
        .map_err(|e| format!("Failed to write main.rs: {}", e))?;

    Ok(temp_dir)
}

fn create_python_multi_framework_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let requirements = r#"django==4.2.0
flask==2.3.0
fastapi==0.100.0
pydantic==2.0.0
"#;

    fs::write(temp_dir.path().join("requirements.txt"), requirements)
        .map_err(|e| format!("Failed to write requirements.txt: {}", e))?;

    fs::write(temp_dir.path().join("app.py"), "# Python app")
        .map_err(|e| format!("Failed to write app.py: {}", e))?;

    Ok(temp_dir)
}

fn create_polyglot_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(
        temp_dir.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "express": "^4.0.0"}}"#,
    )
    .map_err(|e| format!("Failed to write package.json: {}", e))?;

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\n\n[dependencies]\nactix-web = \"4.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    fs::write(temp_dir.path().join("requirements.txt"), "django\nflask")
        .map_err(|e| format!("Failed to write requirements.txt: {}", e))?;

    fs::write(
        temp_dir.path().join("go.mod"),
        "module example.com/app\n\nrequire github.com/gin-gonic/gin v1.9.0",
    )
    .map_err(|e| format!("Failed to write go.mod: {}", e))?;

    Ok(temp_dir)
}

fn benchmark_single_language_detection(
    c: &mut Criterion,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);

    let node_project = create_node_multi_framework_project()?;
    let rust_project = create_rust_multi_framework_project()?;
    let python_project = create_python_multi_framework_project()?;

    let mut group = c.benchmark_group("single_language_detection");

    group.bench_function("node_multi_framework", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(node_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.bench_function("rust_multi_framework", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(rust_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.bench_function("python_multi_framework", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(python_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.finish();
    Ok(())
}

fn benchmark_polyglot_detection(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);

    let polyglot_project = create_polyglot_project()?;

    c.bench_function("polyglot_detection", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(polyglot_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    Ok(())
}

fn benchmark_framework_count_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);

    let temp1 = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    fs::write(
        temp1.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0"}}"#,
    )
    .map_err(|e| format!("Failed to write: {}", e))?;

    c.bench_function("1_framework", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(temp1.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    let temp3 = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    fs::write(
        temp3.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "express": "^4.0.0", "vue": "^3.0.0"}}"#,
    )
    .map_err(|e| format!("Failed to write: {}", e))?;

    c.bench_function("3_frameworks", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(temp3.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    let temp6 = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    fs::write(
        temp6.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "express": "^4.0.0", "vue": "^3.0.0", "next": "^13.0.0", "svelte": "^4.0.0", "angular": "^16.0.0"}}"#,
    )
    .map_err(|e| format!("Failed to write: {}", e))?;

    c.bench_function("6_frameworks", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(temp6.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    Ok(())
}

fn setup_repository(
    temp_dir: &TempDir,
    candidate_dirs: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = temp_dir.path();
    fs::create_dir_all(root.join(".git"))?;
    for index in 0..candidate_dirs {
        let project = root.join(format!("group_{index:05}/project_{index:05}"));
        fs::create_dir_all(&project)?;
        if index % 100 == 0 {
            fs::write(project.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
        } else {
            fs::write(project.join(".keep"), "")?;
        }
    }
    Ok(root.to_path_buf())
}

fn setup_dense_node_repository(
    temp_dir: &TempDir,
    project_count: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = temp_dir.path();
    fs::create_dir_all(root.join(".git"))?;
    for index in 0..project_count {
        let project = root.join(format!("project_{index:05}"));
        fs::create_dir_all(&project)?;
        fs::write(
            project.join("package.json"),
            r#"{"dependencies":{"react":"18"}}"#,
        )?;
    }
    Ok(root.to_path_buf())
}

fn benchmark_dense_node_repository(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("repository_dense_node");
    group.sample_size(10);
    group.warm_up_time(std::time::Duration::from_millis(250));
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);

    for project_count in [100, 1_000] {
        let repository = TempDir::new()?;
        let root = setup_dense_node_repository(&repository, project_count)?;
        let result = engine.detect(&root)?;
        assert!(!result.evidence.scan_truncated);
        assert_eq!(result.nested.len(), project_count);

        group.bench_with_input(BenchmarkId::new("cold", project_count), &root, |b, root| {
            b.iter(|| {
                if let Err(e) = engine.detect(black_box(root)) {
                    eprintln!("Detection failed: {e}");
                }
            });
        });
    }

    group.finish();
    Ok(())
}

fn benchmark_repository_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("repository_scaling");
    group.sample_size(10);
    group.warm_up_time(std::time::Duration::from_millis(250));
    let config = built_in_config();
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );

    for candidate_dirs in [100, 1_000, 10_000] {
        let repository = TempDir::new()?;
        let root = setup_repository(&repository, candidate_dirs)?;
        std::thread::sleep(std::time::Duration::from_millis(60));
        let started = project_indicator::cache::detection_started();
        let result = engine.detect(&root)?;
        assert!(
            !result.evidence.scan_truncated,
            "{candidate_dirs} directories truncated the repository scan"
        );
        println!(
            "repository dirs={candidate_dirs} watched={} read_files={} projects={}",
            result.evidence.watched_dirs.len(),
            result.evidence.read_files.len(),
            result.nested.len()
        );

        group.bench_with_input(
            BenchmarkId::new("cold", candidate_dirs),
            &root,
            |b, root| {
                b.iter(|| {
                    if let Err(e) = engine.detect(black_box(root)) {
                        eprintln!("Detection failed: {e}");
                    }
                });
            },
        );

        let cache_dir = TempDir::new()?;
        let cache =
            project_indicator::cache::PersistentCache::at_base(cache_dir.path().join("results"));
        cache.store(
            &root,
            None,
            &result,
            &config.display,
            &config.detection,
            started,
        );
        let entry_bytes = fs::read_dir(cache_dir.path().join("results"))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        println!("repository dirs={candidate_dirs} cache_bytes={entry_bytes}");
        assert!(
            cache.load(&root, None).is_some(),
            "{candidate_dirs} directories did not produce a cache entry"
        );

        group.bench_with_input(
            BenchmarkId::new("warm", candidate_dirs),
            &root,
            |b, root| {
                b.iter(|| {
                    assert!(cache.load(black_box(root), None).is_some());
                });
            },
        );
    }

    group.finish();
    Ok(())
}

/// Dotfiles-shaped repository: the root itself is a project, descendants
/// beneath that project ancestor carry source files, and qualifying markers
/// appear at the same 100-directory interval as the flat shape.
fn setup_root_project_repository(
    temp_dir: &TempDir,
    candidate_dirs: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = temp_dir.path();
    fs::create_dir_all(root.join(".git"))?;
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"root\"\n")?;
    fs::write(root.join(".gitignore"), "generated/\n")?;
    for index in 0..candidate_dirs {
        let component = root.join(format!("component_{index:05}"));
        fs::create_dir_all(component.join("src"))?;
        fs::write(component.join("src/lib.rs"), "pub fn f() {}\n")?;
        if index % 100 == 0 {
            fs::write(
                component.join("package.json"),
                r#"{"devDependencies":{"typescript":"5"}}"#,
            )?;
        }
        let generated = root.join(format!("generated/package_{index:05}"));
        fs::create_dir_all(&generated)?;
        fs::write(generated.join("package.json"), "{}")?;
    }
    Ok(root.to_path_buf())
}

fn benchmark_root_project_repository_scaling(
    c: &mut Criterion,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("repository_scaling_root_project");
    group.sample_size(10);
    group.warm_up_time(std::time::Duration::from_millis(250));
    let config = built_in_config();
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );

    for candidate_dirs in [100, 1_000, 10_000] {
        let repository = TempDir::new()?;
        let root = setup_root_project_repository(&repository, candidate_dirs)?;
        std::thread::sleep(std::time::Duration::from_millis(60));
        let started = project_indicator::cache::detection_started();
        let result = engine.detect(&root)?;
        assert!(
            !result.evidence.scan_truncated,
            "{candidate_dirs} directories truncated the root-project repository scan"
        );
        assert!(result
            .nested
            .iter()
            .all(|project| !project.name.starts_with("generated/")));
        println!(
            "root-project repository dirs={candidate_dirs} watched={} read_files={} projects={}",
            result.evidence.watched_dirs.len(),
            result.evidence.read_files.len(),
            result.nested.len()
        );

        group.bench_with_input(
            BenchmarkId::new("cold", candidate_dirs),
            &root,
            |b, root| {
                b.iter(|| {
                    if let Err(e) = engine.detect(black_box(root)) {
                        eprintln!("Detection failed: {e}");
                    }
                });
            },
        );

        let cache_dir = TempDir::new()?;
        let cache =
            project_indicator::cache::PersistentCache::at_base(cache_dir.path().join("results"));
        cache.store(
            &root,
            None,
            &result,
            &config.display,
            &config.detection,
            started,
        );
        let entry_bytes = fs::read_dir(cache_dir.path().join("results"))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        println!("root-project repository dirs={candidate_dirs} cache_bytes={entry_bytes}");
        assert!(
            cache.load(&root, None).is_some(),
            "{candidate_dirs} directories did not produce a cache entry"
        );

        group.bench_with_input(
            BenchmarkId::new("warm", candidate_dirs),
            &root,
            |b, root| {
                b.iter(|| {
                    assert!(cache.load(black_box(root), None).is_some());
                });
            },
        );
    }

    group.finish();
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
    benchmark_monorepo_detection_wrapper => benchmark_monorepo_detection,
    benchmark_file_count_scaling_wrapper => benchmark_file_count_scaling,
    benchmark_upward_traversal_wrapper => benchmark_upward_traversal,
    benchmark_indicator_strength_wrapper => benchmark_indicator_strength,
    benchmark_cache_effectiveness_wrapper => benchmark_cache_effectiveness,
    benchmark_single_language_detection_wrapper => benchmark_single_language_detection,
    benchmark_polyglot_detection_wrapper => benchmark_polyglot_detection,
    benchmark_framework_count_scaling_wrapper => benchmark_framework_count_scaling,
    benchmark_dense_node_repository_wrapper => benchmark_dense_node_repository,
    benchmark_repository_scaling_wrapper => benchmark_repository_scaling,
    benchmark_root_project_repository_scaling_wrapper =>
        benchmark_root_project_repository_scaling,
);

criterion_group!(
    benches,
    benchmark_monorepo_detection_wrapper,
    benchmark_file_count_scaling_wrapper,
    benchmark_upward_traversal_wrapper,
    benchmark_indicator_strength_wrapper,
    benchmark_cache_effectiveness_wrapper,
    benchmark_single_language_detection_wrapper,
    benchmark_polyglot_detection_wrapper,
    benchmark_framework_count_scaling_wrapper,
    benchmark_dense_node_repository_wrapper,
    benchmark_repository_scaling_wrapper,
    benchmark_root_project_repository_scaling_wrapper,
);
criterion_main!(benches);
