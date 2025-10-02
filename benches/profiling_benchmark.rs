use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::config::TemplateGenerator;
use project_indicator::detection::engine::DetectionEngine;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a realistic monorepo project with multiple languages and frameworks
fn setup_monorepo(temp_dir: &TempDir) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let base = temp_dir.path();
    let mut project_paths = Vec::new();

    // Backend - Node.js with Express
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

    // Backend - Python with Django
    let backend_python = base.join("backend/python");
    fs::create_dir_all(&backend_python)?;
    fs::write(backend_python.join("requirements.txt"), "django\n")?;
    fs::write(backend_python.join("manage.py"), "#!/usr/bin/env python\n")?;
    project_paths.push(backend_python);

    // Backend - Rust with Actix
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

    // Frontend - React with Next.js
    let frontend = base.join("frontend");
    fs::create_dir_all(&frontend)?;
    fs::write(
        frontend.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "next": "^13.0.0"}}"#,
    )?;
    fs::write(frontend.join("App.tsx"), "import React from 'react';")?;
    project_paths.push(frontend);

    // Mobile - Kotlin with Ktor
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

    // Infrastructure - Go
    let infra = base.join("infrastructure");
    fs::create_dir_all(&infra)?;
    fs::write(infra.join("go.mod"), "module github.com/example/infra\n")?;
    fs::write(infra.join("main.go"), "package main\n")?;
    project_paths.push(infra);

    Ok(project_paths)
}

/// Create a large project with many files to stress-test scanning
fn setup_large_project(
    temp_dir: &TempDir,
    file_count: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = temp_dir.path();

    // Create src directory with many files
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

    // Add Cargo.toml
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

/// Create a deep nested project structure
fn setup_deep_nested_project(
    temp_dir: &TempDir,
    depth: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current = temp_dir.path().to_path_buf();

    // Create deep nesting
    for i in 0..depth {
        current = current.join(format!("level_{}", i));
        fs::create_dir_all(&current)?;

        // Add some files at each level
        fs::write(
            current.join(format!("file_{}.js", i)),
            "console.log('test');",
        )?;
    }

    // Add package.json at the deepest level
    fs::write(
        current.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0"}}"#,
    )?;

    Ok(current)
}

/// Benchmark full detection pipeline on monorepo
fn benchmark_monorepo_detection(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("monorepo_detection");

    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let project_paths = setup_monorepo(&temp_dir)?;

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngine::new(config.languages);

    // Benchmark each sub-project
    for (idx, path) in project_paths.iter().enumerate() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("project_{}", idx));
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

/// Benchmark scaling with file count
fn benchmark_file_count_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("file_count_scaling");

    for file_count in [50, 100, 200, 500].iter() {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let project_path = setup_large_project(&temp_dir, *file_count)?;

        let config = TemplateGenerator::generate_template(Some("full"))
            .map_err(|e| format!("Failed to generate template: {}", e))?;
        let engine = DetectionEngine::new(config.languages);

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

/// Benchmark upward traversal performance
fn benchmark_upward_traversal(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("upward_traversal");

    for depth in [5, 10, 20].iter() {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let deepest_path = setup_deep_nested_project(&temp_dir, *depth)?;

        let config = TemplateGenerator::generate_template(Some("full"))
            .map_err(|e| format!("Failed to generate template: {}", e))?;
        let engine = DetectionEngine::new(config.languages);

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

/// Benchmark early termination effectiveness
fn benchmark_early_termination(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let mut group = c.benchmark_group("early_termination");

    // Project with strong root indicators (should trigger early termination)
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

    // Create many files that won't matter due to early termination
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
    let engine = DetectionEngine::new(config.languages);

    group.bench_function("strong_indicators", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(black_box(temp_dir_strong.path())) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    // Project with weak indicators (needs full scan)
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

/// Benchmark framework detection overhead
fn benchmark_framework_detection_overhead(
    c: &mut Criterion,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Create Node.js project with multiple frameworks
    fs::write(
        temp_dir.path().join("package.json"),
        r#"{
  "dependencies": {
    "react": "^18.0.0",
    "next": "^13.0.0",
    "express": "^4.18.0",
    "vue": "^3.0.0",
    "svelte": "^4.0.0"
  }
}"#,
    )
    .map_err(|e| format!("Failed to write package.json: {}", e))?;

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngine::new(config.languages);

    c.bench_function("framework_detection_overhead", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(black_box(temp_dir.path())) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    Ok(())
}

/// Benchmark cache effectiveness with repeated detection
fn benchmark_cache_effectiveness(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n\n[dependencies]\nactix-web = \"4.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngine::new(config.languages);

    c.bench_function("repeated_detection_with_cache", |b| {
        b.iter(|| {
            // Simulate shell prompt behavior - detect same directory multiple times
            for _ in 0..5 {
                if let Err(e) = engine.detect(black_box(temp_dir.path())) {
                    eprintln!("Detection failed: {}", e);
                }
            }
        });
    });

    Ok(())
}

// Wrapper functions for criterion
fn benchmark_monorepo_detection_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_monorepo_detection(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_file_count_scaling_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_file_count_scaling(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_upward_traversal_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_upward_traversal(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_early_termination_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_early_termination(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_framework_detection_overhead_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_framework_detection_overhead(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_cache_effectiveness_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_cache_effectiveness(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

criterion_group!(
    benches,
    benchmark_monorepo_detection_wrapper,
    benchmark_file_count_scaling_wrapper,
    benchmark_upward_traversal_wrapper,
    benchmark_early_termination_wrapper,
    benchmark_framework_detection_overhead_wrapper,
    benchmark_cache_effectiveness_wrapper
);
criterion_main!(benches);
