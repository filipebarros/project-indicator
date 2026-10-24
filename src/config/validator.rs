use super::{Config, ConfigError};
#[cfg(test)]
use crate::types::DetectionConfig;
use crate::types::{DetectionType, Framework, Indicator, RootIndicator};
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
    // No version gate: deny_unknown_fields enforces the schema structurally,
    // and the detect path accepts (with a warning) any config that parses —
    // validate must not reject a config detection is actively using
    if config.display.max_frameworks == 0 {
        return Err(validation_error("max_frameworks must be greater than 0"));
    }
    validate_languages(&config.indicators)?;
    validate_frameworks(&config.frameworks)?;
    validate_unique_language_names(&config.indicators)?;

    Ok(())
}
fn validate_languages(languages: &[Indicator]) -> Result<()> {
    if languages.is_empty() {
        return Err(validation_error(
            "Configuration must contain at least one language",
        ));
    }

    for (i, language) in languages.iter().enumerate() {
        validate_language(language).map_err(|e| {
            validation_error(format!(
                "Language '{}' at index {}: {}",
                language.name, i, e
            ))
        })?;
    }

    Ok(())
}
fn validate_language(language: &Indicator) -> Result<()> {
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

    validate_root_indicators(&language.root_indicators)
}

fn validate_root_indicators(root_indicators: &[RootIndicator]) -> Result<()> {
    for root in root_indicators {
        if root.pattern.trim().is_empty() {
            return Err(simple_error("root indicator pattern cannot be empty"));
        }
        // NaN fails the range check too
        if !(0.0..=1.0).contains(&root.weight) {
            return Err(simple_error(format!(
                "root indicator '{}' weight must be between 0 and 1, got {}",
                root.pattern, root.weight
            )));
        }
    }

    Ok(())
}
fn validate_frameworks(frameworks: &[Framework]) -> Result<()> {
    let mut seen = HashSet::new();

    for framework in frameworks {
        validate_framework(framework)?;

        let key = (
            framework.name.clone(),
            std::mem::discriminant(&framework.detection),
        );
        if !seen.insert(key) {
            return Err(simple_error(format!(
                "duplicate framework '{}' with same detection type",
                framework.name
            )));
        }
    }

    Ok(())
}
fn validate_framework(framework: &Framework) -> Result<()> {
    if framework.name.trim().is_empty() {
        return Err(simple_error("framework name cannot be empty"));
    }

    if framework.priority == 0 {
        return Err(simple_error("framework priority must be greater than 0"));
    }

    // Frameworks are only surfaced by indicators sharing an ecosystem
    if framework.ecosystems.is_empty() {
        return Err(simple_error(format!(
            "framework '{}' needs at least one ecosystem, or no indicator can surface it",
            framework.name
        )));
    }

    validate_detection_type(&framework.detection)?;
    validate_root_indicators(&framework.root_indicators)?;

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
        DetectionType::Dependencies { dependencies } => {
            validate_non_empty_vec(dependencies, "Dependencies")?;
        }
        DetectionType::FileExists { files } => {
            validate_non_empty_vec(files, "FileExists files")?;
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
fn validate_unique_language_names(languages: &[Indicator]) -> Result<()> {
    let mut names = HashSet::new();

    for language in languages {
        if !names.insert(language.name.to_lowercase()) {
            return Err(validation_error(format!(
                "duplicate language name: {}",
                language.name
            )));
        }
    }

    Ok(())
}
fn is_valid_hex_color(color: &str) -> bool {
    color
        .strip_prefix('#')
        .is_some_and(|hex| matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConfigMeta, DisplayConfig};

    fn create_valid_indicator() -> Indicator {
        Indicator::new(
            "Test Language".to_string(),
            vec!["test.file".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        )
    }

    fn create_valid_framework() -> Framework {
        Framework {
            name: "Test Framework".to_string(),
            ecosystems: vec![crate::types::Ecosystem::Npm],
            detection: DetectionType::Dependencies {
                dependencies: vec!["test-dep".to_string()],
            },
            icon: Some("⚡".to_string()),
            color: Some("#00FF00".to_string()),
            priority: 1,
            root_indicators: vec![],
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config {
            meta: ConfigMeta {
                version: "3.0".to_string(),
            },
            display: DisplayConfig::default(),
            detection: DetectionConfig::default(),
            frameworks: vec![],
            indicators: vec![create_valid_indicator()],
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_accepts_non_current_version() {
        // Detection accepts (with a parser warning) any config that parses;
        // validate must agree instead of rejecting a config that is in use.
        // deny_unknown_fields already guards real schema mismatches.
        let config = Config {
            meta: ConfigMeta {
                version: "1.0".to_string(),
            },
            display: DisplayConfig::default(),
            detection: DetectionConfig::default(),
            frameworks: vec![],
            indicators: vec![create_valid_indicator()],
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_empty_languages() {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            detection: DetectionConfig::default(),
            frameworks: vec![],
            indicators: vec![],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_language_names() {
        let config = Config {
            meta: ConfigMeta::default(),
            display: DisplayConfig::default(),
            detection: DetectionConfig::default(),
            frameworks: vec![],
            indicators: vec![create_valid_indicator(), create_valid_indicator()],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_hex_color() {
        assert!(is_valid_hex_color("#FF0000"));
        assert!(is_valid_hex_color("#f0f"));
        assert!(is_valid_hex_color("#123ABC"));

        assert!(!is_valid_hex_color("FF0000"));
        assert!(!is_valid_hex_color("#GG0000"));
        assert!(!is_valid_hex_color("#FF00"));
        assert!(!is_valid_hex_color("#"));
    }

    #[test]
    fn test_validate_language_with_invalid_color() {
        let mut language = create_valid_indicator();
        language.color = "invalid-color".to_string();

        assert!(validate_language(&language).is_err());
    }

    #[test]
    fn test_validate_language_with_empty_files() {
        let mut language = create_valid_indicator();
        language.files = vec![];

        assert!(validate_language(&language).is_err());
    }

    fn indicator_with_root(pattern: &str, weight: f32) -> Indicator {
        Indicator::with_root_indicators(
            "Test Language".to_string(),
            vec!["test.file".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![crate::types::RootIndicator {
                pattern: pattern.to_string(),
                weight,
                context: crate::types::IndicatorContext::default(),
                alternative_group: None,
            }],
        )
    }

    #[test]
    fn test_validate_accepts_root_weight_bounds() {
        assert!(validate_language(&indicator_with_root("a.toml", 0.0)).is_ok());
        assert!(validate_language(&indicator_with_root("a.toml", 1.0)).is_ok());
    }

    #[test]
    fn test_validate_rejects_negative_root_weight() {
        assert!(validate_language(&indicator_with_root("a.toml", -0.5)).is_err());
    }

    #[test]
    fn test_validate_rejects_root_weight_above_one() {
        assert!(validate_language(&indicator_with_root("a.toml", 5.0)).is_err());
    }

    #[test]
    fn test_validate_rejects_nan_root_weight() {
        assert!(validate_language(&indicator_with_root("a.toml", f32::NAN)).is_err());
    }

    #[test]
    fn test_validate_rejects_empty_root_pattern() {
        assert!(validate_language(&indicator_with_root("  ", 0.5)).is_err());
    }

    #[test]
    fn test_validate_rejects_framework_without_ecosystems() {
        let mut framework = create_valid_framework();
        framework.ecosystems = vec![];
        assert!(validate_framework(&framework).is_err());
    }

    #[test]
    fn test_validate_rejects_framework_root_indicator_with_bad_weight() {
        let mut framework = create_valid_framework();
        framework.root_indicators = vec![crate::types::RootIndicator {
            pattern: "x".to_string(),
            weight: 2.0,
            context: crate::types::IndicatorContext::default(),
            alternative_group: None,
        }];
        assert!(validate_framework(&framework).is_err());
    }

    #[test]
    fn test_every_builtin_template_round_trips_and_validates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (name, _) in crate::config::TemplateGenerator::TEMPLATES {
            let config = crate::config::TemplateGenerator::generate_template(Some(name))?;
            let text = toml::to_string_pretty(&config)?;
            let reparsed = crate::config::ConfigParser::parse_toml_content(&text)
                .map_err(|e| format!("{name}: {e:#}"))?;
            assert_eq!(reparsed.indicators.len(), config.indicators.len(), "{name}");
            assert_eq!(reparsed.frameworks.len(), config.frameworks.len(), "{name}");
            validate_config(&reparsed).map_err(|e| format!("{name}: {e:#}"))?;
        }
        Ok(())
    }

    #[test]
    fn test_validate_framework() {
        let framework = create_valid_framework();
        assert!(validate_framework(&framework).is_ok());
    }

    #[test]
    fn test_validate_framework_with_invalid_detection() {
        let framework = Framework {
            name: "Test".to_string(),
            ecosystems: vec![],
            detection: DetectionType::Dependencies {
                dependencies: vec![],
            },
            icon: None,
            color: None,
            priority: 1,
            root_indicators: vec![],
        };

        assert!(validate_framework(&framework).is_err());
    }

    #[test]
    fn test_validate_display_config() {
        let mut config = Config {
            frameworks: vec![],
            indicators: vec![create_valid_indicator()],
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());

        config.display.max_frameworks = 0;
        assert!(validate_config(&config).is_err());
    }
}
