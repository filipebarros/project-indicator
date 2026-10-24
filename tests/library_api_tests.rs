mod common;

use common::create_test_project;
use project_indicator::{
    detection::DetectionEngine,
    output::{render, OutputFormat},
    types::*,
};
use std::sync::Arc;

#[test]
fn test_detection_result_display() {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        root_indicators: vec![],
    };

    let language = Indicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![],
    );

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result_with_framework =
        DetectionResult::new(Some(Arc::new(language.clone())), vec![framework_match], 0.9);

    assert_eq!(result_with_framework.display_icon(), Some("⚛️"));
    assert_eq!(result_with_framework.display_color(), Some("#61DAFB"));

    let result_language_only = DetectionResult::new(Some(Arc::new(language)), vec![], 0.8);

    assert_eq!(result_language_only.display_icon(), Some("󰛦"));
    assert_eq!(result_language_only.display_color(), Some("#3178C6"));
}

#[test]
fn test_output_formatting() {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        root_indicators: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result = DetectionResult::new(None, vec![framework_match], 0.9);

    let simple = render(&result, OutputFormat::Simple, &DisplayConfig::default());
    assert_eq!(simple, "\x1b[38;2;97;218;251m⚛️\x1b[0m");

    let full = render(&result, OutputFormat::Full, &DisplayConfig::default());
    assert_eq!(full, "⚛️|#61DAFB");

    let json = render(&result, OutputFormat::Json, &DisplayConfig::default());
    assert!(json.contains("\"frameworks\":[\"React\"]"));
    assert!(json.contains("\"icon\":\"⚛️\""));
    assert!(json.contains("\"color\":\"#61DAFB\""));
}

#[test]
fn test_builtin_full_template_detects_frameworks_out_of_box(
) -> Result<(), Box<dyn std::error::Error>> {
    // The full template is the built-in fallback when no user config exists;
    // framework detection must work without running `config init`
    let config = project_indicator::config::TemplateGenerator::generate_template(Some("full"))
        .map_err(|e| format!("full template must exist: {e}"))?;

    let temp_dir = create_test_project(&[
        (
            "package.json",
            r#"{"name": "app", "dependencies": {"react": "^18.2.0"}}"#,
        ),
        ("tsconfig.json", "{}"),
        ("src/index.tsx", "export default null;"),
    ])?;

    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );
    let result = engine.detect(temp_dir.path())?;

    let language = result.indicator.as_ref().ok_or("expected a language")?;
    assert_eq!(language.name, "TypeScript");
    assert!(
        result
            .frameworks
            .iter()
            .any(|f| f.framework.name == "React"),
        "expected React framework, got: {:?}",
        result
            .frameworks
            .iter()
            .map(|f| &f.framework.name)
            .collect::<Vec<_>>()
    );
    Ok(())
}
