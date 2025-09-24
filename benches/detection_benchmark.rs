#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use project_indicator::{
    config::Config,
    detection::DetectionEngine,
    output::{OutputFormat, OutputFormatter},
    types::{DetectionType, DisplayConfig, FrameworkDetector, ProjectIndicator},
};
use std::fs;
use std::hint::black_box;
use tempfile::TempDir;
fn create_test_project(name: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();

    match name {
        "typescript_react" => {
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
            )?;

            fs::write(
                base_path.join("tsconfig.json"),
                r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext"
  }
}"#,
            )?;

            fs::create_dir_all(base_path.join("src/components"))?;
            fs::create_dir_all(base_path.join("src/hooks"))?;
            fs::create_dir_all(base_path.join("src/utils"))?;

            for i in 0..50 {
                fs::write(
                    base_path.join(format!("src/components/Component{}.tsx", i)),
                    "export const Component = () => <div>Hello</div>;",
                )?;
            }

            for i in 0..20 {
                fs::write(
                    base_path.join(format!("src/hooks/useHook{}.ts", i)),
                    "export const useHook = () => { return true; };",
                )?;
            }
        }
        "rust_rocket" => {
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
            )?;

            fs::create_dir_all(base_path.join("src/handlers"))?;
            fs::create_dir_all(base_path.join("src/models"))?;

            fs::write(base_path.join("src/main.rs"), "fn main() {}")?;

            for i in 0..30 {
                fs::write(
                    base_path.join(format!("src/handlers/handler{}.rs", i)),
                    "pub fn handler() {}",
                )?;
            }
        }
        "python_django" => {
            fs::write(
                base_path.join("requirements.txt"),
                "Django>=4.0\npsycopg2>=2.8\ncelery>=5.0",
            )?;

            fs::write(base_path.join("manage.py"), "#!/usr/bin/env python")?;

            fs::create_dir_all(base_path.join("myapp/models"))?;
            fs::create_dir_all(base_path.join("myapp/views"))?;

            for i in 0..25 {
                fs::write(
                    base_path.join(format!("myapp/models/model{}.py", i)),
                    "from django.db import models",
                )?;
            }
        }
        "go_gin" => {
            fs::write(
                base_path.join("go.mod"),
                r#"module gin-app

go 1.19

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/golang/protobuf v1.5.3
)
"#,
            )?;

            fs::create_dir_all(base_path.join("internal/handlers"))?;
            fs::create_dir_all(base_path.join("internal/models"))?;

            fs::write(base_path.join("main.go"), "package main")?;

            for i in 0..20 {
                fs::write(
                    base_path.join(format!("internal/handlers/handler{}.go", i)),
                    "package handlers",
                )?;
            }
        }
        "mixed_large" => {
            fs::write(base_path.join("package.json"), r#"{"name": "mixed"}"#)?;
            fs::create_dir_all(base_path.join("frontend/src"))?;
            for i in 0..100 {
                fs::write(
                    base_path.join(format!("frontend/src/file{}.js", i)),
                    "console.log('hello');",
                )?;
            }

            fs::write(base_path.join("requirements.txt"), "flask>=2.0")?;
            fs::create_dir_all(base_path.join("backend"))?;
            for i in 0..50 {
                fs::write(
                    base_path.join(format!("backend/module{}.py", i)),
                    "print('hello')",
                )?;
            }

            fs::write(base_path.join("go.mod"), "module mixed\ngo 1.19")?;
            for i in 0..30 {
                fs::write(base_path.join(format!("service{}.go", i)), "package main")?;
            }
        }
        _ => panic!("Unknown test project type: {}", name),
    }

    Ok(temp_dir)
}
fn create_benchmark_config() -> Config {
    let typescript = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec![
            "tsconfig.json".to_string(),
            "*.ts".to_string(),
            "*.tsx".to_string(),
        ],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![
            FrameworkDetector {
                name: "React".to_string(),
                detection: DetectionType::NodeEcosystem {
                    dependencies: vec!["react".to_string()],
                },
                icon: Some("⚛️".to_string()),
                color: Some("#61DAFB".to_string()),
                priority: 2,
                files: vec![],
                root_indicators: vec![],
            },
            FrameworkDetector {
                name: "Next.js".to_string(),
                detection: DetectionType::NodeEcosystem {
                    dependencies: vec!["next".to_string()],
                },
                icon: Some("▲".to_string()),
                color: Some("#000000".to_string()),
                priority: 1,
                files: vec![],
                root_indicators: vec![],
            },
            FrameworkDetector {
                name: "Vue".to_string(),
                detection: DetectionType::NodeEcosystem {
                    dependencies: vec!["vue".to_string()],
                },
                icon: Some("󰡄".to_string()),
                color: Some("#4FC08D".to_string()),
                priority: 2,
                files: vec![],
                root_indicators: vec![],
            },
        ],
    );

    let rust = ProjectIndicator::new(
        "Rust".to_string(),
        vec!["Cargo.toml".to_string(), "*.rs".to_string()],
        "#DEA584".to_string(),
        "".to_string(),
        1,
        vec![FrameworkDetector {
            name: "Rocket".to_string(),
            detection: DetectionType::RustEcosystem {
                dependencies: vec!["rocket".to_string()],
            },
            icon: Some("🚀".to_string()),
            color: Some("#D33847".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    let python = ProjectIndicator::new(
        "Python".to_string(),
        vec![
            "*.py".to_string(),
            "requirements.txt".to_string(),
            "setup.py".to_string(),
        ],
        "#3776AB".to_string(),
        "".to_string(),
        2,
        vec![FrameworkDetector {
            name: "Django".to_string(),
            detection: DetectionType::FileExists {
                files: vec!["manage.py".to_string()],
            },
            icon: Some("󰪽".to_string()),
            color: Some("#092E20".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    let go = ProjectIndicator::new(
        "Go".to_string(),
        vec!["go.mod".to_string(), "*.go".to_string()],
        "#00ADD8".to_string(),
        "".to_string(),
        1,
        vec![FrameworkDetector {
            name: "Gin".to_string(),
            detection: DetectionType::GoEcosystem {
                modules: vec!["github.com/gin-gonic/gin".to_string()],
            },
            icon: Some("🍸".to_string()),
            color: Some("#00ADD8".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    let javascript = ProjectIndicator::new(
        "JavaScript".to_string(),
        vec![
            "package.json".to_string(),
            "*.js".to_string(),
            "*.jsx".to_string(),
        ],
        "#F7DF1E".to_string(),
        "".to_string(),
        2,
        vec![],
    );

    Config::new(vec![typescript, rust, python, go, javascript])
}
fn bench_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());

    let projects = [
        (
            "typescript_react",
            create_test_project("typescript_react").unwrap(),
        ),
        ("rust_rocket", create_test_project("rust_rocket").unwrap()),
        (
            "python_django",
            create_test_project("python_django").unwrap(),
        ),
        ("go_gin", create_test_project("go_gin").unwrap()),
        ("mixed_large", create_test_project("mixed_large").unwrap()),
    ];

    for (name, project) in &projects {
        group.bench_with_input(
            BenchmarkId::new("detect_project", name),
            &project.path(),
            |b, path| b.iter(|| engine.detect(black_box(path))),
        );
    }

    group.finish();
}
fn bench_file_scanning(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_scanning");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());

    let large_project = create_test_project("mixed_large").unwrap();

    group.bench_function("scan_large_project", |b| {
        b.iter(|| engine.detect(black_box(large_project.path())))
    });

    group.finish();
}
fn bench_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("formatting");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_test_project("typescript_react").unwrap();
    let result = engine.detect(project.path()).unwrap();

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

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
fn bench_repeated_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_detection");

    let config = create_benchmark_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let project = create_test_project("typescript_react").unwrap();

    group.bench_function("same_project_10x", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let _ = engine.detect(black_box(project.path()));
            }
        })
    });

    group.finish();
}
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    let config = create_benchmark_config();

    group.bench_function("engine_creation_overhead", |b| {
        b.iter(|| {
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
