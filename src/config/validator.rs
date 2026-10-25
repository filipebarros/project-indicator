use super::Config;
use crate::types::{DetectionType, Framework, Indicator, RootIndicator};
use anyhow::Result;
use std::collections::HashSet;
pub fn validate_config(config: &Config) -> Result<()> {
    validate_languages(&config.indicators)?;
    validate_frameworks(&config.frameworks)?;
    validate_unique_language_names(&config.indicators)?;

    Ok(())
}
fn validate_languages(languages: &[Indicator]) -> Result<()> {
    if languages.is_empty() {
        anyhow::bail!("Configuration must contain at least one language",);
    }

    for (i, language) in languages.iter().enumerate() {
        validate_language(language)
            .map_err(|e| anyhow::anyhow!("Language '{}' at index {}: {}", language.name, i, e))?;
    }

    Ok(())
}
fn validate_language(language: &Indicator) -> Result<()> {
    if language.name.trim().is_empty() {
        anyhow::bail!("name cannot be empty");
    }

    if language.files.is_empty() {
        anyhow::bail!("must have at least one file pattern");
    }

    for file_pattern in &language.files {
        if file_pattern.trim().is_empty() {
            anyhow::bail!("file pattern cannot be empty");
        }
    }

    if !is_valid_hex_color(&language.color) {
        anyhow::bail!("invalid hex color: {}", language.color);
    }

    if language.icon.trim().is_empty() {
        log::warn!("Language '{}' has empty icon", language.name);
    }
    reject_control_characters(&language.icon)?;

    validate_root_indicators(&language.root_indicators)
}

