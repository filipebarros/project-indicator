use crate::output::render::{render_debug, render_full, render_json, render_simple};
use crate::types::{DetectionResult, DisplayConfig, NestedProject};
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Simple,
    Full,
    Json,
    Debug,
}

pub fn render(result: &DetectionResult, format: OutputFormat, config: &DisplayConfig) -> String {
    // `max_frameworks = 0` hides frameworks from every user-facing
    // format (icon/color selection included); debug stays an unfiltered dump
    let hidden;
    let has_frameworks =
        !result.frameworks.is_empty() || result.nested.iter().any(|p| !p.frameworks.is_empty());
    let user_facing = if config.max_frameworks > 0 || !has_frameworks {
        result
    } else {
        // user-facing formats never read the evidence, so don't copy it
        let mut copy =
            DetectionResult::new(result.indicator.clone(), Vec::new(), result.confidence);
        copy.nested = result
            .nested
            .iter()
            .map(|project| NestedProject {
                frameworks: Vec::new(),
                ..project.clone()
            })
            .collect();
        hidden = copy;
        &hidden
    };

    match format {
        OutputFormat::Simple => render_simple(user_facing),
        OutputFormat::Full => render_full(user_facing, config),
        OutputFormat::Json => render_json(user_facing, config),
        OutputFormat::Debug => render_debug(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_result;

    fn expected_simple(icon: &str, rgb: (u8, u8, u8)) -> String {
        if std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()) {
            format!("\x1b[38;2;{};{};{}m{icon}\x1b[0m", rgb.0, rgb.1, rgb.2)
        } else {
            icon.to_string()
        }
    }

    #[test]
    fn test_simple_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();

        let output = render(&result, OutputFormat::Simple, &display_config);
        assert_eq!(output, expected_simple("⚛️", (97, 218, 251)));
    }

    #[test]
    fn test_full_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();

        let output = render(&result, OutputFormat::Full, &display_config);
        assert!(output.contains("󰛦"));
        assert!(output.contains("#3178C6"));
        assert!(output.contains("⚛️"));
        assert!(output.contains("#61DAFB"));
        assert!(output.contains("•"));
        assert!(output.contains("|"));
    }

    #[test]
    fn test_json_format_error_resilience() -> Result<(), Box<dyn std::error::Error>> {
        let mut result = create_test_result();

        result.confidence = f32::NAN;

        let json_output = render(&result, OutputFormat::Json, &DisplayConfig::default());
        let parsed: serde_json::Value = serde_json::from_str(&json_output)?;

        assert_eq!(parsed["confidence"], 0.0);
        assert!(!parsed["indicator"].is_null());

        result.confidence = f32::INFINITY;
        let json_output = render(&result, OutputFormat::Json, &DisplayConfig::default());
        let parsed: serde_json::Value = serde_json::from_str(&json_output)?;

        assert_eq!(parsed["confidence"], 0.0);
        Ok(())
    }

    #[test]
    fn test_zero_max_frameworks_hides_frameworks_everywhere() {
        let result = create_test_result();
        let off = DisplayConfig { max_frameworks: 0 };
        let on = DisplayConfig::default();

        let json = render(&result, OutputFormat::Json, &off);
        assert!(json.contains("\"frameworks\":[]"), "{json}");
        assert!(render(&result, OutputFormat::Json, &on).contains("React"));

        assert_ne!(
            render(&result, OutputFormat::Simple, &off),
            render(&result, OutputFormat::Simple, &on)
        );
        let language_only =
            DetectionResult::new(result.indicator.clone(), Vec::new(), result.confidence);
        assert_eq!(
            render(&result, OutputFormat::Simple, &off),
            render(&language_only, OutputFormat::Simple, &on)
        );

        assert!(render(&result, OutputFormat::Debug, &off).contains("React"));
    }

    #[test]
    fn test_hiding_frameworks_hides_them_inside_subprojects_too() {
        use crate::types::{Framework, FrameworkMatch, Indicator, NestedProject};
        use std::sync::Arc;
        let mut result = DetectionResult::new(None, vec![], 0.0);
        result.nested = vec![NestedProject {
            name: "frontend".to_string(),
            indicator: Arc::new(Indicator::new(
                "JavaScript".to_string(),
                vec![],
                "#f7df1e".to_string(),
                "J".to_string(),
                vec![],
            )),
            frameworks: vec![FrameworkMatch::new(
                Framework {
                    name: "React".to_string(),
                    ecosystems: vec![],
                    detection: crate::types::DetectionType::Dependencies(vec![]),
                    icon: Some("⚛".to_string()),
                    color: Some("#61dafb".to_string()),
                    priority: 1,
                },
                1.0,
                vec![],
            )],
        }];
        let config = DisplayConfig { max_frameworks: 0 };

        assert_eq!(render(&result, OutputFormat::Full, &config), "J|#f7df1e");
        assert_eq!(
            render(&result, OutputFormat::Simple, &config),
            expected_simple("J", (247, 223, 30)),
            "simple must show the language glyph, not React's"
        );
        let json: serde_json::Value =
            serde_json::from_str(&render(&result, OutputFormat::Json, &config)).unwrap_or_default();
        assert_eq!(json["projects"][0]["frameworks"], serde_json::json!([]));
        assert_eq!(json["projects"][0]["icon"], "J");
    }
}
