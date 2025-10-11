use super::Config;
use crate::types::ProjectIndicator;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
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
pub struct ConfigParser;

impl ConfigParser {
    pub fn default_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        paths.push(PathBuf::from("project-indicator.toml"));
        paths.push(PathBuf::from(".project-indicator.toml"));

        if let Some(config_dir) = Self::xdg_config_dir() {
            paths.push(config_dir.join("project-indicator/config.toml"));
            paths.push(config_dir.join("project-indicator.toml"));
        }

        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".config/project-indicator/config.toml"));
            paths.push(home_dir.join(".config/project-indicator.toml"));
            paths.push(home_dir.join(".project-indicator.toml"));
        }

        paths
    }
    fn xdg_config_dir() -> Option<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
    }
    pub fn load_default() -> Result<Config> {
        let paths = Self::default_config_paths();

        for path in &paths {
            if path.exists() {
                log::debug!("Found config file at: {}", path.display());
                return Self::load_from_file(path)
                    .with_context(|| format!("Failed to load config from {}", path.display()));
            }
        }

        Self::load_fallback_config()
    }
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::parse_toml_content(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }
    pub fn parse_toml_content(content: &str) -> Result<Config> {
        let config: Config = toml::from_str(content)
            .with_context(|| "Failed to parse configuration file as V2 format")?;

        if config.meta.version != "2.0" && !config.meta.version.starts_with("2.") {
            log::warn!(
                "Config version {} may not be fully compatible",
                config.meta.version
            );
        }

        Ok(config)
    }
    fn load_fallback_config() -> Result<Config> {
        log::info!("No configuration file found, using minimal fallback config");

        let languages = vec![
            ProjectIndicator::new(
                "Rust".to_string(),
                vec!["Cargo.toml".to_string()],
                "#DEA584".to_string(),
                "".to_string(),
                1,
                Vec::new(),
            ),
            ProjectIndicator::new(
                "TypeScript".to_string(),
                vec!["package.json".to_string(), "tsconfig.json".to_string()],
                "#3178C6".to_string(),
                "󰛦".to_string(),
                1,
                Vec::new(),
            ),
            ProjectIndicator::new(
                "JavaScript".to_string(),
                vec!["package.json".to_string()],
                "#F0DB4F".to_string(),
                "󰌞".to_string(),
                2,
                Vec::new(),
            ),
            ProjectIndicator::new(
                "Python".to_string(),
                vec!["pyproject.toml".to_string(), "requirements.txt".to_string()],
                "#3776AB".to_string(),
                "".to_string(),
                1,
                Vec::new(),
            ),
            ProjectIndicator::new(
                "Go".to_string(),
                vec!["go.mod".to_string()],
                "#01ADD8".to_string(),
                "".to_string(),
                1,
                Vec::new(),
            ),
        ];

        Ok(Config::new(languages))
    }
    pub fn save_to_file<P: AsRef<Path>>(config: &Config, path: P) -> Result<()> {
        let path = path.as_ref();

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
    fn test_parse_v2_config() -> Result<(), Box<dyn std::error::Error>> {
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
  type = "NodeEcosystem"
  dependencies = ["react"]
"##;

        let config = ConfigParser::parse_toml_content(toml_content)?;

        assert_eq!(config.meta.version, "2.0");
        assert!(config.display.show_frameworks);
        assert_eq!(config.languages.len(), 1);
        assert_eq!(config.languages[0].name, "TypeScript");
        assert_eq!(config.languages[0].frameworks.len(), 1);
        assert_eq!(config.languages[0].frameworks[0].name, "React");
        Ok(())
    }

    #[test]
    fn test_save_and_load_config() -> Result<(), Box<dyn std::error::Error>> {
        let original_config = Config::new(vec![ProjectIndicator::new(
            "Test Language".to_string(),
            vec!["test.file".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            Vec::new(),
        )]);

        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path();

        ConfigParser::save_to_file(&original_config, temp_path)?;

        let loaded_config = ConfigParser::load_from_file(temp_path)?;

        assert_eq!(original_config, loaded_config);
        Ok(())
    }

    #[test]
    fn test_config_methods() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![
            ProjectIndicator::new(
                "High Priority".to_string(),
                vec!["high.file".to_string()],
                "#FF0000".to_string(),
                "🔥".to_string(),
                1,
                Vec::new(),
            ),
            ProjectIndicator::new(
                "Low Priority".to_string(),
                vec!["low.file".to_string()],
                "#0000FF".to_string(),
                "❄️".to_string(),
                5,
                Vec::new(),
            ),
        ]);

        assert_eq!(config.languages.len(), 2);
        assert_eq!(config.languages[0].name, "High Priority");
        assert_eq!(config.languages[1].name, "Low Priority");
        Ok(())
    }

    #[test]
    fn test_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let invalid_toml = "this is not valid toml [[[";
        let result = ConfigParser::parse_toml_content(invalid_toml);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_fallback_config() -> Result<(), Box<dyn std::error::Error>> {
        let config = ConfigParser::load_fallback_config()?;
        assert!(!config.languages.is_empty());

        let language_names: Vec<&String> = config.languages.iter().map(|l| &l.name).collect();
        assert!(language_names.contains(&&"Rust".to_string()));
        assert!(language_names.contains(&&"TypeScript".to_string()));
        assert!(language_names.contains(&&"Python".to_string()));
        Ok(())
    }
}
