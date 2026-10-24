use crate::types::{DetectionResult, DisplayConfig, EvidenceItem};
use serde::Serialize;

pub fn render_simple(result: &DetectionResult) -> String {
    let icon = result.display_icon().unwrap_or("");
    if icon.is_empty() {
        return String::new();
    }

    // Color the icon with ANSI truecolor so prompts show the configured color
    if color_enabled(std::env::var_os("NO_COLOR").as_deref()) {
        if let Some((r, g, b)) = result.display_color().and_then(hex_to_rgb) {
            return format!("\x1b[38;2;{r};{g};{b}m{icon}\x1b[0m");
        }
    }

    icon.into()
}

/// NO_COLOR (https://no-color.org) disables color when set to a non-empty value
fn color_enabled(no_color: Option<&std::ffi::OsStr>) -> bool {
    no_color.is_none_or(|value| value.is_empty())
}

/// Parse a `#rrggbb` or CSS-shorthand `#rgb` hex color into RGB components.
/// Returns `None` for any other shape (missing `#`, non-hex digits).
fn hex_to_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    if !hex.is_ascii() {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
    match hex.len() {
        6 => Some((channel(0..2)?, channel(2..4)?, channel(4..6)?)),
        // #abc is #aabbcc: one digit per channel, doubled
        3 => Some((
            channel(0..1)? * 17,
            channel(1..2)? * 17,
            channel(2..3)? * 17,
        )),
        _ => None,
    }
}

pub fn render_full(result: &DetectionResult, config: &DisplayConfig) -> String {
    let mut components = Vec::new();

    if let Some(language) = &result.indicator {
        components.push(format!("{}|{}", language.icon, language.color));
    }

    if config.show_frameworks {
        for framework_match in result.frameworks.iter().take(config.max_frameworks) {
            let fw_icon = framework_match.framework.icon.as_deref().unwrap_or("");
            let fw_color = framework_match.framework.color.as_deref().unwrap_or("");

            if !fw_icon.is_empty() {
                components.push(format!("{fw_icon}|{fw_color}"));
            }
        }
    }

    components.join("•")
}

pub fn render_json(result: &DetectionResult, config: &DisplayConfig) -> String {
    #[derive(Serialize)]
    struct JsonOutput<'a> {
        indicator: Option<&'a str>,
        frameworks: Vec<&'a str>,
        icon: Option<&'a str>,
        color: Option<&'a str>,
        confidence: f32,
        evidence: Vec<&'a str>,
    }

    let listed = || result.frameworks.iter().take(config.max_frameworks);

    let json_output = JsonOutput {
        indicator: result.indicator.as_ref().map(|l| l.name.as_str()),
        frameworks: listed().map(|f| f.framework.name.as_str()).collect(),
        icon: result.display_icon(),
        color: result.display_color(),
        // Two decimals: no f32 noise (0.85499996) in the output. NaN/inf
        // have no JSON representation.
        confidence: if result.confidence.is_finite() {
            (result.confidence * 100.0).round() / 100.0
        } else {
            0.0
        },
        evidence: listed()
            .flat_map(|f| f.evidence.iter().map(String::as_str))
            .collect(),
    };

    // A struct of strings and a finite f32 always serializes
    serde_json::to_string(&json_output).unwrap_or_default()
}

pub fn render_debug(result: &DetectionResult) -> String {
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

    let mut section = |title: &str, items: &[EvidenceItem], with_pattern: bool| {
        if items.is_empty() {
            return;
        }
        debug_info.push(format!("{title}:"));
        for e in items {
            let pattern = if with_pattern {
                format!(", pattern: {}", e.pattern_matched)
            } else {
                String::new()
            };
            debug_info.push(format!(
                "  • {} -> {} (weight: {:.2}{pattern})",
                e.file_path, e.description, e.weight
            ));
        }
    };
    section(
        "Indicator Evidence",
        &result.evidence.indicator_evidence,
        true,
    );
    section(
        "Framework Evidence",
        &result.evidence.framework_evidence,
        true,
    );
    section(
        "Root Discovery Evidence",
        &result.evidence.root_discovery,
        false,
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_result;
    use crate::types::{DetectionType, Framework, FrameworkMatch};

    #[test]
    fn test_shorthand_hex_expands_like_css() {
        assert_eq!(hex_to_rgb("#fff"), Some((255, 255, 255)));
        assert_eq!(hex_to_rgb("#abc"), Some((170, 187, 204)));
        assert_eq!(hex_to_rgb("#aabbcc"), Some((170, 187, 204)));
        assert_eq!(hex_to_rgb("#abcd"), None);
        assert_eq!(hex_to_rgb("abc"), None);
    }

    #[test]
    fn test_hex_to_rgb() {
        assert_eq!(hex_to_rgb("#61dafb"), Some((97, 218, 251)));
        assert_eq!(hex_to_rgb("#F7DF1E"), Some((247, 223, 30)));
        assert_eq!(hex_to_rgb("61dafb"), None);
        assert_eq!(hex_to_rgb("#xyzxyz"), None);
    }

    #[test]
    fn test_json_confidence_has_no_f32_noise() {
        let mut result = create_test_result();
        result.confidence = 0.854_999_96;
        let output = render_json(&result, &DisplayConfig::default());
        assert!(output.contains("\"confidence\":0.85,"), "{output}");
    }

    #[test]
    fn test_no_color_only_counts_when_non_empty() {
        use std::ffi::OsStr;
        assert!(color_enabled(None));
        assert!(color_enabled(Some(OsStr::new(""))));
        assert!(!color_enabled(Some(OsStr::new("1"))));
    }

    #[test]
    fn test_render_json() {
        let output = render_json(&create_test_result(), &DisplayConfig::default());
        assert!(output.contains("TypeScript"));
        assert!(output.contains("React"));
    }

    #[test]
    fn test_json_evidence_respects_framework_truncation() {
        // Evidence must only cite frameworks the JSON actually lists
        let mut result = create_test_result();
        result.frameworks.push(FrameworkMatch {
            framework: Framework {
                name: "Vite".to_string(),
                ecosystems: vec![],
                detection: DetectionType::Dependencies {
                    dependencies: vec!["vite".to_string()],
                },
                icon: None,
                color: None,
                priority: 2,
                root_indicators: vec![],
            },
            confidence: 0.9,
            evidence: vec!["vite.config.js".to_string()],
        });
        let config = DisplayConfig {
            show_frameworks: true,
            max_frameworks: 1,
        };

        let output = render_json(&result, &config);
        assert!(output.contains("package.json"));
        assert!(
            !output.contains("vite.config.js"),
            "evidence cites a truncated framework: {output}"
        );
    }

    #[test]
    fn test_render_debug() {
        let output = render_debug(&create_test_result());
        assert!(output.contains("Project: TypeScript"));
        assert!(output.contains("React"));
        assert!(output.contains("confidence"));
    }
}
