//! Advanced output formatting for detection results

use crate::output::themes::ColorTheme;
use crate::types::{DetectionResult, DisplayConfig};
use serde::{Deserialize, Serialize};
use serde_json;

/// Output format options
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    /// Simple icon only (for shell prompts)
    Simple,
    /// Full format with icon and color
    Full,
    /// JSON format for scripting
    Json,
    /// Compact format for status bars
    Compact,
    /// Debug format with detailed information
    Debug,
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
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

/// JSON output structure for scripting
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
    pub language: Option<String>,
    pub frameworks: Vec<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

/// Main output formatter
pub struct OutputFormatter {
    display_config: DisplayConfig,
    theme: Box<dyn ColorTheme>,
}

impl OutputFormatter {
    /// Create a new formatter with configuration and theme
    pub fn new(display_config: DisplayConfig, theme: Box<dyn ColorTheme>) -> Self {
        Self {
            display_config,
            theme,
        }
    }

    /// Format detection results according to the specified format
    pub fn format(&self, result: &DetectionResult, format: OutputFormat) -> String {
        match format {
            OutputFormat::Simple => self.format_simple(result),
            OutputFormat::Full => self.format_full(result),
            OutputFormat::Json => self.format_json(result),
            OutputFormat::Compact => self.format_compact(result),
            OutputFormat::Debug => self.format_debug(result),
        }
    }

    /// Simple format: just the icon
    fn format_simple(&self, result: &DetectionResult) -> String {
        result.display_icon().unwrap_or("").to_string()
    }

    /// Full format: icon with optional frameworks and color codes
    fn format_full(&self, result: &DetectionResult) -> String {
        let icon = result.display_icon().unwrap_or("");
        let color = result.display_color().unwrap_or("");

        if self.display_config.show_frameworks && !result.frameworks.is_empty() {
            let frameworks = self.format_frameworks(result);
            if !frameworks.is_empty() {
                format!("{}{}|{}", icon, frameworks, color)
            } else {
                format!("{}|{}", icon, color)
            }
        } else {
            format!("{}|{}", icon, color)
        }
    }

    /// JSON format: structured data for scripting
    fn format_json(&self, result: &DetectionResult) -> String {
        let frameworks: Vec<String> = result
            .frameworks
            .iter()
            .take(self.display_config.max_frameworks)
            .map(|f| f.framework.name.clone())
            .collect();

        let evidence: Vec<String> = result
            .frameworks
            .iter()
            .flat_map(|f| f.evidence.iter())
            .cloned()
            .collect();

        let json_output = JsonOutput {
            language: result.language.as_ref().map(|l| l.name.clone()),
            frameworks,
            icon: result.display_icon().map(String::from),
            color: result.display_color().map(String::from),
            confidence: result.confidence,
            evidence,
        };

        serde_json::to_string(&json_output).unwrap_or_else(|_| "{}".to_string())
    }

    /// Compact format: language+frameworks (e.g., "React+TS")
    fn format_compact(&self, result: &DetectionResult) -> String {
        let mut parts = Vec::new();

        // Add best framework name or language name
        if let Some(framework) = result.best_framework() {
            parts.push(self.compact_name(&framework.framework.name));
        } else if let Some(language) = &result.language {
            parts.push(self.compact_name(&language.name));
        }

        // Add language abbreviation if we have a framework
        if result.best_framework().is_some() {
            if let Some(language) = &result.language {
                let lang_abbrev = self.compact_name(&language.name);
                if lang_abbrev.len() <= 3 {
                    parts.push(lang_abbrev);
                }
            }
        }

        parts.join(&self.display_config.framework_separator)
    }

    /// Debug format: detailed information
    fn format_debug(&self, result: &DetectionResult) -> String {
        let mut debug_info = Vec::new();

        if let Some(language) = &result.language {
            debug_info.push(format!(
                "Language: {} (priority: {})",
                language.name, language.priority
            ));
        }

        if !result.frameworks.is_empty() {
            debug_info.push("Frameworks:".to_string());
            for framework_match in &result.frameworks {
                debug_info.push(format!(
                    "  - {} (confidence: {:.2}, priority: {}, evidence: {:?})",
                    framework_match.framework.name,
                    framework_match.confidence,
                    framework_match.framework.priority,
                    framework_match.evidence
                ));
            }
        }

        debug_info.push(format!("Overall confidence: {:.2}", result.confidence));

        debug_info.join("\n")
    }

