//! End-to-end integration tests for project-indicator
//!
//! These tests verify the complete pipeline from configuration loading
//! through detection to output formatting.

use project_indicator::{
    config::Config,
    detection::DetectionEngine,
    output::{OutputFormat, OutputFormatter},
    types::{DetectionType, DisplayConfig, FrameworkDetector, ProjectIndicator},
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

/// Create a test project structure with specific files
fn create_test_project(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    for (path, content) in files {
        let file_path = temp_dir.path().join(path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(file_path, content).unwrap();
    }

    temp_dir
}

/// Create a minimal test configuration
fn create_test_config() -> Config {
    let typescript = ProjectIndicator {
        name: "TypeScript".to_string(),
        files: vec!["tsconfig.json".to_string(), "*.ts".to_string()],
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
                priority: 1,
                files: vec![],
            },
            FrameworkDetector {
                name: "Next.js".to_string(),
                detection: DetectionType::PackageJson {
                    dependencies: vec!["next".to_string()],
                },
                icon: Some("▲".to_string()),
                color: Some("#000000".to_string()),
                priority: 0, // Higher priority than React
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
        files: vec!["*.py".to_string(), "requirements.txt".to_string()],
        color: "#3776AB".to_string(),
        icon: "".to_string(),
        priority: 2,
        frameworks: vec![FrameworkDetector {
            name: "Django".to_string(),
            detection: DetectionType::PyProjectToml {
                dependencies: vec!["django".to_string()],
            },
            icon: Some("📝".to_string()),
            color: Some("#1E293B".to_string()),
            priority: 1,
            files: vec![],
        }],
    };

    Config::new(vec![typescript, rust, python])
}

#[test]
fn test_typescript_react_detection() {
    let package_json = r#"{
  "name": "test-app",
  "dependencies": {
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  },
  "devDependencies": {
    "@types/react": "^18.0.0",
    "typescript": "^4.9.0"
  }
}"#;

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "es5",
    "module": "commonjs"
  }
}"#;

    let temp_dir = create_test_project(&[
        ("package.json", package_json),
        ("tsconfig.json", tsconfig),
        ("src/App.tsx", "export const App = () => <div>Hello</div>;"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect TypeScript as language
    assert!(result.language.is_some());
    assert_eq!(result.language.as_ref().unwrap().name, "TypeScript");

    // Should detect React framework
    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "React");

    // Test different output formats
    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Simple format should show React icon
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "⚛️");

    // JSON format should contain all information
    let json_output = formatter.format(&result, OutputFormat::Json);
    let json: Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(json["language"], "TypeScript");
    assert_eq!(json["frameworks"][0], "React");
    assert_eq!(json["icon"], "⚛️");
    assert_eq!(json["color"], "#61DAFB");

    // Compact format should show React+TS
    let compact = formatter.format(&result, OutputFormat::Compact);
    assert_eq!(compact, "React+TS");
}

