use criterion::{criterion_group, criterion_main, Criterion};
use project_indicator::config::TemplateGenerator;
use project_indicator::detection::engine::DetectionEngineBuilder;
use std::fs;
use tempfile::TempDir;

/// Create a multi-framework Node.js project
fn create_node_multi_framework_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // package.json with multiple frameworks
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

/// Create a Rust project with multiple frameworks
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

/// Create a Python project with multiple frameworks
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

/// Create a polyglot project with multiple languages
fn create_polyglot_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Node.js
    fs::write(
        temp_dir.path().join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "express": "^4.0.0"}}"#,
    )
    .map_err(|e| format!("Failed to write package.json: {}", e))?;

    // Rust
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\n\n[dependencies]\nactix-web = \"4.0\"",
    )
    .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

    // Python
    fs::write(temp_dir.path().join("requirements.txt"), "django\nflask")
        .map_err(|e| format!("Failed to write requirements.txt: {}", e))?;

    // Go
    fs::write(
        temp_dir.path().join("go.mod"),
        "module example.com/app\n\nrequire github.com/gin-gonic/gin v1.9.0",
    )
    .map_err(|e| format!("Failed to write go.mod: {}", e))?;

    Ok(temp_dir)
}

/// Benchmark single language detection
fn benchmark_single_language_detection(
    c: &mut Criterion,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(config.detection)
        .build();

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

/// Benchmark polyglot project detection
fn benchmark_polyglot_detection(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(config.detection)
        .build();

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

/// Benchmark framework count scaling
fn benchmark_framework_count_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(config.detection)
        .build();

    // Create projects with different numbers of frameworks
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

/// Benchmark detection with different project sizes
fn benchmark_project_size_scaling(c: &mut Criterion) -> Result<(), Box<dyn std::error::Error>> {
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(config.detection)
        .build();

    let node_project = create_node_multi_framework_project()?;

    let mut group = c.benchmark_group("project_size_scaling");

    group.bench_function("small_project", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(node_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.finish();
    Ok(())
}

/// Benchmark detection performance with different configurations
fn benchmark_configuration_performance(
    c: &mut Criterion,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("Failed to generate template: {}", e))?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(config.detection)
        .build();

    let node_project = create_node_multi_framework_project()?;
    let rust_project = create_rust_multi_framework_project()?;

    let mut group = c.benchmark_group("configuration_performance");

    group.bench_function("full_config", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(node_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.bench_function("rust_full_config", |b| {
        b.iter(|| {
            if let Err(e) = engine.detect(rust_project.path()) {
                eprintln!("Detection failed: {}", e);
            }
        });
    });

    group.finish();
    Ok(())
}

// Wrapper functions for criterion
fn benchmark_single_language_detection_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_single_language_detection(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_polyglot_detection_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_polyglot_detection(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_framework_count_scaling_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_framework_count_scaling(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_project_size_scaling_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_project_size_scaling(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

fn benchmark_configuration_performance_wrapper(c: &mut Criterion) {
    if let Err(e) = benchmark_configuration_performance(c) {
        eprintln!("Benchmark failed: {}", e);
        std::process::exit(1);
    }
}

criterion_group!(
    benches,
    benchmark_single_language_detection_wrapper,
    benchmark_polyglot_detection_wrapper,
    benchmark_framework_count_scaling_wrapper,
    benchmark_project_size_scaling_wrapper,
    benchmark_configuration_performance_wrapper
);
criterion_main!(benches);
