use super::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {message}")]
    ValidationError { message: String },

    #[error("Unsupported configuration version: {version}")]
    UnsupportedVersion { version: String },
}
fn first_existing(paths: Vec<PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.exists())
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
    /// The config file `load_default` would read, without parsing it.
    /// `None` means detection runs on the built-in fallback config.
    pub fn active_config_path() -> Option<PathBuf> {
        first_existing(Self::default_config_paths())
    }
    pub fn load_default() -> Result<Config> {
        let paths = Self::default_config_paths();

        for path in &paths {
            if path.exists() {
                log::debug!("Found config file at: {}", path.display());
                // Soft fallback: a config that fails to parse (old schema,
                // typos) must never break a shell prompt. `config validate`
                // still surfaces the error loudly via load_from_file.
                match Self::load_from_file(path) {
                    Ok(config) => return Ok(config),
                    Err(e) => {
                        log::warn!(
                            "Ignoring unparsable config at {} ({}); using built-in template",
                            path.display(),
                            e
                        );
                        return Self::load_fallback_config();
                    }
                }
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

        if !config.meta.version.starts_with("3.") {
            log::warn!(
                "Config version {} predates the v3 schema and may not be fully compatible",
                config.meta.version
            );
        }

        Ok(config)
    }
    fn load_fallback_config() -> Result<Config> {
        log::info!("No configuration file found, using built-in full template");

        // Use the full built-in template so all languages and framework
        // detection work out of the box without running `config init`
        Ok(crate::config::templates::create_full_template().config)
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
    use crate::types::Indicator;

    #[test]
    fn test_parse_v3_config() -> Result<(), Box<dyn std::error::Error>> {
        let toml_content = r##"
[meta]
version = "3.0"

[display]
show_frameworks = true
max_frameworks = 2
framework_separator = "+"

[[indicators]]
name = "TypeScript"
files = ["package.json", "tsconfig.json"]
color = "#3178C6"
icon = "TS"
priority = 1
ecosystems = ["npm"]

[[frameworks]]
name = "React"
ecosystems = ["npm"]
icon = "React"
priority = 1

[frameworks.detection]
type = "Dependencies"
dependencies = ["react"]
"##;

        let config = ConfigParser::parse_toml_content(toml_content)?;

        assert_eq!(config.meta.version, "3.0");
        assert!(config.display.show_frameworks);
        assert_eq!(config.indicators.len(), 1);
        assert_eq!(config.indicators[0].name, "TypeScript");
        assert_eq!(
            config.indicators[0].ecosystems,
            vec![crate::types::Ecosystem::Npm]
        );
        assert_eq!(config.frameworks.len(), 1);
        assert_eq!(config.frameworks[0].name, "React");
        Ok(())
    }

    #[test]
    fn test_config_methods() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::new(vec![
            Indicator::new(
                "High Priority".to_string(),
                vec!["high.file".to_string()],
                "#FF0000".to_string(),
                "🔥".to_string(),
                1,
                Vec::new(),
            ),
            Indicator::new(
                "Low Priority".to_string(),
                vec!["low.file".to_string()],
                "#0000FF".to_string(),
                "❄️".to_string(),
                5,
                Vec::new(),
            ),
        ]);

        assert_eq!(config.indicators.len(), 2);
        assert_eq!(config.indicators[0].name, "High Priority");
        assert_eq!(config.indicators[1].name, "Low Priority");
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
    fn test_first_existing_picks_earliest_present_path() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let missing = dir.path().join("missing.toml");
        let present_a = dir.path().join("a.toml");
        let present_b = dir.path().join("b.toml");
        fs::write(&present_a, "x")?;
        fs::write(&present_b, "x")?;

        assert_eq!(
            first_existing(vec![missing.clone(), present_a.clone(), present_b.clone()]),
            Some(present_a)
        );
        assert_eq!(first_existing(vec![missing]), None);
        Ok(())
    }

    #[test]
    fn test_fallback_config() -> Result<(), Box<dyn std::error::Error>> {
        let config = ConfigParser::load_fallback_config()?;
        assert!(!config.indicators.is_empty());

        let language_names: Vec<&String> = config.indicators.iter().map(|l| &l.name).collect();
        assert!(language_names.contains(&&"Rust".to_string()));
        assert!(language_names.contains(&&"TypeScript".to_string()));
        assert!(language_names.contains(&&"Python".to_string()));

        // The fallback must include framework detection out of the box
        assert!(
            !config.frameworks.is_empty(),
            "fallback config should define frameworks"
        );
        Ok(())
    }
}
