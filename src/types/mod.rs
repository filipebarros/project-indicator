//! Type definitions for project-indicator.
//!
//! This module contains all the core types used throughout the project-indicator codebase.
//! Types are organized into logical submodules for better maintainability.

mod config;
mod detection;
mod framework;
mod indicators;
mod matched_file;

// Re-export all public types
pub use config::{
    CacheConfig, ConfigMeta, DetectionConfig, DetectionMode, DisplayConfig, TrackingConfig,
};
pub use detection::{
    ConfidenceFactor, DetectionEvidence, DetectionResult, EvidenceItem, EvidenceType,
};
pub use framework::{DetectionType, FrameworkDetector, FrameworkMatch};
pub use indicators::{IndicatorContext, ProjectIndicator, RootIndicator};
pub use matched_file::{DirectoryType, MatchedFile};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_indicator_creation() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "TypeScript".to_string(),
            vec!["package.json".to_string(), "tsconfig.json".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            1,
            vec![],
        );

        assert_eq!(indicator.name, "TypeScript");
        assert_eq!(indicator.files.len(), 2);
        assert_eq!(indicator.priority, 1);
        Ok(())
    }

    #[test]
    fn test_file_matching() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "TypeScript".to_string(),
            vec!["package.json".to_string(), "*.ts".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            1,
            vec![],
        );

        let files = vec![
            "package.json".to_string(),
            "src/main.ts".to_string(),
            "README.md".to_string(),
        ];

        assert!(indicator.matches_files(&files));

        let no_match_files = vec!["README.md".to_string(), "main.py".to_string()];
        assert!(!indicator.matches_files(&no_match_files));
        Ok(())
    }

    #[test]
    fn test_wildcard_matching() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "C++".to_string(),
            vec!["*.cpp".to_string(), "*.h".to_string()],
            "#00599C".to_string(),
            "".to_string(),
            1,
            vec![],
        );

        let files = vec!["main.cpp".to_string(), "header.h".to_string()];
        assert!(indicator.matches_files(&files));

        let no_match = vec!["main.py".to_string()];
        assert!(!indicator.matches_files(&no_match));
        Ok(())
    }

    #[test]
    fn test_detection_result_empty() -> Result<(), Box<dyn std::error::Error>> {
        let result = DetectionResult::empty();
        assert!(result.is_empty());
        assert_eq!(result.confidence, 0.0);
        assert!(result.best_framework().is_none());
        Ok(())
    }

    #[test]
    fn test_detection_result_with_framework() -> Result<(), Box<dyn std::error::Error>> {
        let framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::NodeEcosystem {
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

        assert!(!result.is_empty());
        assert_eq!(result.display_icon(), Some("⚛️"));
        assert_eq!(result.display_color(), Some("#61DAFB"));
        assert!(result.best_framework().is_some());
        Ok(())
    }

    #[test]
    fn test_framework_priority_sorting() -> Result<(), Box<dyn std::error::Error>> {
        let framework1 = FrameworkDetector {
            name: "Framework1".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 3,
            files: vec![],
            root_indicators: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "Framework2".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let indicator = ProjectIndicator::new(
            "Test".to_string(),
            vec![],
            "#000000".to_string(),
            "".to_string(),
            1,
            vec![framework1, framework2],
        );

        let sorted = indicator.frameworks_by_priority();
        assert_eq!(sorted[0].name, "Framework2");
        assert_eq!(sorted[1].name, "Framework1");
        Ok(())
    }

    #[test]
    fn test_serde_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let detection_type = DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string(), "typescript".to_string()],
        };

        let json = serde_json::to_string(&detection_type)?;
        let deserialized: DetectionType = serde_json::from_str(&json)?;

        assert_eq!(detection_type, deserialized);
        Ok(())
    }

    #[test]
    fn test_display_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config = DisplayConfig::default();
        assert!(config.show_frameworks);
        assert_eq!(config.max_frameworks, 2);
        assert_eq!(config.framework_separator, "+");
        Ok(())
    }

    #[test]
    fn test_cache_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_seconds, 300);
        Ok(())
    }

    #[test]
    fn test_directory_type_classification() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::classify("src"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("lib"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("app"), DirectoryType::Source);

        assert_eq!(DirectoryType::classify("test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("tests"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("__tests__"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("fixtures"), DirectoryType::Test);

        assert_eq!(DirectoryType::classify("dist"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("build"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("target"), DirectoryType::Build);

        assert_eq!(
            DirectoryType::classify("node_modules"),
            DirectoryType::Dependencies
        );
        assert_eq!(
            DirectoryType::classify("vendor"),
            DirectoryType::Dependencies
        );

        assert_eq!(DirectoryType::classify("random"), DirectoryType::Unknown);
        Ok(())
    }

    #[test]
    fn test_directory_type_weights() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::Root.weight(), 1.0);
        assert_eq!(DirectoryType::Source.weight(), 1.2);
        assert_eq!(DirectoryType::Config.weight(), 1.1);
        assert_eq!(DirectoryType::Test.weight(), 0.2);
        assert_eq!(DirectoryType::Build.weight(), 0.1);
        assert_eq!(DirectoryType::Dependencies.weight(), 0.05);
        assert_eq!(DirectoryType::Documentation.weight(), 0.6);
        assert_eq!(DirectoryType::Examples.weight(), 0.3);
        assert_eq!(DirectoryType::Unknown.weight(), 0.8);
        Ok(())
    }

    #[test]
    fn test_matched_file_root() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("package.json".to_string(), "package.json".to_string());

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "package.json");
        assert_eq!(file.depth, 0);
        assert_eq!(file.directory_type, DirectoryType::Root);
        assert_eq!(file.weight(), 1.0);
        Ok(())
    }

    #[test]
    fn test_matched_file_source_directory() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());

        assert_eq!(file.filename, "main.rs");
        assert_eq!(file.relative_path, "src/main.rs");
        assert_eq!(file.depth, 1);
        assert_eq!(file.directory_type, DirectoryType::Source);
        assert!((file.weight() - 0.84).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_matched_file_test_directory() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "test/fixtures/package.json");
        assert_eq!(file.depth, 2);
        assert_eq!(file.directory_type, DirectoryType::Test);
        assert!((file.weight() - 0.08).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_matched_file_deep_nesting() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new(
            "config.json".to_string(),
            "very/deep/nested/path/config.json".to_string(),
        );

        assert_eq!(file.depth, 4);
        assert_eq!(file.directory_type, DirectoryType::Unknown);
        assert!((file.weight() - 0.04).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_depth_calculation_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("file.txt".to_string(), "".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "file.txt".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "dir/file.txt".to_string());
        assert_eq!(file.depth, 1);
        Ok(())
    }

    #[test]
    fn test_weight_calculation_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let root_package = MatchedFile::new("package.json".to_string(), "package.json".to_string());
        assert_eq!(root_package.weight(), 1.0);

        let src_file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());
        assert!((src_file.weight() - 0.84).abs() < f32::EPSILON);

        let test_fixture = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );
        assert!((test_fixture.weight() - 0.08).abs() < f32::EPSILON);

        let node_modules_file = MatchedFile::new(
            "package.json".to_string(),
            "node_modules/some-lib/package.json".to_string(),
        );
        assert!((node_modules_file.weight() - 0.02).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_directory_classification_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::classify("SRC"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("Test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("DIST"), DirectoryType::Build);
        assert_eq!(
            DirectoryType::classify("Node_Modules"),
            DirectoryType::Dependencies
        );
        Ok(())
    }
}
