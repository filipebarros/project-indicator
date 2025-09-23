//! Configuration validation

use super::{Config, ConfigError};
#[cfg(test)]
use crate::types::DetectionConfig;
use crate::types::{DetectionType, FrameworkDetector, ProjectIndicator};
use anyhow::Result;
use std::collections::HashSet;

/// Helper to create validation errors with consistent formatting
fn validation_error(message: impl Into<String>) -> anyhow::Error {
    ConfigError::ValidationError {
        message: message.into(),
    }
    .into()
}

/// Helper to create simple anyhow errors with consistent formatting
fn simple_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::anyhow!(message.into())
}

/// Validate a configuration for correctness and consistency
pub fn validate_config(config: &Config) -> Result<()> {
    // Check version compatibility
    validate_version(&config.meta.version)?;

    // Validate display settings
    validate_display_config(config)?;

    // Validate cache settings
    validate_cache_config(config)?;

    // Validate languages
    validate_languages(&config.languages)?;

    // Check for duplicate language names
    validate_unique_language_names(&config.languages)?;

    Ok(())
}

/// Validate configuration version
fn validate_version(version: &str) -> Result<()> {
    if !version.starts_with("2.") {
        return Err(ConfigError::UnsupportedVersion {
            version: version.to_string(),
        }
        .into());
    }
    Ok(())
}

/// Validate display configuration
fn validate_display_config(config: &Config) -> Result<()> {
    if config.display.max_frameworks == 0 {
        return Err(validation_error("max_frameworks must be greater than 0"));
    }

    if config.display.framework_separator.is_empty() {
        return Err(validation_error("framework_separator cannot be empty"));
    }

    Ok(())
}

