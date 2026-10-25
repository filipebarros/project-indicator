//! Core types, split by domain.

mod config;
mod detection;
mod framework;
mod indicators;
mod matched_file;

pub use config::{DetectionConfig, DisplayConfig};
pub use detection::{
    ConfidenceFactor, DetectionEvidence, DetectionResult, EvidenceItem, EvidenceType, NestedProject,
};
pub use framework::{DetectionType, Ecosystem, Framework, FrameworkMatch};
pub use indicators::{Indicator, IndicatorContext, RootIndicator};
pub use matched_file::{DirectoryType, MatchedFile};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_config_defaults() {
        let config = DisplayConfig::default();
        assert_eq!(config.max_frameworks, 5);
    }

    #[test]
    fn test_directory_type_classification() {
        assert_eq!(DirectoryType::classify("src"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("lib"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("app"), DirectoryType::Source);

        assert_eq!(DirectoryType::classify("test"), DirectoryType::Noise);
        assert_eq!(DirectoryType::classify("fixtures"), DirectoryType::Noise);
        assert_eq!(DirectoryType::classify("dist"), DirectoryType::Noise);
        assert_eq!(DirectoryType::classify("target"), DirectoryType::Noise);
        assert_eq!(
            DirectoryType::classify("node_modules"),
            DirectoryType::Noise
        );
        assert_eq!(DirectoryType::classify("vendor"), DirectoryType::Noise);

        assert_eq!(DirectoryType::classify("random"), DirectoryType::Unknown);
    }

    #[test]
    fn test_matched_file_root() {
        let file = MatchedFile::new("package.json".to_string(), "package.json".to_string());

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "package.json");
        assert_eq!(file.depth, 0);
        assert_eq!(file.directory_type, DirectoryType::Root);
        assert_eq!(file.weight(), 1.0);
    }

    #[test]
    fn test_matched_file_source_directory() {
        let file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());

        assert_eq!(file.filename, "main.rs");
        assert_eq!(file.relative_path, "src/main.rs");
        assert_eq!(file.depth, 1);
        assert_eq!(file.directory_type, DirectoryType::Source);
        assert!((file.weight() - 0.84).abs() < f32::EPSILON);
    }

    #[test]
    fn test_matched_file_test_directory() {
        let file = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "test/fixtures/package.json");
        assert_eq!(file.depth, 2);
        assert_eq!(file.directory_type, DirectoryType::Noise);
        assert!((file.weight() - 0.04).abs() < f32::EPSILON);
    }

    #[test]
    fn test_matched_file_deep_nesting() {
        let file = MatchedFile::new(
            "config.json".to_string(),
            "very/deep/nested/path/config.json".to_string(),
        );

        assert_eq!(file.depth, 4);
        assert_eq!(file.directory_type, DirectoryType::Unknown);
        assert!((file.weight() - 0.04).abs() < f32::EPSILON);
    }

    #[test]
    fn test_depth_calculation_edge_cases() {
        let file = MatchedFile::new("file.txt".to_string(), "".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "file.txt".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "dir/file.txt".to_string());
        assert_eq!(file.depth, 1);
    }

    #[test]
    fn test_directory_classification_case_insensitive() {
        assert_eq!(DirectoryType::classify("SRC"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("Test"), DirectoryType::Noise);
        assert_eq!(DirectoryType::classify("DIST"), DirectoryType::Noise);
        assert_eq!(
            DirectoryType::classify("Node_Modules"),
            DirectoryType::Noise
        );
    }
}
