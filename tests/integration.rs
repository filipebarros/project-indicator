mod common;

use common::{create_test_config, create_test_project};
use project_indicator::{
    detection::DetectionEngine,
    output::{render, OutputFormat},
    types::DisplayConfig,
};
use serde_json::Value;

#[test]
fn test_nextjs_priority_over_react() -> Result<(), Box<dyn std::error::Error>> {
    let package_json = r#"{
  "name": "nextjs-app",
  "dependencies": {
    "next": "^13.0.0",
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
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

    let mut names: Vec<&str> = result
        .frameworks
        .iter()
        .map(|m| m.framework.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["Next.js", "React"]);
    assert_eq!(result.frameworks[0].framework.name, "Next.js");

    let display_config = DisplayConfig::default();

    let simple = render(&result, OutputFormat::Simple, &display_config);
    assert!(simple.contains("▲"));
    if std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()) {
        assert!(simple.starts_with("\x1b[38;2;"), "icon should be colored");
    }
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
