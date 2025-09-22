//! Performance benchmarks for project detection
//!
//! These benchmarks measure the performance of different aspects of the detection system
//! to ensure optimizations provide measurable improvements.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::{
    config::Config,
    detection::DetectionEngine,
    output::{OutputFormat, OutputFormatter},
    types::{DetectionType, DisplayConfig, FrameworkDetector, ProjectIndicator},
};
use std::fs;
use tempfile::TempDir;

/// Create a realistic test project structure
fn create_test_project(name: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    match name {
        "typescript_react" => {
            // Large TypeScript React project
            fs::write(
                base_path.join("package.json"),
                r#"{
  "name": "large-react-app",
  "dependencies": {
    "react": "^18.0.0",
    "react-dom": "^18.0.0",
    "typescript": "^4.9.0",
    "@types/react": "^18.0.0"
  },
  "devDependencies": {
    "vite": "^4.0.0",
    "eslint": "^8.0.0"
  }
}"#,
            )
            .unwrap();

            fs::write(
                base_path.join("tsconfig.json"),
                r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext"
  }
}"#,
            )
            .unwrap();

            // Create multiple source files
            fs::create_dir_all(base_path.join("src/components")).unwrap();
            fs::create_dir_all(base_path.join("src/hooks")).unwrap();
            fs::create_dir_all(base_path.join("src/utils")).unwrap();

            for i in 0..50 {
                fs::write(
                    base_path.join(format!("src/components/Component{}.tsx", i)),
                    "export const Component = () => <div>Hello</div>;",
                )
                .unwrap();
            }

            for i in 0..20 {
                fs::write(
                    base_path.join(format!("src/hooks/useHook{}.ts", i)),
                    "export const useHook = () => { return true; };",
                )
                .unwrap();
            }
        }
        "rust_rocket" => {
            // Rust project with Rocket
            fs::write(
                base_path.join("Cargo.toml"),
                r#"[package]
name = "rocket-app"
version = "0.1.0"
edition = "2021"

[dependencies]
rocket = "0.5"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
"#,
            )
            .unwrap();

            fs::create_dir_all(base_path.join("src/handlers")).unwrap();
            fs::create_dir_all(base_path.join("src/models")).unwrap();

            fs::write(base_path.join("src/main.rs"), "fn main() {}").unwrap();

            for i in 0..30 {
                fs::write(
                    base_path.join(format!("src/handlers/handler{}.rs", i)),
                    "pub fn handler() {}",
                )
                .unwrap();
            }
        }
        "python_django" => {
            // Python Django project
            fs::write(
                base_path.join("requirements.txt"),
                "Django>=4.0\npsycopg2>=2.8\ncelery>=5.0",
            )
            .unwrap();

            fs::write(base_path.join("manage.py"), "#!/usr/bin/env python").unwrap();

            fs::create_dir_all(base_path.join("myapp/models")).unwrap();
            fs::create_dir_all(base_path.join("myapp/views")).unwrap();

            for i in 0..25 {
                fs::write(
                    base_path.join(format!("myapp/models/model{}.py", i)),
                    "from django.db import models",
                )
                .unwrap();
            }
        }
        "go_gin" => {
            // Go project with Gin
            fs::write(
                base_path.join("go.mod"),
                r#"module gin-app

go 1.19

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/golang/protobuf v1.5.3
)
"#,
            )
            .unwrap();

            fs::create_dir_all(base_path.join("internal/handlers")).unwrap();
            fs::create_dir_all(base_path.join("internal/models")).unwrap();

            fs::write(base_path.join("main.go"), "package main").unwrap();

            for i in 0..20 {
                fs::write(
                    base_path.join(format!("internal/handlers/handler{}.go", i)),
                    "package handlers",
                )
                .unwrap();
            }
        }
        "mixed_large" => {
            // Large mixed project with multiple languages
            // JavaScript/TypeScript
            fs::write(base_path.join("package.json"), r#"{"name": "mixed"}"#).unwrap();
            fs::create_dir_all(base_path.join("frontend/src")).unwrap();
            for i in 0..100 {
                fs::write(
                    base_path.join(format!("frontend/src/file{}.js", i)),
                    "console.log('hello');",
                )
                .unwrap();
            }

            // Python
            fs::write(base_path.join("requirements.txt"), "flask>=2.0").unwrap();
            fs::create_dir_all(base_path.join("backend")).unwrap();
            for i in 0..50 {
                fs::write(
                    base_path.join(format!("backend/module{}.py", i)),
                    "print('hello')",
                )
                .unwrap();
            }

            // Go
            fs::write(base_path.join("go.mod"), "module mixed\ngo 1.19").unwrap();
            for i in 0..30 {
                fs::write(base_path.join(format!("service{}.go", i)), "package main").unwrap();
            }
        }
        _ => panic!("Unknown test project type: {}", name),
    }

    temp_dir
}

