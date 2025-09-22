//! Configuration file parsing

use super::Config;
use crate::types::ProjectIndicator;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Configuration parsing errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found at {path}")]
    FileNotFound { path: PathBuf },

    #[error("Failed to read configuration file: {source}")]
    ReadError {
        #[from]
        source: std::io::Error,
    },

    #[error("Failed to parse TOML configuration: {source}")]
    TomlParseError {
        #[from]
        source: toml::de::Error,
    },

    #[error("Invalid configuration: {message}")]
    ValidationError { message: String },

    #[error("Unsupported configuration version: {version}")]
    UnsupportedVersion { version: String },
}

/// Configuration file parser
pub struct ConfigParser;

impl ConfigParser {
    /// Default configuration file paths to search (in order of preference)
    pub fn default_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Current directory
        paths.push(PathBuf::from("project-indicator.toml"));
        paths.push(PathBuf::from(".project-indicator.toml"));

        // 2. XDG config directory
        if let Some(config_dir) = Self::xdg_config_dir() {
            paths.push(config_dir.join("project-indicator/config.toml"));
            paths.push(config_dir.join("project-indicator.toml"));
        }

        // 3. Home directory
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".config/project-indicator/config.toml"));
            paths.push(home_dir.join(".config/project-indicator.toml"));
            paths.push(home_dir.join(".project-indicator.toml"));
        }

        // 4. Fish config directory (for compatibility)
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".config/fish/project_indicators.toml"));
        }

        paths
    }

    /// Get XDG config directory
    fn xdg_config_dir() -> Option<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
    }

    /// Load configuration from default locations
    pub fn load_default() -> Result<Config> {
        let paths = Self::default_config_paths();

        for path in &paths {
            if path.exists() {
                log::debug!("Found config file at: {}", path.display());
                return Self::load_from_file(path)
                    .with_context(|| format!("Failed to load config from {}", path.display()));
            }
        }

        // If no config file found, try to load from example
        Self::load_fallback_config()
    }

    /// Load configuration from a specific file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::parse_toml_content(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    /// Parse TOML content into Config
    pub fn parse_toml_content(content: &str) -> Result<Config> {
        // Try to parse as V2 format first
        match toml::from_str::<Config>(content) {
            Ok(config) => {
                // Validate version compatibility
                if config.meta.version != "2.0" && !config.meta.version.starts_with("2.") {
                    log::warn!(
                        "Config version {} may not be fully compatible",
                        config.meta.version
                    );
                }
                Ok(config)
            }
            Err(e) => {
                // Try to parse as V1 format (legacy Fish config)
                log::debug!("Failed to parse as V2 config, trying V1 format: {}", e);
                Self::parse_v1_format(content)
                    .with_context(|| "Failed to parse as both V2 and V1 config formats")
            }
        }
    }

    /// Parse legacy V1 format (Fish config style)
    fn parse_v1_format(content: &str) -> Result<Config> {
        #[derive(serde::Deserialize)]
        struct V1Config {
            #[serde(rename = "indicators")]
            indicators: Vec<V1Indicator>,
        }

        #[derive(serde::Deserialize)]
        struct V1Indicator {
            name: String,
            files: Vec<String>,
            color: String,
            icon: String,
            priority: u8,
        }

        let v1_config: V1Config =
            toml::from_str(content).map_err(|e| ConfigError::TomlParseError { source: e })?;

        let languages: Vec<ProjectIndicator> = v1_config
            .indicators
            .into_iter()
            .map(|indicator| ProjectIndicator {
                name: indicator.name,
                files: indicator.files,
                color: indicator.color,
                icon: indicator.icon,
                priority: indicator.priority,
                frameworks: Vec::new(), // V1 doesn't have frameworks
            })
            .collect();

        Ok(Config::new(languages))
    }

    /// Load fallback configuration when no config file is found
    fn load_fallback_config() -> Result<Config> {
        log::info!("No configuration file found, using minimal fallback config");

        // Create a minimal config with common languages
        let languages = vec![
            ProjectIndicator {
                name: "Rust".to_string(),
                files: vec!["Cargo.toml".to_string()],
                color: "#DEA584".to_string(),
                icon: "".to_string(),
                priority: 1,
                frameworks: Vec::new(),
            },
            ProjectIndicator {
                name: "TypeScript".to_string(),
                files: vec!["package.json".to_string(), "tsconfig.json".to_string()],
                color: "#3178C6".to_string(),
                icon: "󰛦".to_string(),
                priority: 1,
                frameworks: Vec::new(),
            },
            ProjectIndicator {
                name: "JavaScript".to_string(),
                files: vec!["package.json".to_string()],
                color: "#F0DB4F".to_string(),
                icon: "󰌞".to_string(),
                priority: 2,
                frameworks: Vec::new(),
            },
            ProjectIndicator {
                name: "Python".to_string(),
                files: vec!["pyproject.toml".to_string(), "requirements.txt".to_string()],
                color: "#3776AB".to_string(),
                icon: "".to_string(),
                priority: 1,
                frameworks: Vec::new(),
            },
            ProjectIndicator {
                name: "Go".to_string(),
                files: vec!["go.mod".to_string()],
                color: "#01ADD8".to_string(),
                icon: "".to_string(),
                priority: 1,
                frameworks: Vec::new(),
            },
        ];

        Ok(Config::new(languages))
    }

    /// Save configuration to a file
    pub fn save_to_file<P: AsRef<Path>>(config: &Config, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let content =
            toml::to_string_pretty(config).with_context(|| "Failed to serialize config to TOML")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        log::info!("Configuration saved to: {}", path.display());
        Ok(())
    }

    /// Get the recommended config file path for saving
    pub fn default_save_path() -> Result<PathBuf> {
        if let Some(config_dir) = Self::xdg_config_dir() {
            Ok(config_dir.join("project-indicator/config.toml"))
        } else {
            anyhow::bail!("Could not determine config directory");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_v2_config() {
        let toml_content = r##"
[meta]
version = "2.0"

[display]
show_frameworks = true
max_frameworks = 2
framework_separator = "+"

[[languages]]
name = "TypeScript"
files = ["package.json", "tsconfig.json"]
color = "#3178C6"
icon = "TS"
priority = 1

  [[languages.frameworks]]
  name = "React"
  icon = "React"
  priority = 1

  [languages.frameworks.detection]
  type = "PackageJson"
  dependencies = ["react"]
"##;

        let config = ConfigParser::parse_toml_content(toml_content).unwrap();

        assert_eq!(config.meta.version, "2.0");
        assert!(config.display.show_frameworks);
        assert_eq!(config.languages.len(), 1);
        assert_eq!(config.languages[0].name, "TypeScript");
        assert_eq!(config.languages[0].frameworks.len(), 1);
        assert_eq!(config.languages[0].frameworks[0].name, "React");
    }

    #[test]
    fn test_parse_v1_config() {
        let toml_content = r##"
[[indicators]]
name = "Python"
files = ["pyproject.toml", "requirements.txt"]
color = "#3776AB"
icon = "PY"
priority = 1

[[indicators]]
name = "Rust"
files = ["Cargo.toml"]
color = "#DEA584"
icon = "RS"
priority = 1
"##;

        let config = ConfigParser::parse_toml_content(toml_content).unwrap();

        assert_eq!(config.languages.len(), 2);
        assert_eq!(config.languages[0].name, "Python");
        assert_eq!(config.languages[1].name, "Rust");
        // V1 format should not have frameworks
        assert!(config.languages[0].frameworks.is_empty());
        assert!(config.languages[1].frameworks.is_empty());
    }

    #[test]
    fn test_save_and_load_config() {
        let original_config = Config::new(vec![ProjectIndicator {
            name: "Test Language".to_string(),
            files: vec!["test.file".to_string()],
            color: "#FF0000".to_string(),
            icon: "🔥".to_string(),
            priority: 1,
            frameworks: Vec::new(),
        }]);

        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Save config
        ConfigParser::save_to_file(&original_config, temp_path).unwrap();

        // Load config back
        let loaded_config = ConfigParser::load_from_file(temp_path).unwrap();

        assert_eq!(original_config, loaded_config);
    }

    #[test]
    fn test_config_methods() {
        let config = Config::new(vec![
            ProjectIndicator {
                name: "High Priority".to_string(),
                files: vec!["high.file".to_string()],
                color: "#FF0000".to_string(),
                icon: "🔥".to_string(),
                priority: 1,
                frameworks: Vec::new(),
            },
            ProjectIndicator {
                name: "Low Priority".to_string(),
                files: vec!["low.file".to_string()],
                color: "#0000FF".to_string(),
                icon: "❄️".to_string(),
                priority: 5,
                frameworks: Vec::new(),
            },
        ]);

        // Test priority sorting
        let sorted = config.languages_by_priority();
        assert_eq!(sorted[0].name, "High Priority");
        assert_eq!(sorted[1].name, "Low Priority");

        // Test finding language
        assert!(config.find_language("high priority").is_some());
        assert!(config.find_language("nonexistent").is_none());

        // Test file patterns
        let patterns = config.all_file_patterns();
        assert!(patterns.contains(&&"high.file".to_string()));
        assert!(patterns.contains(&&"low.file".to_string()));
    }

    #[test]
    fn test_invalid_toml() {
        let invalid_toml = "this is not valid toml [[[";
        let result = ConfigParser::parse_toml_content(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_fallback_config() {
        let config = ConfigParser::load_fallback_config().unwrap();
        assert!(!config.languages.is_empty());

        // Should contain common languages
        let language_names: Vec<&String> = config.languages.iter().map(|l| &l.name).collect();
        assert!(language_names.contains(&&"Rust".to_string()));
        assert!(language_names.contains(&&"TypeScript".to_string()));
        assert!(language_names.contains(&&"Python".to_string()));
    }
}