fn validate_root_indicators(root_indicators: &[RootIndicator]) -> Result<()> {
    for root in root_indicators {
        if root.pattern.trim().is_empty() {
            anyhow::bail!("root indicator pattern cannot be empty");
        }
        // NaN fails the range check too
        if !(0.0..=1.0).contains(&root.weight) {
            anyhow::bail!(
                "root indicator '{}' weight must be between 0 and 1, got {}",
                root.pattern,
                root.weight
            );
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
            anyhow::bail!(
                "duplicate framework '{}' with same detection type",
                framework.name
            );
        }
    }

    Ok(())
}
fn validate_framework(framework: &Framework) -> Result<()> {
    if framework.name.trim().is_empty() {
        anyhow::bail!("framework name cannot be empty");
    }

    if framework.priority == 0 {
        anyhow::bail!("framework priority must be greater than 0");
    }

    // Frameworks are only surfaced by indicators sharing an ecosystem
    if framework.ecosystems.is_empty() {
        anyhow::bail!(
            "framework '{}' needs at least one ecosystem, or no indicator can surface it",
            framework.name
        );
    }

    validate_detection_type(&framework.detection)?;

    if let Some(icon) = &framework.icon {
        reject_control_characters(icon)?;
    }

    if let Some(color) = &framework.color {
        if !is_valid_hex_color(color) {
            anyhow::bail!("invalid framework hex color: {}", color);
        }
    }

    Ok(())
}
/// Icons are printed straight into the prompt
fn reject_control_characters(icon: &str) -> Result<()> {
    if icon.chars().any(crate::output::render::is_unprintable) {
        anyhow::bail!("icon contains an unprintable character");
    }
    Ok(())
}
fn validate_detection_type(detection: &DetectionType) -> Result<()> {
    match detection {
        DetectionType::Dependencies(dependencies) => {
            validate_non_empty_vec(dependencies, "Dependencies")?;
        }
        DetectionType::FileExists(files) => {
            validate_non_empty_vec(files, "FileExists files")?;
        }
    }

    Ok(())
}
fn validate_non_empty_vec(vec: &[String], context: &str) -> Result<()> {
    if vec.is_empty() {
        anyhow::bail!("{} cannot be empty", context);
    }

    for item in vec {
        if item.trim().is_empty() {
            anyhow::bail!("{} cannot contain empty strings", context);
        }
    }

    Ok(())
}
fn validate_unique_language_names(languages: &[Indicator]) -> Result<()> {
    let mut names = HashSet::new();

    for language in languages {
        if !names.insert(language.name.to_lowercase()) {
            anyhow::bail!("duplicate language name: {}", language.name);
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
    use crate::types::{DetectionConfig, DisplayConfig, Ecosystem, IndicatorContext};

    fn create_valid_indicator() -> Indicator {
        Indicator::new(
            "Test Language".to_string(),
            vec!["test.file".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            vec![],
        )
    }

    fn create_valid_framework() -> Framework {
        Framework {
            name: "Test Framework".to_string(),
            ecosystems: vec![Ecosystem::Npm],
            detection: DetectionType::Dependencies(vec!["test-dep".to_string()]),
            icon: Some("⚡".to_string()),
            color: Some("#00FF00".to_string()),
            priority: 1,
        }
    }

    fn config_with(indicators: Vec<Indicator>, frameworks: Vec<Framework>) -> Config {
        Config {
            display: DisplayConfig::default(),
            detection: DetectionConfig::default(),
            frameworks,
            indicators,
        }
    }

    fn root_indicator(pattern: &str, weight: f32) -> RootIndicator {
        RootIndicator {
            pattern: pattern.to_string(),
            weight,
            context: IndicatorContext::default(),
            alternative_group: None,
        }
    }

    fn indicator_with_root(pattern: &str, weight: f32) -> Indicator {
        let mut indicator = create_valid_indicator();
        indicator.root_indicators = vec![root_indicator(pattern, weight)];
        indicator
    }

    /// The error must come from the rule under test, not from any rule
    fn assert_rejected<T: std::fmt::Debug>(result: Result<T>, reason: &str) {
        match result {
            Ok(value) => panic!("expected a rejection mentioning {reason:?}, got Ok({value:?})"),
            Err(e) => assert!(
                format!("{e:#}").contains(reason),
                "expected {reason:?} in: {e:#}"
            ),
        }
    }

    #[test]
    fn test_validate_accepts_a_valid_config_with_a_framework() {
        let config = config_with(
            vec![create_valid_indicator()],
            vec![create_valid_framework()],
        );

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_rejects_no_languages() {
        assert_rejected(
            validate_config(&config_with(vec![], vec![])),
            "at least one language",
        );
    }

    #[test]
    fn test_validate_rejects_duplicate_language_names_ignoring_case() {
        let mut shouting = create_valid_indicator();
        shouting.name = shouting.name.to_uppercase();
        let config = config_with(vec![create_valid_indicator(), shouting], vec![]);

        assert_rejected(validate_config(&config), "duplicate language name");
    }

    #[test]
    fn test_validate_names_the_failing_language_and_index() {
        let mut broken = create_valid_indicator();
        broken.name = "Broken".to_string();
        broken.color = "not-a-color".to_string();
        let config = config_with(vec![create_valid_indicator(), broken], vec![]);

        assert_rejected(validate_config(&config), "Language 'Broken' at index 1");
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
    fn test_validate_language_rejects_an_empty_name() {
        let mut language = create_valid_indicator();
        language.name = "  ".to_string();

        assert_rejected(validate_language(&language), "name cannot be empty");
    }

    #[test]
    fn test_validate_language_rejects_no_files() {
        let mut language = create_valid_indicator();
        language.files = vec![];

        assert_rejected(validate_language(&language), "at least one file pattern");
    }

    #[test]
    fn test_validate_language_rejects_a_blank_file_pattern() {
        let mut language = create_valid_indicator();
        language.files = vec!["ok.file".to_string(), " ".to_string()];

        assert_rejected(validate_language(&language), "file pattern cannot be empty");
    }

    #[test]
    fn test_validate_language_rejects_an_invalid_color() {
        let mut language = create_valid_indicator();
        language.color = "invalid-color".to_string();

        assert_rejected(validate_language(&language), "invalid hex color");
    }

    #[test]
    fn test_validate_accepts_root_weight_bounds() {
        assert!(validate_language(&indicator_with_root("a.toml", 0.0)).is_ok());
        assert!(validate_language(&indicator_with_root("a.toml", 1.0)).is_ok());
    }

    #[test]
    fn test_validate_rejects_negative_root_weight() {
        assert_rejected(
            validate_language(&indicator_with_root("a.toml", -0.5)),
            "weight must be between 0 and 1",
        );
    }

    #[test]
    fn test_validate_rejects_root_weight_above_one() {
        assert_rejected(
            validate_language(&indicator_with_root("a.toml", 5.0)),
            "weight must be between 0 and 1",
        );
    }

    #[test]
    fn test_validate_rejects_nan_root_weight() {
        assert_rejected(
            validate_language(&indicator_with_root("a.toml", f32::NAN)),
            "weight must be between 0 and 1",
        );
    }

    #[test]
    fn test_validate_rejects_empty_root_pattern() {
        assert_rejected(
            validate_language(&indicator_with_root("  ", 0.5)),
            "root indicator pattern cannot be empty",
        );
    }

    #[test]
    fn test_validate_framework_rejects_an_empty_name() {
        let mut framework = create_valid_framework();
        framework.name = String::new();

        assert_rejected(
            validate_framework(&framework),
            "framework name cannot be empty",
        );
    }

    #[test]
    fn test_validate_framework_rejects_priority_zero() {
        let mut framework = create_valid_framework();
        framework.priority = 0;

        assert_rejected(
            validate_framework(&framework),
            "framework priority must be greater than 0",
        );
    }

    #[test]
    fn test_validate_rejects_framework_without_ecosystems() {
        let mut framework = create_valid_framework();
        framework.ecosystems = vec![];

        assert_rejected(
            validate_framework(&framework),
            "needs at least one ecosystem",
        );
    }

    #[test]
    fn test_validate_framework_rejects_no_dependencies() {
        let mut framework = create_valid_framework();
        framework.detection = DetectionType::Dependencies(vec![]);

        assert_rejected(
            validate_framework(&framework),
            "Dependencies cannot be empty",
        );
    }

    #[test]
    fn test_validate_framework_rejects_a_blank_dependency() {
        let mut framework = create_valid_framework();
        framework.detection =
            DetectionType::Dependencies(vec!["react".to_string(), "".to_string()]);

        assert_rejected(
            validate_framework(&framework),
            "Dependencies cannot contain empty strings",
        );
    }

    #[test]
    fn test_validate_framework_rejects_no_probe_files() {
        let mut framework = create_valid_framework();
        framework.detection = DetectionType::FileExists(vec![]);

        assert_rejected(
            validate_framework(&framework),
            "FileExists files cannot be empty",
        );
    }

    #[test]
    fn test_validate_framework_rejects_an_invalid_color() {
        let mut framework = create_valid_framework();
        framework.color = Some("green".to_string());

        assert_rejected(
            validate_framework(&framework),
            "invalid framework hex color",
        );
    }

    #[test]
    fn test_validate_rejects_a_duplicate_framework_with_the_same_detection_type() {
        let config = config_with(
            vec![create_valid_indicator()],
            vec![create_valid_framework(), create_valid_framework()],
        );

        assert_rejected(
            validate_config(&config),
            "duplicate framework 'Test Framework'",
        );
    }

    #[test]
    fn test_validate_accepts_a_framework_name_used_by_two_detection_types() {
        let mut by_files = create_valid_framework();
        by_files.detection = DetectionType::FileExists(vec!["marker.conf".to_string()]);
        let config = config_with(
            vec![create_valid_indicator()],
            vec![create_valid_framework(), by_files],
        );

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_language_rejects_control_characters_in_the_icon() {
        let mut language = create_valid_indicator();
        language.icon = "\u{1b}]0;pwned\u{7}Z".to_string();

        assert_rejected(
            validate_language(&language),
            "icon contains an unprintable character",
        );
    }

    #[test]
    fn test_validate_language_rejects_bidirectional_overrides_in_the_icon() {
        let mut language = create_valid_indicator();
        language.icon = "a\u{202e}b".to_string();

        assert_rejected(
            validate_language(&language),
            "icon contains an unprintable character",
        );
    }

    #[test]
    fn test_validate_framework_rejects_control_characters_in_the_icon() {
        let mut framework = create_valid_framework();
        framework.icon = Some("\u{1b}[2J".to_string());

        assert_rejected(
            validate_framework(&framework),
            "icon contains an unprintable character",
        );
    }

    #[test]
    fn test_the_built_in_config_round_trips_and_validates() -> Result<(), Box<dyn std::error::Error>>
    {
        let config = crate::config::built_in_config();
        let text = toml::to_string_pretty(&config)?;
        let reparsed = crate::config::parser::parse_toml_content(&text)?;
        assert_eq!(reparsed.indicators.len(), config.indicators.len());
        assert_eq!(reparsed.frameworks.len(), config.frameworks.len());
        validate_config(&reparsed)?;
        Ok(())
    }
}