/// Create a comprehensive test configuration
fn create_benchmark_config() -> Config {
    let typescript = ProjectIndicator {
        name: "TypeScript".to_string(),
        files: vec![
            "tsconfig.json".to_string(),
            "*.ts".to_string(),
            "*.tsx".to_string(),
        ],
        color: "#3178C6".to_string(),
        icon: "󰛦".to_string(),
        priority: 1,
        frameworks: vec![
            FrameworkDetector {
                name: "React".to_string(),
                detection: DetectionType::PackageJson {
                    dependencies: vec!["react".to_string()],
                },
                icon: Some("⚛️".to_string()),
                color: Some("#61DAFB".to_string()),
                priority: 2,
                files: vec![],
            },
            FrameworkDetector {
                name: "Next.js".to_string(),
                detection: DetectionType::PackageJson {
                    dependencies: vec!["next".to_string()],
                },
                icon: Some("▲".to_string()),
                color: Some("#000000".to_string()),
                priority: 1,
                files: vec![],
            },
            FrameworkDetector {
                name: "Vue".to_string(),
                detection: DetectionType::PackageJson {
                    dependencies: vec!["vue".to_string()],
                },
                icon: Some("󰡄".to_string()),
                color: Some("#4FC08D".to_string()),
                priority: 2,
                files: vec![],
            },
        ],
    };

    let rust = ProjectIndicator {
        name: "Rust".to_string(),
        files: vec!["Cargo.toml".to_string(), "*.rs".to_string()],
        color: "#DEA584".to_string(),
        icon: "".to_string(),
        priority: 1,
        frameworks: vec![FrameworkDetector {
            name: "Rocket".to_string(),
            detection: DetectionType::CargoToml {
                dependencies: vec!["rocket".to_string()],
            },
            icon: Some("🚀".to_string()),
            color: Some("#D33847".to_string()),
            priority: 1,
            files: vec![],
        }],
    };

    let python = ProjectIndicator {
        name: "Python".to_string(),
        files: vec![
            "*.py".to_string(),
            "requirements.txt".to_string(),
            "setup.py".to_string(),
        ],
        color: "#3776AB".to_string(),
        icon: "".to_string(),
        priority: 2,
        frameworks: vec![FrameworkDetector {
            name: "Django".to_string(),
            detection: DetectionType::FileExists {
                files: vec!["manage.py".to_string()],
            },
            icon: Some("󰪽".to_string()),
            color: Some("#092E20".to_string()),
            priority: 1,
            files: vec![],
        }],
    };

    let go = ProjectIndicator {
        name: "Go".to_string(),
        files: vec!["go.mod".to_string(), "*.go".to_string()],
        color: "#00ADD8".to_string(),
        icon: "".to_string(),
        priority: 1,
        frameworks: vec![FrameworkDetector {
            name: "Gin".to_string(),
            detection: DetectionType::GoMod {
                modules: vec!["github.com/gin-gonic/gin".to_string()],
            },
            icon: Some("🍸".to_string()),
            color: Some("#00ADD8".to_string()),
            priority: 1,
            files: vec![],
        }],
    };

    let javascript = ProjectIndicator {
        name: "JavaScript".to_string(),
        files: vec![
            "package.json".to_string(),
            "*.js".to_string(),
            "*.jsx".to_string(),
        ],
        color: "#F7DF1E".to_string(),
        icon: "".to_string(),
        priority: 2,
        frameworks: vec![],
    };

    Config::new(vec![typescript, rust, python, go, javascript])
}

/// Benchmark basic project detection
fn bench_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());

    let projects = [
        ("typescript_react", create_test_project("typescript_react")),
        ("rust_rocket", create_test_project("rust_rocket")),
        ("python_django", create_test_project("python_django")),
        ("go_gin", create_test_project("go_gin")),
        ("mixed_large", create_test_project("mixed_large")),
    ];

    for (name, project) in &projects {
        group.bench_with_input(
            BenchmarkId::new("detect_project", name),
            &project.path(),
            |b, path| b.iter(|| engine.detect(black_box(path)).unwrap()),
        );
    }

    group.finish();
}

/// Benchmark file scanning performance
fn bench_file_scanning(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_scanning");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());

    let large_project = create_test_project("mixed_large");

    group.bench_function("scan_large_project", |b| {
        b.iter(|| {
            // This will call the internal file scanning method
            engine.detect(black_box(large_project.path())).unwrap()
        })
    });

    group.finish();
}

/// Benchmark output formatting performance
fn bench_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatting");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_test_project("typescript_react");
    let result = engine.detect(project.path()).unwrap();

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    let formats = [
        OutputFormat::Simple,
        OutputFormat::Full,
        OutputFormat::Json,
        OutputFormat::Compact,
        OutputFormat::Debug,
    ];

    for format in &formats {
        group.bench_with_input(
            BenchmarkId::new("format_output", format!("{:?}", format)),
            format,
            |b, fmt| b.iter(|| formatter.format(black_box(&result), black_box(fmt.clone()))),
        );
    }

    group.finish();
}

/// Benchmark configuration loading
fn bench_config_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("config");

    group.bench_function("create_config", |b| {
        b.iter(|| black_box(create_benchmark_config()))
    });

    group.bench_function("create_detection_engine", |b| {
        let config = create_benchmark_config();
        b.iter(|| DetectionEngine::new(black_box(config.languages.clone())))
    });

    group.finish();
}

/// Benchmark repeated detection (simulating shell prompt usage)
fn bench_repeated_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_detection");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_test_project("typescript_react");

    // Simulate checking the same project multiple times (like in shell prompt)
    group.bench_function("same_project_10x", |b| {
        b.iter(|| {
            for _ in 0..10 {
                engine.detect(black_box(project.path())).unwrap();
            }
        })
    });

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    let config = create_benchmark_config();

    group.bench_function("engine_creation_overhead", |b| {
        b.iter(|| {
            // Create and immediately drop to measure creation cost
            let engine = DetectionEngine::new(black_box(config.languages.clone()));
            drop(engine);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_detection,
    bench_file_scanning,
    bench_formatting,
    bench_config_loading,
    bench_repeated_detection,
    bench_memory_patterns
);
criterion_main!(benches);
