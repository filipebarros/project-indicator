use crate::types::{DetectionResult, DisplayConfig};

pub trait Render {
    fn render(&self, result: &DetectionResult, config: &DisplayConfig) -> String;
}

pub struct SimpleRenderer;

impl Render for SimpleRenderer {
    fn render(&self, result: &DetectionResult, _config: &DisplayConfig) -> String {
        let icon = result.display_icon().unwrap_or("");
        if icon.is_empty() {
            return String::new();
        }

        // Color the icon with ANSI truecolor so prompts show the configured
        // color. NO_COLOR (https://no-color.org) disables it.
        if std::env::var_os("NO_COLOR").is_none() {
            if let Some((r, g, b)) = result.display_color().and_then(hex_to_rgb) {
                return format!("\x1b[38;2;{r};{g};{b}m{icon}\x1b[0m");
            }
        }

        icon.into()
    }
}

/// Parse a `#rrggbb` hex color into RGB components. Returns `None` for any
/// other shape (shorthand `#fff`, missing `#`, non-hex digits).
fn hex_to_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
    let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
    Some((r, g, b))
}

pub struct FullRenderer;

impl Render for FullRenderer {
    fn render(&self, result: &DetectionResult, config: &DisplayConfig) -> String {
        let mut components = Vec::new();

        if let Some(language) = &result.indicator {
            components.push(format!("{}|{}", language.icon, language.color));
        }

        if config.show_frameworks && !result.frameworks.is_empty() {
            for framework_match in result.frameworks.iter().take(config.max_frameworks) {
                let fw_icon = framework_match.framework.icon.as_deref().unwrap_or("");
                let fw_color = framework_match.framework.color.as_deref().unwrap_or("");

                if !fw_icon.is_empty() {
                    components.push(format!("{}|{}", fw_icon, fw_color));
                }
            }
        }

        components.join("•")
    }
}

pub struct JsonRenderer;

impl Render for JsonRenderer {
    fn render(&self, result: &DetectionResult, config: &DisplayConfig) -> String {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct JsonOutput {
            indicator: Option<String>,
            frameworks: Vec<String>,
            icon: Option<String>,
            color: Option<String>,
            confidence: f32,
            evidence: Vec<String>,
        }

        let frameworks: Vec<&str> = result
            .frameworks
            .iter()
            .take(config.max_frameworks)
            .map(|f| f.framework.name.as_str())
            .collect();

        let evidence: Vec<&str> = result
            .frameworks
            .iter()
            .flat_map(|f| f.evidence.iter().map(|s| s.as_str()))
            .collect();

        let safe_confidence = if result.confidence.is_finite() {
            result.confidence
        } else {
            0.0
        };

        let json_output = JsonOutput {
            indicator: result.indicator.as_ref().map(|l| l.name.clone()),
            frameworks: frameworks.into_iter().map(String::from).collect(),
            icon: result.display_icon().map(String::from),
            color: result.display_color().map(String::from),
            confidence: safe_confidence,
            evidence: evidence.into_iter().map(String::from).collect(),
        };

        match serde_json::to_string(&json_output) {
            Ok(json_str) => json_str,
            Err(e) => {
                eprintln!("Warning: JSON serialization failed: {}", e);

                let fallback = JsonOutput {
                    indicator: result.indicator.as_ref().map(|l| l.name.clone()),
                    frameworks: vec![],
                    icon: None,
                    color: None,
                    confidence: result.confidence,
                    evidence: vec!["serialization_error".to_string()],
                };

                serde_json::to_string(&fallback).unwrap_or_else(|_| {
                    r#"{"language":null,"frameworks":[],"icon":null,"color":null,"confidence":0.0,"evidence":["critical_serialization_error"]}"#.to_string()
                })
            }
        }
    }
}

pub struct CompactRenderer;

impl CompactRenderer {
    pub fn compact_name(&self, name: &str) -> String {
        match name {
            "TypeScript" => "TS".to_string(),
            "JavaScript" => "JS".to_string(),
            "Python" => "Py".to_string(),
            "Rust" => "RS".to_string(),
            "Go" => "Go".to_string(),
            "C++" => "C++".to_string(),
            "C#" => "C#".to_string(),
            "Ruby" => "RB".to_string(),
            "PHP" => "PHP".to_string(),
            "Java" => "Java".to_string(),
            "Swift" => "Swift".to_string(),
            "Kotlin" => "KT".to_string(),
            "Next.js" => "Next".to_string(),
            "NestJS" => "Nest".to_string(),
            "React" => "React".to_string(),
            "Vue" => "Vue".to_string(),
            "Angular" => "Ng".to_string(),
            "Svelte" => "Svelte".to_string(),
            _ => name.to_string(),
        }
    }
}

impl Render for CompactRenderer {
    fn render(&self, result: &DetectionResult, config: &DisplayConfig) -> String {
        let mut parts = Vec::new();

        if let Some(framework) = result.best_framework() {
            parts.push(self.compact_name(&framework.framework.name));
        } else if let Some(language) = &result.indicator {
            parts.push(self.compact_name(&language.name));
        }

        if result.best_framework().is_some() {
            if let Some(language) = &result.indicator {
                let lang_abbrev = self.compact_name(&language.name);
                if lang_abbrev.len() <= 3 {
                    parts.push(lang_abbrev);
                }
            }
        }

        parts.join(&config.framework_separator)
    }
}

