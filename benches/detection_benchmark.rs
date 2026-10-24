//! End-to-end detection benchmarks: full `DetectionEngine::detect` runs on
//! synthetic projects (monorepo, size scaling, upward traversal, indicator
//! strength, per-run cache effectiveness, framework/language scenarios).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::config::TemplateGenerator;
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

    let src = base.join("src");
    fs::create_dir_all(&src)?;

    for i in 0..file_count {
        let dir = src.join(format!("module_{}", i / 10));
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join(format!("file_{}.rs", i)),
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

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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

    for file_count in [50, 100, 200, 500].iter() {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let project_path = setup_large_project(&temp_dir, *file_count)?;

        let config = TemplateGenerator::generate_template(Some("full"))
            .map_err(|e| format!("Failed to generate template: {}", e))?;
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

        let config = TemplateGenerator::generate_template(Some("full"))
            .map_err(|e| format!("Failed to generate template: {}", e))?;
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

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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

/// Warm engine vs the CLI's real cold start.
///
/// `DetectionEngine` owns its per-run caches and never clears them, so every
/// bench above that loops `detect()` on ONE engine measures warm-engine time
/// after the first iteration. The CLI builds a fresh engine per process (or
/// skips the engine entirely on a persistent-cache hit), so
/// `cold_engine_detection` is the number that corresponds to a cold prompt.
fn benchmark_cache_effectiveness(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n\n[dependencies]\nactix-web = \"4.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
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
);
criterion_main!(benches);
