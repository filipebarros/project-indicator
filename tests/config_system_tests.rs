use project_indicator::detection::DetectionEngine;
use project_indicator::types::DetectionConfig;
use project_indicator::Config;
use std::fs;
use tempfile::TempDir;
#[test]
fn test_detection_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let config = DetectionConfig::default();

    assert_eq!(config.max_upward_traversal, 10);
    assert!(!config.require_vcs_root);
    assert_eq!(config.confidence_threshold, 0.3);
    Ok(())
}
#[test]
fn test_detection_engine_with_custom_config() -> Result<(), Box<dyn std::error::Error>> {
    let detection_config = DetectionConfig {
        max_upward_traversal: 3,
        require_vcs_root: false,
        confidence_threshold: 0.8,
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    let engine = DetectionEngine::with_config(vec![], vec![], detection_config);

    let temp_dir = TempDir::new()?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.is_empty());
    Ok(())
}
#[test]
fn test_detection_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let original_config = DetectionConfig {
        max_upward_traversal: 8,
        require_vcs_root: true,
        confidence_threshold: 0.4,
        max_depth: 3,
        detection_mode: project_indicator::types::DetectionMode::default(),
    };

    let toml_str = toml::to_string(&original_config)?;
    assert!(toml_str.contains("max_upward_traversal = 8"));
    assert!(toml_str.contains("require_vcs_root = true"));
    assert!(toml_str.contains("confidence_threshold = 0.4"));

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
version = \"3.0\"

[display]
show_frameworks = true
max_frameworks = 3
framework_separator = \" | \"

[detection]
max_upward_traversal = 5
require_vcs_root = true
confidence_threshold = 0.6

[[indicators]]
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

    assert_eq!(config.display.max_frameworks, 3);
    assert_eq!(config.display.framework_separator, " | ");
    assert_eq!(config.indicators.len(), 1);
    assert_eq!(config.indicators[0].name, "Test Language");

    Ok(())
}
#[test]
fn test_config_with_removed_keys_still_parses() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");

    // Old-schema config containing keys removed in the 2026-09 ponytail
    // cleanup (detection.max_matches_per_pattern/small_project_threshold/
    // extreme_size_threshold, detection.root_indicators, Framework.files).
    // Serde ignores unknown fields by default, so a config written before
    // that cleanup must still parse without error.
    let config_content = "[meta]
version = \"3.0\"

[display]
show_frameworks = true
max_frameworks = 3
framework_separator = \" | \"

[detection]
max_upward_traversal = 5
require_vcs_root = true
confidence_threshold = 0.6
max_matches_per_pattern = 15
small_project_threshold = 50
extreme_size_threshold = 500

[[detection.root_indicators]]
pattern = \".git\"
weight = 1.0
context = \"VersionControl\"

[[indicators]]
name = \"Test Language\"
files = [\"test.file\"]
color = \"#FF0000\"
icon = \"🧪\"
priority = 1

[[frameworks]]
name = \"Test Framework\"
ecosystems = [\"npm\"]
color = \"#00FF00\"
priority = 1
files = [\"legacy.marker\"]

[frameworks.detection]
type = \"Dependencies\"
dependencies = [\"test-framework\"]
";

    fs::write(&config_file, config_content)?;

    let config = Config::load_from_file(&config_file)?;

    assert_eq!(config.detection.max_upward_traversal, 5);
    assert_eq!(config.detection.confidence_threshold, 0.6);
    assert_eq!(config.indicators.len(), 1);
    assert_eq!(config.frameworks.len(), 1);
    assert_eq!(config.frameworks[0].name, "Test Framework");

    Ok(())
}