pub struct DebugRenderer;

impl Render for DebugRenderer {
    fn render(&self, result: &DetectionResult, _config: &DisplayConfig) -> String {
        let mut debug_info = Vec::new();

        if let Some(language) = &result.indicator {
            debug_info.push(format!(
                "Project: {} (priority: {})",
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

        debug_info.push("\n=== DETECTION EVIDENCE ===".to_string());

        if !result.evidence.indicator_evidence.is_empty() {
            debug_info.push("Indicator Evidence:".to_string());
            for evidence in &result.evidence.indicator_evidence {
                debug_info.push(format!(
                    "  • {} -> {} (weight: {:.2}, pattern: {})",
                    evidence.file_path,
                    evidence.description,
                    evidence.weight,
                    evidence.pattern_matched
                ));
            }
        }

        if !result.evidence.framework_evidence.is_empty() {
            debug_info.push("Framework Evidence:".to_string());
            for evidence in &result.evidence.framework_evidence {
                debug_info.push(format!(
                    "  • {} -> {} (weight: {:.2}, pattern: {})",
                    evidence.file_path,
                    evidence.description,
                    evidence.weight,
                    evidence.pattern_matched
                ));
            }
        }

        if !result.evidence.root_discovery.is_empty() {
            debug_info.push("Root Discovery Evidence:".to_string());
            for evidence in &result.evidence.root_discovery {
                debug_info.push(format!(
                    "  • {} -> {} (weight: {:.2})",
                    evidence.file_path, evidence.description, evidence.weight
                ));
            }
        }

        if !result.evidence.confidence_factors.is_empty() {
            debug_info.push("Confidence Calculation:".to_string());
            for factor in &result.evidence.confidence_factors {
                debug_info.push(format!(
                    "  • {}: {:.2} × {:.2} = {:.2} ({})",
                    factor.factor_type,
                    factor.value,
                    factor.weight,
                    factor.value * factor.weight,
                    factor.description
                ));
            }
        }

        debug_info.push(format!(
            "\nScan Metrics: {} files scanned in {}ms",
            result.evidence.files_scanned, result.evidence.total_scan_time_ms
        ));

        debug_info.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DetectionType, Framework, FrameworkMatch, Indicator};
    use std::sync::Arc;

    fn create_test_result() -> DetectionResult {
        let language = Arc::new(Indicator::new(
            "TypeScript".to_string(),
            vec!["*.ts".to_string()],
            "#3178c6".to_string(),
            "TS".to_string(),
            5,
            vec![],
        ));

        let framework = FrameworkMatch {
            framework: Framework {
                name: "React".to_string(),
                ecosystems: vec![],
                detection: DetectionType::Dependencies {
                    dependencies: vec!["react".to_string()],
                },
                icon: Some("⚛".to_string()),
                color: Some("#61dafb".to_string()),
                priority: 1,
                files: vec![],
                root_indicators: vec![],
            },
            confidence: 0.95,
            evidence: vec!["package.json".to_string()],
        };

        DetectionResult::new(Some(language), vec![framework], 0.95)
    }

    #[test]
    fn test_simple_renderer_colors_the_icon() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let config = DisplayConfig::default();
        let renderer = SimpleRenderer;

        // create_test_result: React framework, icon ⚛, color #61DAFB
        let output = renderer.render(&result, &config);
        assert_eq!(output, "\x1b[38;2;97;218;251m⚛\x1b[0m");
        Ok(())
    }

    #[test]
    fn test_hex_to_rgb() {
        assert_eq!(hex_to_rgb("#61dafb"), Some((97, 218, 251)));
        assert_eq!(hex_to_rgb("#F7DF1E"), Some((247, 223, 30)));
        assert_eq!(hex_to_rgb("61dafb"), None);
        assert_eq!(hex_to_rgb("#xyzxyz"), None);
        assert_eq!(hex_to_rgb("#fff"), None);
    }

    #[test]
    fn test_json_renderer() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let config = DisplayConfig::default();
        let renderer = JsonRenderer;

        let output = renderer.render(&result, &config);
        assert!(output.contains("TypeScript"));
        assert!(output.contains("React"));
        Ok(())
    }

    #[test]
    fn test_compact_renderer() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let config = DisplayConfig::default();
        let renderer = CompactRenderer;

        let output = renderer.render(&result, &config);
        assert!(output.contains("React"));
        assert!(output.contains("TS"));
        Ok(())
    }

    #[test]
    fn test_debug_renderer() -> Result<(), Box<dyn std::error::Error>> {
        let result = create_test_result();
        let config = DisplayConfig::default();
        let renderer = DebugRenderer;

        let output = renderer.render(&result, &config);
        assert!(output.contains("Project: TypeScript"));
        assert!(output.contains("React"));
        assert!(output.contains("confidence"));
        Ok(())
    }
}
