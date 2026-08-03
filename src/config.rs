mod parser;
mod templates;
mod validator;

pub use self::parser::{ConfigError, ConfigParser};
pub use self::templates::{
    generate_root_indicators_simple_max_weight, vcs_root_indicators, ConfigTemplate,
    TemplateGenerator,
};
pub use self::validator::validate_config;

use crate::types::{ConfigMeta, DetectionConfig, DisplayConfig, Framework, Indicator};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Config {
    #[serde(default)]
    pub meta: ConfigMeta,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default, rename = "frameworks")]
    pub frameworks: Vec<Framework>,
    #[serde(rename = "indicators")]
    pub indicators: Vec<Indicator>,
}

impl Config {
    pub fn new(indicators: Vec<Indicator>) -> Self {
        Self {
            indicators,
            ..Default::default()
        }
    }
    pub fn load_default() -> crate::Result<Self> {
        ConfigParser::load_default()
    }
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> crate::Result<Self> {
        ConfigParser::load_from_file(path)
    }
    pub fn get_config_path() -> crate::Result<std::path::PathBuf> {
        ConfigParser::default_save_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::detection::matchers::test_helpers::helpers::{
        create_test_framework_generic, create_test_indicator_with_priority,
    };

    #[test]
    fn test_config_creation() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![]);
        assert!(config.indicators.is_empty());
        assert_eq!(config.meta.version, "3.0");
        Ok(())
    }

    #[test]
    fn test_config_with_languages() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_indicator_with_priority("Rust", vec!["Cargo.toml"], 1),
            create_test_indicator_with_priority(
                "TypeScript",
                vec!["package.json", "tsconfig.json"],
                2,
            ),
        ];

        let config = Config::new(languages);

        assert_eq!(config.indicators.len(), 2);
        assert_eq!(config.indicators[0].name, "Rust");
        assert_eq!(config.indicators[1].name, "TypeScript");
        Ok(())
    }

    #[test]
    fn test_frameworks() -> Result<(), Box<dyn std::error::Error>> {
        let frameworks = vec![
            create_test_framework_generic("React", 1),
            create_test_framework_generic("Vue", 2),
        ];

        let languages = vec![create_test_indicator_with_priority(
            "JavaScript",
            vec!["package.json"],
            1,
        )];

        let config = Config {
            frameworks,
            indicators: languages,
            ..Default::default()
        };
        let all_frameworks = config.frameworks;

        assert_eq!(all_frameworks.len(), 2);
        assert_eq!(all_frameworks[0].name, "React");
        assert_eq!(all_frameworks[1].name, "Vue");
        Ok(())
    }

    #[test]
    fn test_config_with_multiple_languages_and_frameworks() -> Result<(), Box<dyn std::error::Error>>
    {
        let frameworks = vec![
            create_test_framework_generic("React", 1),
            create_test_framework_generic("Express", 2),
        ];

        let languages = vec![
            create_test_indicator_with_priority("JavaScript", vec!["package.json", "*.js"], 1),
            create_test_indicator_with_priority("TypeScript", vec!["package.json", "*.ts"], 2),
        ];

        let config = Config {
            frameworks,
            indicators: languages,
            ..Default::default()
        };

        assert_eq!(config.indicators.len(), 2);
        assert_eq!(config.frameworks.len(), 2);
        assert_eq!(config.indicators[0].name, "JavaScript");
        assert_eq!(config.indicators[1].name, "TypeScript");
        Ok(())
    }

    #[test]
    fn test_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![create_test_indicator_with_priority(
            "Test",
            vec!["test.file"],
            1,
        )]);

        let serialized = toml::to_string(&config)?;
        let deserialized: Config = toml::from_str(&serialized)?;

        assert_eq!(config, deserialized);
        Ok(())
    }

    #[test]
    fn test_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::default();

        assert_eq!(config.meta.version, "3.0");
        assert_eq!(config.display.max_frameworks, 2);
        assert_eq!(config.detection.max_upward_traversal, 10);
        assert!(config.indicators.is_empty());
        Ok(())
    }

    #[test]
    fn test_config_path_construction() -> Result<(), Box<dyn std::error::Error>> {
        let path = Config::get_config_path()?;
        let expected_path = ConfigParser::default_save_path()?;
        assert_eq!(path, expected_path);
        Ok(())
    }

    #[test]
    fn test_config_empty_languages() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![]);

        assert!(config.indicators.is_empty());
        assert!(config.frameworks.is_empty());
        Ok(())
    }
}
