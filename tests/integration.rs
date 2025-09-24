use project_indicator::{
    config::Config,
    detection::DetectionEngine,
    output::{OutputFormat, OutputFormatter},
    types::{DetectionType, DisplayConfig, FrameworkDetector, ProjectIndicator},
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;
fn create_test_project(files: &[(&str, &str)]) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;

    for (path, content) in files {
        let file_path = temp_dir.path().join(path);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(file_path, content)?;
    }

    Ok(temp_dir)
}
fn create_test_config() -> Config {
    let typescript = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec!["tsconfig.json".to_string(), "*.ts".to_string()],
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
                priority: 1,
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
                priority: 0,
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
        vec!["*.py".to_string(), "requirements.txt".to_string()],
        "#3776AB".to_string(),
        "".to_string(),
        2,
        vec![FrameworkDetector {
            name: "Django".to_string(),
            detection: DetectionType::PythonEcosystem {
                dependencies: vec!["django".to_string()],
            },
            icon: Some("📝".to_string()),
            color: Some("#1E293B".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    Config::new(vec![typescript, rust, python])
}

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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_some());
    assert_eq!(
        result
            .language
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "TypeScript"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "React");

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "⚛️");

    let json_output = formatter.format(&result, OutputFormat::Json);
    let json: Value = serde_json::from_str(&json_output)?;
    assert_eq!(json["language"], "TypeScript");
    assert_eq!(json["frameworks"][0], "React");
    assert_eq!(json["icon"], "⚛️");
    assert_eq!(json["color"], "#61DAFB");

    let compact = formatter.format(&result, OutputFormat::Compact);
    assert_eq!(compact, "React+TS");
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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_some());
    assert_eq!(
        result
            .language
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Rocket");

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "🚀");

    let full = formatter.format(&result, OutputFormat::Full);
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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_some());
    assert_eq!(
        result
            .language
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Python"
    );

    assert!(!result.frameworks.is_empty());
    assert_eq!(result.frameworks[0].framework.name, "Django");

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let debug = formatter.format(&result, OutputFormat::Debug);
    assert!(debug.contains("Language: Python"));
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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.frameworks.len() >= 2);
    assert_eq!(result.frameworks[0].framework.name, "Next.js");

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "▲");
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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_some());
    assert_eq!(
        result
            .language
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );
    assert!(result.frameworks.is_empty());

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "");

    let compact = formatter.format(&result, OutputFormat::Compact);
    assert_eq!(compact, "RS");
    Ok(())
}

#[test]
fn test_no_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = create_test_project(&[
        ("README.md", "# Unknown Project"),
        ("data.txt", "some data"),
    ])?;

    let config = create_test_config();
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_none());
    assert!(result.frameworks.is_empty());
    assert_eq!(result.confidence, 0.0);

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert_eq!(simple, "");

    let json_output = formatter.format(&result, OutputFormat::Json);
    let json: Value = serde_json::from_str(&json_output)?;
    assert_eq!(json["language"], Value::Null);
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

    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    let formatter = OutputFormatter::new(display_config);

    let json_output = formatter.format(&result, OutputFormat::Json);
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
    let engine = DetectionEngine::new(config.languages.clone());
    let result = engine.detect(temp_dir.path())?;

    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);

    let simple = formatter.format(&result, OutputFormat::Simple);
    assert!(!simple.is_empty());

    let full = formatter.format(&result, OutputFormat::Full);
    assert!(full.contains("|"));
    assert!(!full.is_empty());

    let json_output = formatter.format(&result, OutputFormat::Json);
    assert!(serde_json::from_str::<Value>(&json_output).is_ok());

    let compact = formatter.format(&result, OutputFormat::Compact);
    assert!(!compact.is_empty());

    let debug = formatter.format(&result, OutputFormat::Debug);
    assert!(debug.contains("confidence"));
    Ok(())
}
