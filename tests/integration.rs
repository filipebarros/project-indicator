mod common;

use common::{create_test_config, create_test_project};
use project_indicator::{
    detection::DetectionEngine,
    output::{render, OutputFormat},
    types::DisplayConfig,
};
use serde_json::Value;

#[test]
fn test_typescript_react_detection() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.indicator.is_some());
    assert_eq!(
        result
            .indicator
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "TypeScript"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "React");

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert!(simple.contains("⚛️"));
    assert!(simple.starts_with("\x1b[38;2;"), "icon should be colored");

    let json_output = render(&result, OutputFormat::Json, &display_config);
    let json: Value = serde_json::from_str(&json_output)?;
    assert_eq!(json["indicator"], "TypeScript");
    assert_eq!(json["frameworks"][0], "React");
    assert_eq!(json["icon"], "⚛️");
    assert_eq!(json["color"], "#61DAFB");
    Ok(())
}

#[test]
fn test_rust_rocket_detection() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.indicator.is_some());
    assert_eq!(
        result
            .indicator
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Rocket");

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert!(simple.contains("🚀"));
    assert!(simple.starts_with("\x1b[38;2;"), "icon should be colored");

    let full = render(&result, OutputFormat::Full, &display_config);
    assert!(full.contains("🚀"));
    assert!(full.contains("#D33847"));
    assert!(full.contains("|"));
    assert!(full.contains("•"));
    Ok(())
}

#[test]
fn test_python_django_detection() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.indicator.is_some());
    assert_eq!(
        result
            .indicator
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Python"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Django");

    let display_config = DisplayConfig::default();

    let debug = render(&result, OutputFormat::Debug, &display_config);
    assert!(debug.contains("Project: Python"));
    assert!(debug.contains("Django"));
    assert!(debug.contains("confidence"));
    Ok(())
}

#[test]
fn test_nextjs_priority_over_react() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.frameworks.len() >= 2);
    assert_eq!(result.frameworks[0].framework.name, "Next.js");

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert!(simple.contains("▲"));
    assert!(simple.starts_with("\x1b[38;2;"), "icon should be colored");
    Ok(())
}

#[test]
fn test_language_only_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_project(&[
        (
            "Cargo.toml",
            "[package]\nname = \"simple-rust\"\nversion = \"0.1.0\"",
        ),
        ("src/main.rs", "fn main() {}"),
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.indicator.is_some());
    assert_eq!(
        result
            .indicator
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );
    assert!(result.frameworks.is_empty());

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert_eq!(simple, "");
    Ok(())
}

#[test]
fn test_no_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_project(&[
        ("README.md", "# Unknown Project"),
        ("data.txt", "some data"),
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    assert!(result.indicator.is_none());
    assert!(result.frameworks.is_empty());
    assert_eq!(result.confidence, 0.0);

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert_eq!(simple, "");

    let json_output = render(&result, OutputFormat::Json, &display_config);
    let json: Value = serde_json::from_str(&json_output)?;
    assert_eq!(json["indicator"], Value::Null);
    assert_eq!(json["frameworks"], Value::Array(vec![]));
    assert_eq!(json["confidence"], 0.0);
    Ok(())
}

#[test]
fn test_framework_limiting() -> Result<(), Box<dyn std::error::Error>> {
    let package_json = r#"{
  "name": "multi-framework",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^13.0.0"
  }
}"#;

    let temp_dir = create_test_project(&[("package.json", package_json), ("tsconfig.json", "{}")])?;

    let config = create_test_config();
    let display_config = DisplayConfig {
        max_frameworks: 1,
        ..Default::default()
    };

    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    let json_output = render(&result, OutputFormat::Json, &display_config);
    let json: Value = serde_json::from_str(&json_output)?;

    assert_eq!(
        json["frameworks"]
            .as_array()
            .ok_or("Failed to get frameworks array")?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn test_all_output_formats() -> Result<(), Box<dyn std::error::Error>> {
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
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.indicators, config.frameworks);
    let result = engine.detect(temp_dir.path())?;

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert!(simple.contains("\x1b["), "one colored glyph: {simple:?}");

    let full = render(&result, OutputFormat::Full, &display_config);
    assert!(full.contains('|'), "icon|color components: {full:?}");

    let json_output = render(&result, OutputFormat::Json, &display_config);
    let json: Value = serde_json::from_str(&json_output)?;
    assert_eq!(json["indicator"], "TypeScript");
    assert!(json["frameworks"].as_array().is_some_and(|f| !f.is_empty()));

    let debug = render(&result, OutputFormat::Debug, &display_config);
    assert!(debug.contains("confidence"));
    Ok(())
}
