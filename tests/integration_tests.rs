mod common;

use common::create_test_project;
use project_indicator::{
    detection::DetectionEngineBuilder,
    output::{format_result, OutputFormat},
    types::*,
    Config,
};
use std::sync::Arc;

#[test]
fn test_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let language = Indicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string(), "tsconfig.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![Ecosystem::Npm],
    );

    let config = Config {
        meta: ConfigMeta::default(),
        display: DisplayConfig::default(),
        detection: DetectionConfig::default(),
        frameworks: vec![framework],
        indicators: vec![language],
    };

    let toml_str = toml::to_string(&config)?;
    assert!(toml_str.contains("TypeScript"));
    assert!(toml_str.contains("React"));

    let deserialized: Config = toml::from_str(&toml_str)?;
    assert_eq!(deserialized.indicators.len(), 1);
    assert_eq!(deserialized.indicators[0].name, "TypeScript");
    assert_eq!(deserialized.frameworks.len(), 1);
    assert_eq!(deserialized.frameworks[0].name, "React");
    Ok(())
}

#[test]
fn test_detection_types_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let detection_types = vec![
        DetectionType::Dependencies {
            dependencies: vec!["react".to_string(), "vue".to_string()],
        },
        DetectionType::Dependencies {
            dependencies: vec!["serde".to_string()],
        },
        DetectionType::Dependencies {
            dependencies: vec!["github.com/gin-gonic/gin".to_string()],
        },
        DetectionType::Dependencies {
            dependencies: vec!["Django".to_string()],
        },
        DetectionType::Dependencies {
            dependencies: vec!["rails".to_string()],
        },
        DetectionType::Dependencies {
            dependencies: vec!["laravel/framework".to_string()],
        },
        DetectionType::FileExists {
            files: vec!["next.config.js".to_string()],
        },
        DetectionType::ConfigFile {
            file: "pyproject.toml".to_string(),
            keys: vec!["tool.poetry".to_string()],
        },
    ];

    for detection_type in detection_types {
        let json = serde_json::to_string(&detection_type)?;
        let deserialized: DetectionType = serde_json::from_str(&json)?;
        assert_eq!(detection_type, deserialized);
    }
    Ok(())
}

#[test]
fn test_detection_result_display() -> Result<(), Box<dyn std::error::Error>> {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
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
    Ok(())
}

#[test]
fn test_output_formatting() -> Result<(), Box<dyn std::error::Error>> {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result = DetectionResult::new(None, vec![framework_match], 0.9);

    let simple = format_result(&result, OutputFormat::Simple);
    assert_eq!(simple, "⚛️");

    let full = format_result(&result, OutputFormat::Full);
    assert_eq!(full, "⚛️|#61DAFB");

    let json = format_result(&result, OutputFormat::Json);
    assert!(json.contains("\"frameworks\":[\"React\"]"));
    assert!(json.contains("\"icon\":\"⚛️\""));
    assert!(json.contains("\"color\":\"#61DAFB\""));
    Ok(())
}

#[test]
fn test_detection_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
    let language = Indicator::new(
        "Rust".to_string(),
        vec!["Cargo.toml".to_string()],
        "#DEA584".to_string(),
        "".to_string(),
        1,
        vec![],
    );

    let engine = DetectionEngineBuilder::new(vec![language], vec![]).build();

    let temp_dir = create_test_project(&[("Cargo.toml", "[package]\nname = \"test\"")])?;
    let result = engine.detect(temp_dir.path())?;

    assert!(!result.is_empty());
    assert_eq!(
        result
            .indicator
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );
    assert!(result.confidence > 0.0);
    Ok(())
}

#[test]
fn test_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();

    assert_eq!(config.meta.version, "3.0");
    assert!(config.display.show_frameworks);
    assert_eq!(config.display.max_frameworks, 2);
    assert_eq!(config.display.framework_separator, "+");
    assert!(config.indicators.is_empty());
    Ok(())
}

#[test]
fn test_wildcard_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let cpp_lang = Indicator::new(
        "C++".to_string(),
        vec![
            "*.cpp".to_string(),
            "*.hpp".to_string(),
            "CMakeLists.txt".to_string(),
        ],
        "#00599C".to_string(),
        "".to_string(),
        1,
        vec![],
    );

    let cpp_files = vec!["main.cpp".to_string(), "header.hpp".to_string()];
    assert!(cpp_lang.matches_files(&cpp_files));

    let cmake_files = vec!["CMakeLists.txt".to_string()];
    assert!(cpp_lang.matches_files(&cmake_files));

    let no_match_files = vec!["main.py".to_string(), "package.json".to_string()];
    assert!(!cpp_lang.matches_files(&no_match_files));

    let partial_files = vec!["src/main.cpp".to_string()];
    assert!(cpp_lang.matches_files(&partial_files));
    Ok(())
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

    let engine = DetectionEngineBuilder::new(config.indicators.clone(), config.frameworks.clone())
        .with_config(config.detection.clone())
        .build();
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

#[test]
fn test_framework_catalog_priority_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let mut frameworks = [
        Framework {
            name: "Library".to_string(),
            ecosystems: vec![Ecosystem::Npm],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 3,
            files: vec![],
            root_indicators: vec![],
        },
        Framework {
            name: "MetaFramework".to_string(),
            ecosystems: vec![Ecosystem::Npm],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        },
    ];

    frameworks.sort_by_key(|f| f.priority);
    assert_eq!(frameworks[0].name, "MetaFramework");
    assert_eq!(frameworks[1].name, "Library");
    Ok(())
}
