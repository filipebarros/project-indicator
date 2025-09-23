//! Configuration parsing and management

mod parser;
mod validator;

pub use self::parser::{ConfigError, ConfigParser};
pub use self::validator::validate_config;

use crate::types::{CacheConfig, ConfigMeta, DisplayConfig, ProjectIndicator};
use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Config {
    /// Configuration metadata
    #[serde(default)]
    pub meta: ConfigMeta,
    /// Display settings
    #[serde(default)]
    pub display: DisplayConfig,
    /// Cache settings
    #[serde(default)]
    pub cache: CacheConfig,
    /// Language definitions
    #[serde(rename = "languages")]
    pub languages: Vec<ProjectIndicator>,
}

impl Config {
    /// Create a new config with the given languages
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self {
            languages,
            ..Default::default()
        }
    }

    /// Load configuration from the default locations
    pub fn load_default() -> crate::Result<Self> {
        ConfigParser::load_default()
    }

    /// Load configuration from a specific file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> crate::Result<Self> {
        ConfigParser::load_from_file(path)
    }

    /// Get languages sorted by priority
    pub fn languages_by_priority(&self) -> Vec<&ProjectIndicator> {
        let mut languages: Vec<&ProjectIndicator> = self.languages.iter().collect();
        languages.sort_by_key(|lang| lang.priority);
        languages
    }

    /// Find a language by name
    pub fn find_language(&self, name: &str) -> Option<&ProjectIndicator> {
        self.languages
            .iter()
            .find(|lang| lang.name.eq_ignore_ascii_case(name))
    }

    /// Get all unique file patterns from all languages
    pub fn all_file_patterns(&self) -> Vec<&String> {
        self.languages.iter().flat_map(|lang| &lang.files).collect()
    }

    /// Get the configuration file path
    pub fn get_config_path() -> crate::Result<std::path::PathBuf> {
        use dirs::config_dir;

        let config_dir =
            config_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?;

        let path = config_dir.join("project-indicator").join("config.toml");
        Ok(path)
    }

    /// Get all frameworks from all languages
    pub fn frameworks(&self) -> Vec<&crate::types::FrameworkDetector> {
        self.languages
            .iter()
            .flat_map(|lang| &lang.frameworks)
            .collect()
    }
}