#[test]
fn test_rust_rocket_detection() {
    let cargo_toml = r#"[package]
name = "rocket-app"
version = "0.1.0"
edition = "2021"

[dependencies]
rocket = "0.5"
serde = { version = "1.0", features = ["derive"] }
"#;

    let temp_dir = create_test_project(&[
        ("Cargo.toml", cargo_toml),
        ("src/main.rs", "fn main() { println!(\"Hello, world!\"); }"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect Rust as language
    assert!(result.language.is_some());
    assert_eq!(result.language.as_ref().unwrap().name, "Rust");

    // Should detect Rocket framework
    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Rocket");

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Simple format should show Rocket icon
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "🚀");

    // Full format should include framework info
    let full = formatter.format(&result, OutputFormat::Full);
    assert!(full.contains("🚀"));
    assert!(full.contains("Rocket"));
    assert!(full.contains("#D33847"));
}

#[test]
fn test_python_django_detection() {
    let pyproject_toml = r#"[project]
name = "python-app"
version = "0.1.0"
description = ""
dependencies = [
    "django>=4.0",
    "requests",
]

[build-system]
requires = ["setuptools", "wheel"]
build-backend = "setuptools.build_meta"
"#;

    let temp_dir = create_test_project(&[
        ("pyproject.toml", pyproject_toml),
        ("main.py", "print('Hello, world!')"),
        ("app/__init__.py", ""),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect Python as language
    assert!(result.language.is_some());
    assert_eq!(result.language.as_ref().unwrap().name, "Python");

    // Should detect Django framework
    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Django");

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Debug format should show detailed information
    let debug = formatter.format(&result, OutputFormat::Debug);
    assert!(debug.contains("Language: Python"));
    assert!(debug.contains("Django"));
    assert!(debug.contains("confidence"));
}

#[test]
fn test_nextjs_priority_over_react() {
    let package_json = r#"{
  "name": "nextjs-app",
  "dependencies": {
    "next": "^13.0.0",
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  }
}"#;

    let temp_dir = create_test_project(&[
        ("package.json", package_json),
        ("next.config.js", "module.exports = {}"),
        (
            "pages/index.tsx",
            "export default function Home() { return <div>Hello</div>; }",
        ),
        ("tsconfig.json", "{}"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect both Next.js and React, but Next.js should come first due to higher priority
    assert!(result.frameworks.len() >= 2);
    assert_eq!(result.frameworks[0].framework.name, "Next.js");

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Simple format should show Next.js icon (higher priority)
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "▲");
}

#[test]
fn test_language_only_detection() {
    let temp_dir = create_test_project(&[
        (
            "Cargo.toml",
            "[package]\nname = \"simple-rust\"\nversion = \"0.1.0\"",
        ),
        ("src/main.rs", "fn main() {}"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect Rust but no frameworks
    assert!(result.language.is_some());
    assert_eq!(result.language.as_ref().unwrap().name, "Rust");
    assert!(result.frameworks.is_empty());

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Should fall back to language icon
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "");

    // Compact format should just show language
    let compact = formatter.format(&result, OutputFormat::Compact);
    assert_eq!(compact, "RS");
}

#[test]
fn test_no_detection() {
    let temp_dir = create_test_project(&[
        ("README.md", "# Unknown Project"),
        ("data.txt", "some data"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    // Should detect nothing
    assert!(result.language.is_none());
    assert!(result.frameworks.is_empty());
    assert_eq!(result.confidence, 0.0);

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // All formats should handle empty results gracefully
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "");

    let json_output = formatter.format(&result, OutputFormat::Json);
    let json: Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(json["language"], Value::Null);
    assert_eq!(json["frameworks"], Value::Array(vec![]));
    assert_eq!(json["confidence"], 0.0);
}

#[test]
fn test_framework_limiting() {
    // Create a project with multiple frameworks that should be detected
    let package_json = r#"{
  "name": "multi-framework",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^13.0.0"
  }
}"#;

    let temp_dir = create_test_project(&[("package.json", package_json), ("tsconfig.json", "{}")]);

    let config = create_test_config();
    let display_config = DisplayConfig {
        max_frameworks: 1, // Limit to 1 framework
        ..Default::default()
    };

    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    let json_output = formatter.format(&result, OutputFormat::Json);
    let json: Value = serde_json::from_str(&json_output).unwrap();

    // Should only show 1 framework despite multiple being detected
    assert_eq!(json["frameworks"].as_array().unwrap().len(), 1);
}

#[test]
fn test_all_output_formats() {
    let package_json = r#"{
  "name": "format-test",
  "dependencies": {
    "react": "^18.0.0"
  }
}"#;

    let temp_dir = create_test_project(&[
        ("package.json", package_json),
        ("tsconfig.json", "{}"),
        ("src/App.tsx", "export const App = () => <div>Test</div>;"),
    ]);

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path()).unwrap();

    let display_config = DisplayConfig::default();
    let theme = Box::new(project_indicator::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);

    // Test all output formats work without errors
    let simple = formatter.format(&result, OutputFormat::Simple);
    assert!(!simple.is_empty());

    let full = formatter.format(&result, OutputFormat::Full);
    assert!(full.contains("React"));

    let json_output = formatter.format(&result, OutputFormat::Json);
    assert!(serde_json::from_str::<Value>(&json_output).is_ok());

    let compact = formatter.format(&result, OutputFormat::Compact);
    assert!(!compact.is_empty());

    let debug = formatter.format(&result, OutputFormat::Debug);
    assert!(debug.contains("confidence"));
}
