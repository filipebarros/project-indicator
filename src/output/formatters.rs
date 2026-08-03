use crate::output::render::{
    CompactRenderer, DebugRenderer, FullRenderer, JsonRenderer, Render, SimpleRenderer,
};
use crate::output::rich::RichFormatter;
use crate::types::{DetectionResult, DisplayConfig};
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Simple,
    Full,
    Json,
    Compact,
    Debug,
    Rich,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(OutputFormat::Simple),
            "full" => Ok(OutputFormat::Full),
            "json" => Ok(OutputFormat::Json),
            "compact" => Ok(OutputFormat::Compact),
            "debug" => Ok(OutputFormat::Debug),
            "rich" => Ok(OutputFormat::Rich),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}
pub struct OutputFormatter {
    display_config: DisplayConfig,
}

impl OutputFormatter {
    pub fn new(display_config: DisplayConfig) -> Self {
        Self { display_config }
    }
    pub fn format(&self, result: &DetectionResult, format: OutputFormat) -> String {
        match format {
            OutputFormat::Simple => SimpleRenderer.render(result, &self.display_config),
            OutputFormat::Full => FullRenderer.render(result, &self.display_config),
            OutputFormat::Json => JsonRenderer.render(result, &self.display_config),
            OutputFormat::Compact => CompactRenderer.render(result, &self.display_config),
            OutputFormat::Debug => DebugRenderer.render(result, &self.display_config),
            OutputFormat::Rich => self.format_rich(result),
        }
    }
    fn format_rich(&self, result: &DetectionResult) -> String {
        let rich_formatter = RichFormatter::new(self.display_config.clone());
        rich_formatter.create_detailed_table(result)
    }
}

pub fn format_result(result: &DetectionResult, format: OutputFormat) -> String {
    let display_config = DisplayConfig::default();
    let formatter = OutputFormatter::new(display_config);
    formatter.format(result, format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_result;
    use crate::types::{DetectionType, Framework, FrameworkMatch};

    #[derive(Debug, Serialize, Deserialize)]
    struct JsonOutput {
        pub indicator: Option<String>,
        pub frameworks: Vec<String>,
        pub icon: Option<String>,
        pub color: Option<String>,
        pub confidence: f32,
        pub evidence: Vec<String>,
    }

    #[test]
    fn test_output_format_parsing() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!("simple".parse::<OutputFormat>()?, OutputFormat::Simple);
        assert_eq!("full".parse::<OutputFormat>()?, OutputFormat::Full);
        assert_eq!("json".parse::<OutputFormat>()?, OutputFormat::Json);
        assert_eq!("compact".parse::<OutputFormat>()?, OutputFormat::Compact);
        assert_eq!("debug".parse::<OutputFormat>()?, OutputFormat::Debug);
        assert_eq!("rich".parse::<OutputFormat>()?, OutputFormat::Rich);

        assert!("invalid".parse::<OutputFormat>().is_err());
        Ok(())
    }

    #[test]
    fn test_simple_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Simple);
        assert_eq!(output, "⚛️");
        Ok(())
    }

    #[test]
    fn test_full_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Full);
        assert!(output.contains("󰛦"));
        assert!(output.contains("#3178C6"));
        assert!(output.contains("⚛️"));
        assert!(output.contains("#61DAFB"));
        assert!(output.contains("•"));
        assert!(output.contains("|"));
        Ok(())
    }

    #[test]
    fn test_json_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Json);

        let json: JsonOutput = serde_json::from_str(&output)?;
        assert_eq!(json.indicator, Some("TypeScript".to_string()));
        assert_eq!(json.frameworks, vec!["React".to_string()]);
        assert_eq!(json.icon, Some("⚛️".to_string()));
        assert_eq!(json.color, Some("#61DAFB".to_string()));
        assert_eq!(json.confidence, 0.9);
        Ok(())
    }

    #[test]
    fn test_compact_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Compact);
        assert_eq!(output, "React+TS");
        Ok(())
    }

    #[test]
    fn test_debug_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Debug);
        assert!(output.contains("Project: TypeScript"));
        assert!(output.contains("React"));
        assert!(output.contains("confidence: 0.90"));
        assert!(output.contains("evidence"));
        Ok(())
    }

    #[test]
    fn test_rich_format() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let formatter = OutputFormatter::new(display_config);

        let output = formatter.format(&result, OutputFormat::Rich);
        assert!(output.contains("Project Analysis"));
        assert!(output.contains("TypeScript"));
        assert!(output.contains("React"));
        assert!(output.contains("Frameworks Detected"));
        Ok(())
    }

    #[test]
    fn test_compact_names() -> Result<(), Box<dyn std::error::Error>> {
        let renderer = CompactRenderer;

        assert_eq!(renderer.compact_name("TypeScript"), "TS");
        assert_eq!(renderer.compact_name("JavaScript"), "JS");
        assert_eq!(renderer.compact_name("React"), "React");
        assert_eq!(renderer.compact_name("Next.js"), "Next");
        assert_eq!(renderer.compact_name("CustomFramework"), "CustomFramework");
        Ok(())
    }

    #[test]
    fn test_framework_limiting() -> Result<(), Box<dyn std::error::Error>> {
        let mut result = create_test_result();

        let vue_framework = Framework {
            name: "Vue".to_string(),
            ecosystems: vec![],
            detection: DetectionType::Dependencies {
                dependencies: vec![],
            },
            icon: None,
            color: None,
            priority: 2,
            files: vec![],
            root_indicators: vec![],
        };

        result
            .frameworks
            .push(FrameworkMatch::new(vue_framework, 0.8, vec![]));

        let display_config = DisplayConfig {
            max_frameworks: 1,
            ..Default::default()
        };

        let formatter = OutputFormatter::new(display_config);

        let json_output = formatter.format(&result, OutputFormat::Json);
        let json: JsonOutput = serde_json::from_str(&json_output)?;
        assert_eq!(json.frameworks.len(), 1);
        Ok(())
    }

    #[test]
    fn test_json_format_error_resilience() -> Result<(), Box<dyn std::error::Error>> {
        let mut result = create_test_result();

        result.confidence = f32::NAN;
        let formatter = OutputFormatter::new(DisplayConfig::default());

        let json_output = formatter.format(&result, OutputFormat::Json);
        let parsed: JsonOutput = serde_json::from_str(&json_output)?;

        assert_eq!(parsed.confidence, 0.0);
        assert!(parsed.indicator.is_some());

        result.confidence = f32::INFINITY;
        let json_output = formatter.format(&result, OutputFormat::Json);
        let parsed: JsonOutput = serde_json::from_str(&json_output)?;

        assert_eq!(parsed.confidence, 0.0);
        Ok(())
    }

    #[test]
    fn test_json_format_fallback_structure() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let formatter = OutputFormatter::new(DisplayConfig::default());

        let json_output = formatter.format(&result, OutputFormat::Json);
        let json: JsonOutput = serde_json::from_str(&json_output)?;

        assert!(json.indicator.is_some());
        assert!(!json.frameworks.is_empty());
        assert!(json.confidence >= 0.0);
        assert!(!json.evidence.is_empty());

        assert!(json_output.starts_with('{'));
        assert!(json_output.ends_with('}'));
        Ok(())
    }
}
