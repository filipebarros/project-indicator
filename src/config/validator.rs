use super::{Config, ConfigError};
#[cfg(test)]
use crate::types::DetectionConfig;
use crate::types::{DetectionType, FrameworkDetector, ProjectIndicator};
use anyhow::Result;
use std::collections::HashSet;
fn validation_error(message: impl Into<String>) -> anyhow::Error {
    ConfigError::ValidationError {
        message: message.into(),
    }
    .into()
}
fn simple_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::anyhow!(message.into())
}
pub fn validate_config(config: &Config) -> Result<()> {
    validate_version(&config.meta.version)?;
    validate_display_config(config)?;
    validate_cache_config(config)?;
    validate_languages(&config.languages)?;
    validate_unique_language_names(&config.languages)?;

    Ok(())
}
fn validate_version(version: &str) -> Result<()> {
    if !version.starts_with("2.") {
        return Err(ConfigError::UnsupportedVersion {
            version: version.to_string(),
        }
        .into());
    }
    Ok(())
}
fn validate_display_config(config: &Config) -> Result<()> {
    if config.display.max_frameworks == 0 {
        return Err(validation_error("max_frameworks must be greater than 0"));
    }

    if config.display.framework_separator.is_empty() {
        return Err(validation_error("framework_separator cannot be empty"));
    }

    Ok(())
}
fn validate_cache_config(config: &Config) -> Result<()> {
    if config.cache.ttl_seconds > 86400 {
        log::warn!(
            "Cache TTL is very high: {} seconds ({}h)",
            config.cache.ttl_seconds,
            config.cache.ttl_seconds / 3600
        );
    }

    if config.cache.max_entries > 10000 {
        log::warn!(
            "Cache max entries is very high: {}",
            config.cache.max_entries
        );
    }

    Ok(())
}
fn validate_languages(languages: &[ProjectIndicator]) -> Result<()> {
    if languages.is_empty() {
        return Err(ConfigError::ValidationError {
            message: "Configuration must contain at least one language".to_string(),
        }
        .into());
    }

    for (i, language) in languages.iter().enumerate() {
        validate_language(language).map_err(|e| ConfigError::ValidationError {
            message: format!("Language '{}' at index {}: {}", language.name, i, e),
        })?;
    }

    Ok(())
}
fn validate_language(language: &ProjectIndicator) -> Result<()> {
    if language.name.trim().is_empty() {
        return Err(simple_error("name cannot be empty"));
    }

    if language.files.is_empty() {
        return Err(simple_error("must have at least one file pattern"));
    }

    for file_pattern in &language.files {
        if file_pattern.trim().is_empty() {
            return Err(simple_error("file pattern cannot be empty"));
        }
    }

    if !is_valid_hex_color(&language.color) {
        return Err(simple_error(format!(
            "invalid hex color: {}",
            language.color
        )));
    }

    if language.icon.trim().is_empty() {
        log::warn!("Language '{}' has empty icon", language.name);
    }

    if language.priority == 0 {
        return Err(simple_error("priority must be greater than 0"));
    }

    validate_frameworks(&language.frameworks)?;

    Ok(())
}
fn validate_frameworks(frameworks: &[FrameworkDetector]) -> Result<()> {
    use std::collections::HashMap;
    let mut name_detection_pairs = HashMap::new();

    for framework in frameworks {
        validate_framework(framework)?;

        let key = (
            framework.name.clone(),
            std::mem::discriminant(&framework.detection),
        );
        if name_detection_pairs.contains_key(&key) {
            return Err(simple_error(format!(
                "duplicate framework '{}' with same detection type",
                framework.name
            )));
        }
        name_detection_pairs.insert(key, ());
    }

    Ok(())
}
fn validate_framework(framework: &FrameworkDetector) -> Result<()> {
    if framework.name.trim().is_empty() {
        return Err(simple_error("framework name cannot be empty"));
    }

    if framework.priority == 0 {
        return Err(simple_error("framework priority must be greater than 0"));
    }

    validate_detection_type(&framework.detection)?;

    if let Some(color) = &framework.color {
        if !is_valid_hex_color(color) {
            return Err(simple_error(format!(
                "invalid framework hex color: {}",
                color
            )));
        }
    }

    Ok(())
}
fn validate_detection_type(detection: &DetectionType) -> Result<()> {
    match detection {
        DetectionType::NodeEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "NodeEcosystem dependencies")?;
        }
        DetectionType::RustEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "RustEcosystem dependencies")?;
        }
        DetectionType::GoEcosystem { modules } => {
            validate_non_empty_vec(modules, "GoEcosystem modules")?;
        }
        DetectionType::PythonEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "PythonEcosystem dependencies")?;
        }
        DetectionType::RubyEcosystem { gems } => {
            validate_non_empty_vec(gems, "RubyEcosystem gems")?;
        }
        DetectionType::PHPEcosystem { packages } => {
            validate_non_empty_vec(packages, "PHPEcosystem packages")?;
        }
        DetectionType::JavaEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "JavaEcosystem dependencies")?;
        }
        DetectionType::DotNetEcosystem { packages } => {
            validate_non_empty_vec(packages, "DotNetEcosystem packages")?;
        }
        DetectionType::ScalaEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "ScalaEcosystem dependencies")?;
        }
        DetectionType::DartEcosystem { dependencies } => {
            validate_non_empty_vec(dependencies, "DartEcosystem dependencies")?;
        }
        DetectionType::LuaEcosystem { packages } => {
            validate_non_empty_vec(packages, "LuaEcosystem packages")?;
        }
        DetectionType::FileExists { files } => {
            validate_non_empty_vec(files, "FileExists files")?;
        }
        DetectionType::ConfigFile { file, keys } => {
            if file.trim().is_empty() {
                return Err(simple_error("ConfigFile file path cannot be empty"));
            }
            validate_non_empty_vec(keys, "ConfigFile keys")?;
        }
    }

    Ok(())
}
fn validate_non_empty_vec(vec: &[String], context: &str) -> Result<()> {
    if vec.is_empty() {
        return Err(simple_error(format!("{} cannot be empty", context)));
    }

    for item in vec {
        if item.trim().is_empty() {
            return Err(simple_error(format!(
                "{} cannot contain empty strings",
                context
            )));
        }
    }

    Ok(())
}
fn validate_unique_language_names(languages: &[ProjectIndicator]) -> Result<()> {
    let mut names = HashSet::new();

    for language in languages {
        let lowercase_name = language.name.to_lowercase();
        if !names.insert(lowercase_name.clone()) {
            return Err(validation_error(format!(
                "duplicate language name: {}",
                language.name
            )));
        }
    }

    Ok(())
}
fn is_valid_hex_color(color: &str) -> bool {
    if !color.starts_with('#') {
        return false;
    }

    let hex_part = &color[1..];
    if hex_part.len() != 3 && hex_part.len() != 6 {
        return false;
    }

    hex_part.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CacheConfig, ConfigMeta, DisplayConfig};

    fn create_valid_language() -> ProjectIndicator {
        ProjectIndicator::new(
            "Test Language".to_string(),
            vec!["test.file".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        )
    }

    fn create_valid_framework() -> FrameworkDetector {
        FrameworkDetector {
            name: "Test Framework".to_string(),
            detection: DetectionType::NodeEcosystem {
                dependencies: vec!["test-dep".to_string()],
            },
            icon: Some("⚡".to_string()),
            color: Some("#00FF00".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }
    }

    #[test]
    fn test_validate_valid_config() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            meta: ConfigMeta {
                version: "2.0".to_string(),
            },
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![create_valid_language()],
        };

        assert!(validate_config(&config).is_ok());
        Ok(())
    }

    #[test]
    fn test_validate_invalid_version() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            meta: ConfigMeta {
                version: "1.0".to_string(),
            },
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![create_valid_language()],
        };

        assert!(validate_config(&config).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_empty_languages() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![],
        };

        assert!(validate_config(&config).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_duplicate_language_names() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![create_valid_language(), create_valid_language()],
        };

        assert!(validate_config(&config).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_hex_color() -> Result<(), Box<dyn std::error::Error>> {
        assert!(is_valid_hex_color("#FF0000"));
        assert!(is_valid_hex_color("#f0f"));
        assert!(is_valid_hex_color("#123ABC"));

        assert!(!is_valid_hex_color("FF0000"));
        assert!(!is_valid_hex_color("#GG0000"));
        assert!(!is_valid_hex_color("#FF00"));
        assert!(!is_valid_hex_color("#"));
        Ok(())
    }

    #[test]
    fn test_validate_language_with_invalid_color() -> Result<(), Box<dyn std::error::Error>> {
        let mut language = create_valid_language();
        language.color = "invalid-color".to_string();

        assert!(validate_language(&language).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_language_with_empty_files() -> Result<(), Box<dyn std::error::Error>> {
        let mut language = create_valid_language();
        language.files = vec![];

        assert!(validate_language(&language).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_framework() -> Result<(), Box<dyn std::error::Error>> {
        let framework = create_valid_framework();
        assert!(validate_framework(&framework).is_ok());
        Ok(())
    }

    #[test]
    fn test_validate_framework_with_invalid_detection() -> Result<(), Box<dyn std::error::Error>> {
        let framework = FrameworkDetector {
            name: "Test".to_string(),
            detection: DetectionType::NodeEcosystem {
                dependencies: vec![],
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        assert!(validate_framework(&framework).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_display_config() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Config {
            languages: vec![create_valid_language()],
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());

        config.display.max_frameworks = 0;
        assert!(validate_config(&config).is_err());

        config.display.max_frameworks = 2;
        config.display.framework_separator = "".to_string();
        assert!(validate_config(&config).is_err());
        Ok(())
    }

    #[test]
    fn test_validate_detection_types() -> Result<(), Box<dyn std::error::Error>> {
        let detection_types = vec![
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
            DetectionType::RustEcosystem {
                dependencies: vec!["serde".to_string()],
            },
            DetectionType::GoEcosystem {
                modules: vec!["github.com/gin-gonic/gin".to_string()],
            },
            DetectionType::PythonEcosystem {
                dependencies: vec!["Django".to_string()],
            },
            DetectionType::RubyEcosystem {
                gems: vec!["rails".to_string()],
            },
            DetectionType::PHPEcosystem {
                packages: vec!["laravel/framework".to_string()],
            },
            DetectionType::FileExists {
                files: vec!["next.config.js".to_string()],
            },
            DetectionType::ConfigFile {
                file: "pyproject.toml".to_string(),
                keys: vec!["tool.poetry".to_string()],
            },
        ];

        for detection_type in detection_types {
            assert!(validate_detection_type(&detection_type).is_ok());
        }
        Ok(())
    }
}