/// Validate cache configuration
fn validate_cache_config(config: &Config) -> Result<()> {
    if config.cache.ttl_seconds > 86400 {
        // 24 hours in seconds
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

/// Validate all language configurations
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

/// Validate a single language configuration
fn validate_language(language: &ProjectIndicator) -> Result<()> {
    // Validate name
    if language.name.trim().is_empty() {
        return Err(simple_error("name cannot be empty"));
    }

    // Validate files
    if language.files.is_empty() {
        return Err(simple_error("must have at least one file pattern"));
    }

    for file_pattern in &language.files {
        if file_pattern.trim().is_empty() {
            return Err(simple_error("file pattern cannot be empty"));
        }
    }

    // Validate color (basic hex color check)
    if !is_valid_hex_color(&language.color) {
        return Err(simple_error(format!(
            "invalid hex color: {}",
            language.color
        )));
    }

    // Validate icon (basic non-empty check)
    if language.icon.trim().is_empty() {
        log::warn!("Language '{}' has empty icon", language.name);
    }

    // Validate priority
    if language.priority == 0 {
        return Err(simple_error("priority must be greater than 0"));
    }

    // Validate frameworks
    validate_frameworks(&language.frameworks)?;

    Ok(())
}

/// Validate framework configurations
fn validate_frameworks(frameworks: &[FrameworkDetector]) -> Result<()> {
    let mut names = HashSet::new();

    for framework in frameworks {
        validate_framework(framework)?;

        // Check for duplicate framework names
        if !names.insert(&framework.name) {
            return Err(simple_error(format!(
                "duplicate framework name: {}",
                framework.name
            )));
        }
    }

    Ok(())
}

/// Validate a single framework configuration
fn validate_framework(framework: &FrameworkDetector) -> Result<()> {
    // Validate name
    if framework.name.trim().is_empty() {
        return Err(simple_error("framework name cannot be empty"));
    }

    // Validate priority
    if framework.priority == 0 {
        return Err(simple_error("framework priority must be greater than 0"));
    }

    // Validate detection configuration
    validate_detection_type(&framework.detection)?;

    // Validate color if provided
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

/// Validate detection type configuration
fn validate_detection_type(detection: &DetectionType) -> Result<()> {
    match detection {
        DetectionType::PackageJson { dependencies } => {
            validate_non_empty_vec(dependencies, "PackageJson dependencies")?;
        }
        DetectionType::CargoToml { dependencies } => {
            validate_non_empty_vec(dependencies, "CargoToml dependencies")?;
        }
        DetectionType::GoMod { modules } => {
            validate_non_empty_vec(modules, "GoMod modules")?;
        }
        DetectionType::PyProjectToml { dependencies } => {
            validate_non_empty_vec(dependencies, "PyProjectToml dependencies")?;
        }
        DetectionType::GemSpec { gems } => {
            validate_non_empty_vec(gems, "GemSpec gems")?;
        }
        DetectionType::ComposerJson { packages } => {
            validate_non_empty_vec(packages, "ComposerJson packages")?;
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

/// Validate that a vector is not empty and contains no empty strings
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

/// Validate unique language names (case-insensitive)
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

/// Check if a string is a valid hex color
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
        ProjectIndicator {
            name: "Test Language".to_string(),
            files: vec!["test.file".to_string()],
            color: "#FF0000".to_string(),
            icon: "🔥".to_string(),
            priority: 1,
            frameworks: vec![],
        }
    }

    fn create_valid_framework() -> FrameworkDetector {
        FrameworkDetector {
            name: "Test Framework".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["test-dep".to_string()],
            },
            icon: Some("⚡".to_string()),
            color: Some("#00FF00".to_string()),
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_validate_valid_config() {
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
    }

    #[test]
    fn test_validate_invalid_version() {
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
    }

    #[test]
    fn test_validate_empty_languages() {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_language_names() {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            cache: CacheConfig::default(),
            detection: DetectionConfig::default(),
            languages: vec![
                create_valid_language(),
                create_valid_language(), // Same name
            ],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_hex_color() {
        assert!(is_valid_hex_color("#FF0000"));
        assert!(is_valid_hex_color("#f0f"));
        assert!(is_valid_hex_color("#123ABC"));

        assert!(!is_valid_hex_color("FF0000")); // Missing #
        assert!(!is_valid_hex_color("#GG0000")); // Invalid hex
        assert!(!is_valid_hex_color("#FF00")); // Wrong length
        assert!(!is_valid_hex_color("#")); // Empty hex
    }

    #[test]
    fn test_validate_language_with_invalid_color() {
        let mut language = create_valid_language();
        language.color = "invalid-color".to_string();

        assert!(validate_language(&language).is_err());
    }

    #[test]
    fn test_validate_language_with_empty_files() {
        let mut language = create_valid_language();
        language.files = vec![];

        assert!(validate_language(&language).is_err());
    }

    #[test]
    fn test_validate_framework() {
        let framework = create_valid_framework();
        assert!(validate_framework(&framework).is_ok());
    }

    #[test]
    fn test_validate_framework_with_invalid_detection() {
        let framework = FrameworkDetector {
            name: "Test".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec![], // Empty dependencies
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        assert!(validate_framework(&framework).is_err());
    }

    #[test]
    fn test_validate_display_config() {
        let mut config = Config {
            languages: vec![create_valid_language()],
            ..Default::default()
        };

        // Valid config
        assert!(validate_config(&config).is_ok());

        // Invalid max_frameworks
        config.display.max_frameworks = 0;
        assert!(validate_config(&config).is_err());

        // Reset and test empty separator
        config.display.max_frameworks = 2;
        config.display.framework_separator = "".to_string();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_detection_types() {
        // Test all detection types with valid data
        let detection_types = vec![
            DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            DetectionType::CargoToml {
                dependencies: vec!["serde".to_string()],
            },
            DetectionType::GoMod {
                modules: vec!["github.com/gin-gonic/gin".to_string()],
            },
            DetectionType::PyProjectToml {
                dependencies: vec!["Django".to_string()],
            },
            DetectionType::GemSpec {
                gems: vec!["rails".to_string()],
            },
            DetectionType::ComposerJson {
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
    }
}