    /// Format frameworks for display
    fn format_frameworks(&self, result: &DetectionResult) -> String {
        if result.frameworks.is_empty() {
            return String::new();
        }

        let framework_names: Vec<String> = result
            .frameworks
            .iter()
            .take(self.display_config.max_frameworks)
            .map(|f| f.framework.name.clone())
            .collect();

        if framework_names.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                framework_names.join(&self.display_config.framework_separator)
            )
        }
    }

    /// Create compact name (abbreviations for common frameworks/languages)
    fn compact_name(&self, name: &str) -> String {
        match name {
            "TypeScript" => "TS".to_string(),
            "JavaScript" => "JS".to_string(),
            "Python" => "PY".to_string(),
            "Rust" => "RS".to_string(),
            "Go" => "GO".to_string(),
            "C++" => "CPP".to_string(),
            "C#" => "CS".to_string(),
            "Ruby" => "RB".to_string(),
            "PHP" => "PHP".to_string(),
            "Java" => "JAVA".to_string(),
            "Swift" => "SWIFT".to_string(),
            "Kotlin" => "KT".to_string(),
            "Next.js" => "Next".to_string(),
            "React" => "React".to_string(),
            "Vue" => "Vue".to_string(),
            "Angular" => "Ng".to_string(),
            "Svelte" => "Svelte".to_string(),
            _ => name.to_string(),
        }
    }

    /// Apply color theme to text (if supported by terminal)
    pub fn colorize(&self, text: &str, color: &str) -> String {
        self.theme.apply_color(text, color)
    }
}

/// Legacy function for backward compatibility
pub fn format_result(result: &DetectionResult, format: OutputFormat) -> String {
    let display_config = DisplayConfig::default();
    let theme = Box::new(crate::output::themes::DefaultTheme);
    let formatter = OutputFormatter::new(display_config, theme);
    formatter.format(result, format)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch, ProjectIndicator};

    fn create_test_result() -> DetectionResult {
        let language = ProjectIndicator {
            name: "TypeScript".to_string(),
            files: vec!["package.json".to_string()],
            color: "#3178C6".to_string(),
            icon: "󰛦".to_string(),
            priority: 1,
            frameworks: vec![],
        };

        let framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: Some("⚛️".to_string()),
            color: Some("#61DAFB".to_string()),
            priority: 1,
            files: vec![],
        };

        let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

        DetectionResult::new(Some(language), vec![framework_match], 0.9)
    }

    #[test]
    fn test_output_format_parsing() {
        assert_eq!(
            "simple".parse::<OutputFormat>().unwrap(),
            OutputFormat::Simple
        );
        assert_eq!("full".parse::<OutputFormat>().unwrap(), OutputFormat::Full);
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "compact".parse::<OutputFormat>().unwrap(),
            OutputFormat::Compact
        );
        assert_eq!(
            "debug".parse::<OutputFormat>().unwrap(),
            OutputFormat::Debug
        );

        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_simple_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let output = formatter.format(&result, OutputFormat::Simple);
        assert_eq!(output, "⚛️"); // Should prefer framework icon
    }

    #[test]
    fn test_full_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let output = formatter.format(&result, OutputFormat::Full);
        assert!(output.contains("⚛️"));
        assert!(output.contains("#61DAFB"));
        assert!(output.contains("React"));
    }

    #[test]
    fn test_json_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let output = formatter.format(&result, OutputFormat::Json);

        let json: JsonOutput = serde_json::from_str(&output).unwrap();
        assert_eq!(json.language, Some("TypeScript".to_string()));
        assert_eq!(json.frameworks, vec!["React".to_string()]);
        assert_eq!(json.icon, Some("⚛️".to_string()));
        assert_eq!(json.color, Some("#61DAFB".to_string()));
        assert_eq!(json.confidence, 0.9);
    }

    #[test]
    fn test_compact_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let output = formatter.format(&result, OutputFormat::Compact);
        assert_eq!(output, "React+TS");
    }

    #[test]
    fn test_debug_format() {
        let result = create_test_result();
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let output = formatter.format(&result, OutputFormat::Debug);
        assert!(output.contains("Language: TypeScript"));
        assert!(output.contains("React"));
        assert!(output.contains("confidence: 0.90"));
        assert!(output.contains("evidence"));
    }

    #[test]
    fn test_compact_names() {
        let display_config = DisplayConfig::default();
        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        assert_eq!(formatter.compact_name("TypeScript"), "TS");
        assert_eq!(formatter.compact_name("JavaScript"), "JS");
        assert_eq!(formatter.compact_name("React"), "React");
        assert_eq!(formatter.compact_name("Next.js"), "Next");
        assert_eq!(formatter.compact_name("CustomFramework"), "CustomFramework");
    }

    #[test]
    fn test_framework_limiting() {
        let mut result = create_test_result();

        // Add more frameworks
        let vue_framework = FrameworkDetector {
            name: "Vue".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec![],
            },
            icon: None,
            color: None,
            priority: 2,
            files: vec![],
        };

        result
            .frameworks
            .push(FrameworkMatch::new(vue_framework, 0.8, vec![]));

        let display_config = DisplayConfig {
            max_frameworks: 1, // Limit to 1 framework
            ..Default::default()
        };

        let theme = Box::new(crate::output::themes::DefaultTheme);
        let formatter = OutputFormatter::new(display_config, theme);

        let json_output = formatter.format(&result, OutputFormat::Json);
        let json: JsonOutput = serde_json::from_str(&json_output).unwrap();
        assert_eq!(json.frameworks.len(), 1); // Should be limited to 1
    }
}
