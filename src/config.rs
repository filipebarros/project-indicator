mod parser;
mod templates;
mod validator;

pub use self::parser::{ConfigError, ConfigParser};
pub use self::templates::{ConfigTemplate, TemplateGenerator};
pub use self::validator::validate_config;

use crate::types::{ConfigMeta, DetectionConfig, DisplayConfig, Framework, Indicator};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator_with_priority;

    #[test]
    fn test_config_creation() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![]);
        assert!(config.indicators.is_empty());
        assert_eq!(config.meta.version, "3.0");
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
}
