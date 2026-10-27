pub mod parser;
mod templates;
mod validator;

pub use self::templates::built_in_config;
pub use self::validator::validate_config;

use crate::types::{DetectionConfig, DisplayConfig, Framework, Indicator};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default, rename = "frameworks")]
    pub frameworks: Vec<Framework>,
    #[serde(rename = "indicators")]
    pub indicators: Vec<Indicator>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::detection::matchers::test_helpers::helpers::create_test_indicator;

    #[test]
    fn test_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            indicators: vec![create_test_indicator("Test", &["test.file"])],
            ..Default::default()
        };

        let serialized = toml::to_string(&config)?;
        let deserialized: Config = toml::from_str(&serialized)?;

        assert_eq!(config, deserialized);
        Ok(())
    }
}
