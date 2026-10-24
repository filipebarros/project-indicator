use crate::output::render::{render_debug, render_full, render_json, render_simple};
use crate::types::{DetectionResult, DisplayConfig};
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Simple,
    Full,
    Json,
    Debug,
}

pub fn render(result: &DetectionResult, format: OutputFormat, config: &DisplayConfig) -> String {
    // `show_frameworks = false` hides frameworks from every user-facing
    // format (icon/color selection included); debug stays an unfiltered dump
    let hidden;
    let user_facing = if config.show_frameworks || result.frameworks.is_empty() {
        result
    } else {
        // user-facing formats never read the evidence, so don't copy it
        hidden = DetectionResult::new(result.indicator.clone(), Vec::new(), result.confidence);
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

    #[test]
    fn test_simple_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();

        let output = render(&result, OutputFormat::Simple, &display_config);
        assert_eq!(output, "\x1b[38;2;97;218;251m⚛️\x1b[0m");
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
    fn test_show_frameworks_false_hides_frameworks_everywhere() {
        let result = create_test_result();
        let off = DisplayConfig {
            show_frameworks: false,
            ..DisplayConfig::default()
        };
        let on = DisplayConfig::default();

        // JSON lists no frameworks (nor their evidence)
        let json = render(&result, OutputFormat::Json, &off);
        assert!(json.contains("\"frameworks\":[]"), "{json}");
        assert!(render(&result, OutputFormat::Json, &on).contains("React"));

        // The prompt glyph falls back to the language, not the framework
        assert_ne!(
            render(&result, OutputFormat::Simple, &off),
            render(&result, OutputFormat::Simple, &on)
        );

        // Debug is a diagnostic dump: always complete
        assert!(render(&result, OutputFormat::Debug, &off).contains("React"));
    }
}
