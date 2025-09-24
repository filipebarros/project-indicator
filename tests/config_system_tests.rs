use project_indicator::types::{DetectionConfig, IndicatorContext, RootIndicator};
use project_indicator::{Config, DetectionEngine};
use std::fs;
use tempfile::TempDir;
#[test]
fn test_detection_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let config = DetectionConfig::default();

    assert_eq!(config.max_upward_traversal, 10);
    assert!(!config.require_vcs_root);
    assert_eq!(config.confidence_threshold, 0.3);
    assert!(config.root_indicators.is_empty());
    Ok(())
}
#[test]
fn test_custom_root_indicators() -> Result<(), Box<dyn std::error::Error>> {
    let custom_indicators = vec![
        RootIndicator {
            pattern: "my-project.toml".to_string(),
            weight: 0.8,
            context: IndicatorContext::LanguageRoot,
        },
        RootIndicator {
            pattern: ".custom".to_string(),
            weight: 0.6,
            context: IndicatorContext::Configuration,
        },
    ];

    let config = DetectionConfig {
        max_upward_traversal: 5,
        require_vcs_root: false,
        confidence_threshold: 0.5,
        root_indicators: custom_indicators.clone(),
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    assert_eq!(config.root_indicators.len(), 2);
    assert_eq!(config.root_indicators[0].pattern, "my-project.toml");
    assert_eq!(config.root_indicators[0].weight, 0.8);
    assert_eq!(config.confidence_threshold, 0.5);
    assert_eq!(config.max_upward_traversal, 5);
    Ok(())
}
#[test]
fn test_detection_engine_with_custom_config() -> Result<(), Box<dyn std::error::Error>> {
    let detection_config = DetectionConfig {
        max_upward_traversal: 3,
        require_vcs_root: false,
        confidence_threshold: 0.8,
        root_indicators: vec![],
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    let engine = DetectionEngine::with_config(vec![], detection_config);

    let temp_dir = TempDir::new()?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.is_empty());
    Ok(())
}
#[test]
fn test_custom_root_indicators_replace_builtin() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use tempfile::TempDir;

    let detection_config = DetectionConfig {
        max_upward_traversal: 10,
        require_vcs_root: false,
        confidence_threshold: 0.3,
        root_indicators: vec![RootIndicator {
            pattern: "custom-project.toml".to_string(),
            weight: 1.0,
            context: IndicatorContext::LanguageRoot,
        }],
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    let engine = DetectionEngine::with_config(vec![], detection_config);

    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path();

    fs::write(
        project_path.join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;
    fs::write(project_path.join("package.json"), "{\"name\": \"test\"}")?;

    fs::write(
        project_path.join("custom-project.toml"),
        "[project]\nname = \"test\"",
    )?;

    let sub_dir = project_path.join("src");
    fs::create_dir_all(&sub_dir)?;

    let result_custom = engine.detect(&sub_dir)?;

    let default_config = DetectionConfig::default();
    let default_engine = DetectionEngine::with_config(vec![], default_config);

    fs::remove_file(project_path.join("custom-project.toml"))?;

    let result_no_indicators = default_engine.detect(&sub_dir)?;

    assert!(result_custom.is_empty());
    assert!(result_no_indicators.is_empty());

    Ok(())
}
#[test]
fn test_detection_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let original_config = DetectionConfig {
        max_upward_traversal: 8,
        require_vcs_root: true,
        confidence_threshold: 0.4,
        root_indicators: vec![RootIndicator {
            pattern: "project.yaml".to_string(),
            weight: 0.7,
            context: IndicatorContext::LanguageRoot,
        }],
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    let toml_str = toml::to_string(&original_config)?;
    assert!(toml_str.contains("max_upward_traversal = 8"));
    assert!(toml_str.contains("require_vcs_root = true"));
    assert!(toml_str.contains("confidence_threshold = 0.4"));
    assert!(toml_str.contains("project.yaml"));

    let deserialized_config: DetectionConfig = toml::from_str(&toml_str)?;
    assert_eq!(
        deserialized_config.max_upward_traversal,
        original_config.max_upward_traversal
    );
    assert_eq!(
        deserialized_config.require_vcs_root,
        original_config.require_vcs_root
    );
    assert_eq!(
        deserialized_config.confidence_threshold,
        original_config.confidence_threshold
    );
    assert_eq!(deserialized_config.root_indicators.len(), 1);
    assert_eq!(
        deserialized_config.root_indicators[0].pattern,
        "project.yaml"
    );
    assert_eq!(deserialized_config.root_indicators[0].weight, 0.7);
    Ok(())
}
#[test]
fn test_main_config_includes_detection() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new(vec![]);

    assert_eq!(config.detection.max_upward_traversal, 10);
    assert_eq!(config.detection.confidence_threshold, 0.3);
    Ok(())
}
#[test]
fn test_full_config_with_detection_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new(vec![]);

    let toml_str = toml::to_string(&config)?;

    assert!(toml_str.contains("[detection]"));
    assert!(toml_str.contains("max_upward_traversal"));
    assert!(toml_str.contains("confidence_threshold"));

    let deserialized_config: Config = toml::from_str(&toml_str)?;
    assert_eq!(
        deserialized_config.detection.max_upward_traversal,
        config.detection.max_upward_traversal
    );
    assert_eq!(
        deserialized_config.detection.confidence_threshold,
        config.detection.confidence_threshold
    );
    Ok(())
}
#[test]
fn test_config_file_loading_with_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");

    let config_content = "[meta]
version = \"2.0\"

[display]
show_frameworks = true
max_frameworks = 3
framework_separator = \" | \"

[cache]
enabled = true
max_entries = 500
ttl_seconds = 600

[detection]
max_upward_traversal = 5
require_vcs_root = true
confidence_threshold = 0.6

[[detection.root_indicators]]
pattern = \"build.sbt\"
weight = 0.9

[[detection.root_indicators]]
pattern = \"project.clj\"
weight = 0.85

[[languages]]
name = \"Test Language\"
files = [\"test.file\"]
color = \"#FF0000\"
icon = \"🧪\"
priority = 1
";

    fs::write(&config_file, config_content)?;

    let config = Config::load_from_file(&config_file)?;

    assert_eq!(config.detection.max_upward_traversal, 5);
    assert!(config.detection.require_vcs_root);
    assert_eq!(config.detection.confidence_threshold, 0.6);
    assert_eq!(config.detection.root_indicators.len(), 2);
    assert_eq!(config.detection.root_indicators[0].pattern, "build.sbt");
    assert_eq!(config.detection.root_indicators[0].weight, 0.9);
    assert_eq!(config.detection.root_indicators[1].pattern, "project.clj");
    assert_eq!(config.detection.root_indicators[1].weight, 0.85);

    assert_eq!(config.display.max_frameworks, 3);
    assert_eq!(config.display.framework_separator, " | ");
    assert_eq!(config.cache.max_entries, 500);
    assert_eq!(config.cache.ttl_seconds, 600);
    assert_eq!(config.languages.len(), 1);
    assert_eq!(config.languages[0].name, "Test Language");

    Ok(())
}
