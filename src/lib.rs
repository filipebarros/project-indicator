pub mod cli;
pub mod config;
pub mod constants;
pub mod detection;
pub mod output;
pub mod patterns;
pub mod performance;
pub mod types;

#[cfg(test)]
pub mod test_utils;

pub use types::{
    DetectionResult, DetectionType, FrameworkDetector, FrameworkMatch, ProjectIndicator,
};

pub use config::Config;
pub use detection::DetectionEngine;
pub type Result<T, E = anyhow::Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_exports() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            std::mem::size_of::<DetectionResult>(),
            std::mem::size_of::<crate::types::DetectionResult>()
        );
        assert_eq!(
            std::mem::size_of::<FrameworkDetector>(),
            std::mem::size_of::<crate::types::FrameworkDetector>()
        );
        assert_eq!(
            std::mem::size_of::<ProjectIndicator>(),
            std::mem::size_of::<crate::types::ProjectIndicator>()
        );
        Ok(())
    }

    #[test]
    fn test_detection_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
        let engine = DetectionEngine::new(vec![]);

        let patterns = engine.all_file_patterns();
        assert!(patterns.is_empty());

        let languages = engine.languages_by_priority();
        assert!(languages.is_empty());

        assert!(engine.find_language("nonexistent").is_none());
        Ok(())
    }

    #[test]
    fn test_config_creation() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![]);
        assert!(config.languages.is_empty());
        assert_eq!(config.meta.version, "2.0");
        Ok(())
    }

    #[test]
    fn test_module_imports() -> Result<(), Box<dyn std::error::Error>> {
        let config = config::Config::default();
        let detection = detection::DetectionEngine::new(vec![]);
        let output = output::formatters::OutputFormat::Simple;
        let patterns_result = patterns::pattern_to_regex("*.rs");
        let performance = performance::FileSystemCache::default();
        let types = types::DetectionResult::new(None, vec![], 0.0);

        assert_eq!(config.languages.len(), 0);
        let languages = detection.languages_by_priority();
        assert!(languages.is_empty());
        assert!(patterns_result.is_none());
        assert!(types.language.is_none());
        assert_eq!(types.confidence, 0.0);

        let cache_stats = performance.stats();
        assert_eq!(cache_stats.metadata_entries, 0);

        match output {
            output::formatters::OutputFormat::Simple => {}
            output::formatters::OutputFormat::Full => {}
            output::formatters::OutputFormat::Json => {}
            output::formatters::OutputFormat::Compact => {}
            output::formatters::OutputFormat::Debug => {}
            output::formatters::OutputFormat::Rich => {}
        }
        Ok(())
    }

    #[test]
    fn test_result_type_alias() -> Result<(), Box<dyn std::error::Error>> {
        fn test_function() -> Result<()> {
            Ok(())
        }

        assert!(test_function().is_ok());
        Ok(())
    }

    #[test]
    fn test_detection_result_creation() -> Result<(), Box<dyn std::error::Error>> {
        let result = DetectionResult::new(None, vec![], 0.0);
        assert!(result.language.is_none());
        assert!(result.frameworks.is_empty());
        assert_eq!(result.confidence, 0.0);
        Ok(())
    }

    #[test]
    fn test_framework_detector_creation() -> Result<(), Box<dyn std::error::Error>> {
        use crate::types::DetectionType;

        let detector = FrameworkDetector {
            name: "Test".to_string(),
            detection: DetectionType::FileExists {
                files: vec!["test.txt".to_string()],
            },
            icon: Some("⚡".to_string()),
            color: Some("#FF0000".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        assert_eq!(detector.name, "Test");
        assert_eq!(detector.priority, 1);
        Ok(())
    }

    #[test]
    fn test_project_indicator_creation() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "Test Language".to_string(),
            vec!["test.txt".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        );

        assert_eq!(indicator.name, "Test Language");
        assert_eq!(indicator.files, vec!["test.txt"]);
        assert_eq!(indicator.color, "#FF0000");
        assert_eq!(indicator.icon, "🔥");
        assert_eq!(indicator.priority, 1);
        assert!(indicator.frameworks.is_empty());
        Ok(())
    }

    #[test]
    fn test_detection_types() -> Result<(), Box<dyn std::error::Error>> {
        use crate::types::DetectionType;

        let file_exists = DetectionType::FileExists {
            files: vec!["package.json".to_string()],
        };
        assert!(matches!(file_exists, DetectionType::FileExists { .. }));

        let package_json = DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        };
        assert!(matches!(package_json, DetectionType::NodeEcosystem { .. }));
        Ok(())
    }
}
