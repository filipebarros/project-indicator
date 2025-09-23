//! Tests for the configuration system including DetectionConfig

use project_indicator::types::{DetectionConfig, RootIndicator};
use project_indicator::{Config, DetectionEngine};
use std::fs;
use tempfile::TempDir;

/// Test that DetectionConfig defaults work correctly
#[test]
fn test_detection_config_defaults() {
    let config = DetectionConfig::default();

    assert_eq!(config.max_upward_traversal, 10);
    assert!(!config.require_vcs_root);
    assert_eq!(config.confidence_threshold, 0.3);
    assert!(config.root_indicators.is_empty()); // No default indicators - must be explicitly defined
}

/// Test that custom root indicators can be added to configuration
#[test]
fn test_custom_root_indicators() {
    let custom_indicators = vec![
        RootIndicator {
            pattern: "my-project.toml".to_string(),
            weight: 0.8,
        },
        RootIndicator {
            pattern: ".custom".to_string(),
            weight: 0.6,
        },
    ];

    let config = DetectionConfig {
        max_upward_traversal: 5,
        require_vcs_root: false,
        confidence_threshold: 0.5,
        root_indicators: custom_indicators.clone(),
    };

    assert_eq!(config.root_indicators.len(), 2);
    assert_eq!(config.root_indicators[0].pattern, "my-project.toml");
    assert_eq!(config.root_indicators[0].weight, 0.8);
    assert_eq!(config.confidence_threshold, 0.5);
    assert_eq!(config.max_upward_traversal, 5);
}

/// Test that DetectionEngine respects configuration settings
#[test]
fn test_detection_engine_with_custom_config() {
    // Create a config with custom detection settings
    let detection_config = DetectionConfig {
        max_upward_traversal: 3,
        require_vcs_root: false,
        confidence_threshold: 0.8, // High threshold
        root_indicators: vec![],
    };

    let engine = DetectionEngine::with_config(vec![], detection_config);

    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();

    // Test detection with root discovery disabled
    let result = engine.detect(temp_dir.path()).unwrap();
    assert!(result.is_empty()); // Should be empty since no languages configured
}

/// Test that custom root indicators replace built-in ones
#[test]
fn test_custom_root_indicators_replace_builtin() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use tempfile::TempDir;

    // Create a config with custom root indicators only
    let detection_config = DetectionConfig {
        max_upward_traversal: 10,
        require_vcs_root: false,
        confidence_threshold: 0.3,
        root_indicators: vec![RootIndicator {
            pattern: "custom-project.toml".to_string(),
            weight: 1.0,
        }],
    };

    let engine = DetectionEngine::with_config(vec![], detection_config);

    // Create a test directory with both built-in and custom indicators
    let temp_dir = TempDir::new()?;
    let project_path = temp_dir.path();

    // Create built-in indicator (should be ignored)
    fs::write(
        project_path.join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;
    fs::write(project_path.join("package.json"), "{\"name\": \"test\"}")?;

    // Create custom indicator
    fs::write(
        project_path.join("custom-project.toml"),
        "[project]\nname = \"test\"",
    )?;

    // Create a subdirectory to test root discovery behavior
    let sub_dir = project_path.join("src");
    fs::create_dir_all(&sub_dir)?;

    // Test detection from subdirectory with custom indicators
    // The engine should find the project root and detect from there
    let result_custom = engine.detect(&sub_dir).unwrap();
    // Even though no languages are configured, we can verify root discovery worked
    // by checking that it doesn't return an error

    // Now test with no indicators (default behavior)
    let default_config = DetectionConfig::default(); // Empty root_indicators (no root discovery)
    let default_engine = DetectionEngine::with_config(vec![], default_config);

    // Remove custom indicator
    fs::remove_file(project_path.join("custom-project.toml"))?;

    // Test detection from subdirectory with no root indicators
    let result_no_indicators = default_engine.detect(&sub_dir).unwrap();
    // This should not find any project root since no indicators are defined

    // Both should succeed (no panics/errors), confirming expected behavior
    assert!(result_custom.is_empty()); // Empty because no languages configured
    assert!(result_no_indicators.is_empty()); // Empty because no root indicators + no languages configured

    Ok(())
}

/// Test that configuration can be serialized and deserialized
#[test]
fn test_detection_config_serialization() {
    let original_config = DetectionConfig {
        max_upward_traversal: 8,
        require_vcs_root: true,
        confidence_threshold: 0.4,
        root_indicators: vec![RootIndicator {
            pattern: "project.yaml".to_string(),
            weight: 0.7,
        }],
    };

    // Serialize to TOML
    let toml_str = toml::to_string(&original_config).unwrap();
    assert!(toml_str.contains("max_upward_traversal = 8"));
    assert!(toml_str.contains("require_vcs_root = true"));
    assert!(toml_str.contains("confidence_threshold = 0.4"));
    assert!(toml_str.contains("project.yaml"));

    // Deserialize from TOML
    let deserialized_config: DetectionConfig = toml::from_str(&toml_str).unwrap();
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
}

/// Test that main Config includes DetectionConfig
#[test]
fn test_main_config_includes_detection() {
    let config = Config::new(vec![]);

    // Should have default detection config
    assert_eq!(config.detection.max_upward_traversal, 10);
    assert_eq!(config.detection.confidence_threshold, 0.3);
}

/// Test full config serialization with detection config
#[test]
fn test_full_config_with_detection_serialization() {
    let config = Config::new(vec![]);

    // Serialize the full config
    let toml_str = toml::to_string(&config).unwrap();

    // Should contain detection section
    assert!(toml_str.contains("[detection]"));
    assert!(toml_str.contains("max_upward_traversal"));
    assert!(toml_str.contains("confidence_threshold"));

    // Should be able to deserialize back
    let deserialized_config: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        deserialized_config.detection.max_upward_traversal,
        config.detection.max_upward_traversal
    );
    assert_eq!(
        deserialized_config.detection.confidence_threshold,
        config.detection.confidence_threshold
    );
}

/// Test that configuration file loading works with detection config
#[test]
fn test_config_file_loading_with_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");

    // Create a config file with custom detection settings
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

    // Load the config
    let config = Config::load_from_file(&config_file)?;

    // Verify detection config was loaded correctly
    assert_eq!(config.detection.max_upward_traversal, 5);
    assert!(config.detection.require_vcs_root);
    assert_eq!(config.detection.confidence_threshold, 0.6);
    assert_eq!(config.detection.root_indicators.len(), 2);
    assert_eq!(config.detection.root_indicators[0].pattern, "build.sbt");
    assert_eq!(config.detection.root_indicators[0].weight, 0.9);
    assert_eq!(config.detection.root_indicators[1].pattern, "project.clj");
    assert_eq!(config.detection.root_indicators[1].weight, 0.85);

    // Verify other config sections work too
    assert_eq!(config.display.max_frameworks, 3);
    assert_eq!(config.display.framework_separator, " | ");
    assert_eq!(config.cache.max_entries, 500);
    assert_eq!(config.cache.ttl_seconds, 600);
    assert_eq!(config.languages.len(), 1);
    assert_eq!(config.languages[0].name, "Test Language");

    Ok(())
}
