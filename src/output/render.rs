use crate::types::{DetectionResult, DisplayConfig, EvidenceItem, FrameworkMatch, Indicator};
use serde::Serialize;

/// Drop control characters from user-configured display text.
fn printable(text: &str) -> String {
    text.chars().filter(|c| !is_unprintable(*c)).collect()
}

/// Control characters, plus the invisible characters that are not "control"
/// to Rust but can reorder or hide what the terminal shows: bidirectional
/// overrides and marks, zero-width characters, line separators, tag characters
pub(crate) fn is_unprintable(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
                | '\u{e0000}'..='\u{e007f}'
        )
}

/// One glyph: the icon, in its color unless `NO_COLOR` asks otherwise
fn glyph(icon: Option<&str>, color: Option<&str>) -> String {
    let icon = printable(icon.unwrap_or(""));
    if icon.is_empty() {
        return String::new();
    }

    if color_enabled(std::env::var_os("NO_COLOR").as_deref()) {
        if let Some((r, g, b)) = color.and_then(hex_to_rgb) {
            return format!("\x1b[38;2;{r};{g};{b}m{icon}\x1b[0m");
        }
    }

    icon
}

pub fn render_simple(result: &DetectionResult) -> String {
    // A folder that is no project but holds some: one glyph per distinct one
    if result.indicator.is_none() && !result.nested.is_empty() {
        let mut glyphs: Vec<String> = Vec::new();
        for project in &result.nested {
            let one = glyph(project.display_icon(), project.display_color());
            if !one.is_empty() && !glyphs.contains(&one) {
                glyphs.push(one);
            }
        }
        return glyphs.join(" ");
    }
    glyph(result.display_icon(), result.display_color())
}

/// NO_COLOR (<https://no-color.org>) disables color when set to a non-empty value
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

/// `icon|color` components of a language and its frameworks that have a glyph
fn components(
    language: Option<&Indicator>,
    frameworks: &[FrameworkMatch],
    config: &DisplayConfig,
    into: &mut Vec<String>,
) {
    let mut push = |component: String| {
        if !into.contains(&component) {
            into.push(component);
        }
    };
    if let Some(language) = language {
        push(format!(
            "{}|{}",
            printable(&language.icon),
            printable(&language.color)
        ));
    }

    for framework_match in frameworks.iter().take(config.max_frameworks) {
        let fw_icon = printable(framework_match.framework.icon.as_deref().unwrap_or(""));
        let fw_color = printable(framework_match.framework.color.as_deref().unwrap_or(""));

        if !fw_icon.is_empty() {
            push(format!("{fw_icon}|{fw_color}"));
        }
    }
}

pub fn render_full(result: &DetectionResult, config: &DisplayConfig) -> String {
    let mut all = Vec::new();
    components(
        result.indicator.as_deref(),
        &result.frameworks,
        config,
        &mut all,
    );
    for project in &result.nested {
        components(
            Some(&project.indicator),
            &project.frameworks,
            config,
            &mut all,
        );
    }
    all.join("•")
}

pub fn render_json(result: &DetectionResult, config: &DisplayConfig) -> String {
    #[derive(Serialize)]
    struct JsonOutput<'a> {
        indicator: Option<&'a str>,
        frameworks: Vec<&'a str>,
        icon: Option<String>,
        color: Option<String>,
        confidence: f32,
        evidence: Vec<&'a str>,
        /// Projects discovered in this result; absent for an ordinary result
        #[serde(skip_serializing_if = "Vec::is_empty")]
        projects: Vec<JsonProject<'a>>,
    }

    #[derive(Serialize)]
    struct JsonProject<'a> {
        name: String,
        indicator: &'a str,
        frameworks: Vec<&'a str>,
        icon: Option<String>,
        color: Option<String>,
    }

    let clean = |text: Option<&str>| text.map(printable);

    let listed = || result.frameworks.iter().take(config.max_frameworks);

    let json_output = JsonOutput {
        indicator: result.indicator.as_ref().map(|l| l.name.as_str()),
        frameworks: listed().map(|f| f.framework.name.as_str()).collect(),
        icon: clean(result.display_icon()),
        color: clean(result.display_color()),
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
        projects: result
            .nested
            .iter()
            .map(|project| JsonProject {
                name: printable(&project.name),
                indicator: &project.indicator.name,
                frameworks: project
                    .frameworks
                    .iter()
                    .take(config.max_frameworks)
                    .map(|f| f.framework.name.as_str())
                    .collect(),
                icon: clean(project.display_icon()),
                color: clean(project.display_color()),
            })
            .collect(),
    };

    // A struct of strings and a finite f32 always serializes
    serde_json::to_string(&json_output).unwrap_or_default()
}

pub fn render_debug(result: &DetectionResult) -> String {
    let mut debug_info = Vec::new();

    if let Some(language) = &result.indicator {
        debug_info.push(format!("Project: {}", language.name));
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
                format!(", pattern: {}", printable(&e.pattern_matched))
            } else {
                String::new()
            };
            debug_info.push(format!(
                "  • {} -> {} (weight: {:.2}{pattern})",
                printable(&e.file_path),
                printable(&e.description),
                e.weight
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

    if !result.nested.is_empty() {
        debug_info.push("Nested Projects:".to_string());
        for project in &result.nested {
            let frameworks: Vec<String> = project
                .frameworks
                .iter()
                .map(|f| printable(&f.framework.name))
                .collect();
            let suffix = if frameworks.is_empty() {
                String::new()
            } else {
                format!(" ({})", frameworks.join(", "))
            };
            debug_info.push(format!(
                "  • {} -> {}{suffix}",
                printable(&project.name),
                printable(&project.indicator.name)
            ));
        }
    }

    if !result.evidence.confidence_factors.is_empty() {
        debug_info.push("Confidence Calculation:".to_string());
        for factor in &result.evidence.confidence_factors {
            debug_info.push(format!(
                "  • {}: {:.2} ({})",
                printable(&factor.factor_type),
                factor.value,
                printable(&factor.description)
            ));
        }
    }

    debug_info.push(format!(
        "\nScan Metrics: {} files matched in {}ms",
        result.evidence.files_matched, result.evidence.total_scan_time_ms
    ));

    // File names come from the repository: keep line breaks, drop the rest
    debug_info
        .join("\n")
        .chars()
        .filter(|c| *c == '\n' || !is_unprintable(*c))
        .collect()
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
    fn test_invisible_and_line_separator_characters_are_unprintable() {
        for c in [
            '\u{200b}',
            '\u{200d}',
            '\u{2028}',
            '\u{2029}',
            '\u{2060}',
            '\u{feff}',
            '\u{e0041}',
        ] {
            assert!(is_unprintable(c), "U+{:04X} should be dropped", c as u32);
        }
        assert!(!is_unprintable('⚛') && !is_unprintable('é'));
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
                detection: DetectionType::Dependencies(vec!["vite".to_string()]),
                icon: None,
                color: None,
                priority: 2,
            },
            confidence: 0.9,
            evidence: vec!["vite.config.js".to_string()],
        });
        let config = DisplayConfig { max_frameworks: 1 };

        let output = render_json(&result, &config);
        let listed: serde_json::Value = serde_json::from_str(&output).unwrap_or_default();
        assert_eq!(listed["frameworks"].as_array().map(Vec::len), Some(1));
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

    #[test]
    fn test_debug_reports_matched_files_not_scanned_files() {
        let mut result = create_test_result();
        result.evidence.files_matched = 3;
        result.evidence.total_scan_time_ms = 7;

        let debug = render_debug(&result);

        assert!(debug.contains("3 files matched in 7ms"), "{debug}");
    }

    /// User-configured display text must not emit terminal control sequences.
    fn hostile_result(icon: &str, color: &str) -> DetectionResult {
        let indicator = crate::types::Indicator::new(
            "Hostile".to_string(),
            vec!["x.txt".to_string()],
            color.to_string(),
            icon.to_string(),
            vec![],
        );
        DetectionResult::new(Some(std::sync::Arc::new(indicator)), vec![], 0.9)
    }

    #[test]
    fn test_simple_output_drops_control_characters_from_the_icon() {
        let output = render_simple(&hostile_result("\u{1b}]0;pwned\u{7}Z", "#ffffff"));

        assert!(!output.contains('\u{7}'), "{output:?}");
        assert!(!output.contains("\x1b]"), "{output:?}");
        assert!(
            output.contains("]0;pwnedZ"),
            "printable text survives: {output:?}"
        );
    }

    #[test]
    fn test_full_output_drops_control_characters_from_icon_and_color() {
        let output = render_full(
            &hostile_result("\u{1b}[2JZ", "\u{1b}]0;x\u{7}#fff"),
            &DisplayConfig::default(),
        );

        assert!(!output.chars().any(char::is_control), "{output:?}");
        assert!(output.starts_with("[2JZ|"), "{output:?}");
    }

    /// The debug dump prints repository file names, which can contain
    /// anything a file system allows, including terminal escape sequences.
    #[test]
    fn test_debug_output_drops_control_characters_from_file_names() {
        let mut result = create_test_result();
        result
            .evidence
            .add_indicator_evidence(EvidenceItem::indicator_file(
                "evil\u{1b}]0;PWNED\u{7}.js".to_string(),
                "*.js".to_string(),
                0.8,
            ));

        let debug = render_debug(&result);

        assert!(
            !debug.chars().any(|c| c.is_control() && c != '\n'),
            "{debug:?}"
        );
        assert!(debug.contains("evil]0;PWNED.js"), "printable text survives");
    }

    #[test]
    fn test_debug_output_drops_bidi_characters_from_every_field() {
        let mut result = create_test_result();
        result
            .evidence
            .add_indicator_evidence(EvidenceItem::indicator_file(
                "a\u{202e}b.js".to_string(),
                "*.js".to_string(),
                0.8,
            ));

        let debug = render_debug(&result);

        assert!(!debug.contains('\u{202e}'), "{debug:?}");
        assert!(debug.contains("ab.js"));
    }

    #[test]
    fn test_debug_output_cannot_be_split_into_forged_lines_by_a_file_name() {
        let mut result = create_test_result();
        result
            .evidence
            .add_indicator_evidence(EvidenceItem::indicator_file(
                "a.js\n  • forged -> line".to_string(),
                "*.js".to_string(),
                0.8,
            ));

        let debug = render_debug(&result);

        assert!(!debug.contains("\n  • forged"), "{debug}");
        assert!(
            debug.contains("a.js  • forged -> line"),
            "text survives on one line"
        );
    }

    #[test]
    fn test_bidirectional_overrides_are_dropped_from_prompt_text() {
        let output = render_simple(&hostile_result("a\u{202e}b\u{2066}c\u{200f}d", "#ffffff"));

        assert!(output.contains("abcd"), "{output:?}");
    }

    mod nested {
        use super::*;
        use crate::types::{Indicator, NestedProject};
        use std::sync::Arc;

        fn framework(name: &str, icon: Option<&str>, color: Option<&str>) -> FrameworkMatch {
            FrameworkMatch::new(
                Framework {
                    name: name.to_string(),
                    ecosystems: vec![],
                    detection: DetectionType::Dependencies(vec![]),
                    icon: icon.map(str::to_string),
                    color: color.map(str::to_string),
                    priority: 1,
                },
                1.0,
                vec![],
            )
        }

        fn project(
            name: &str,
            language: (&str, &str, &str),
            frameworks: Vec<FrameworkMatch>,
        ) -> NestedProject {
            let (language_name, icon, color) = language;
            NestedProject {
                name: name.to_string(),
                indicator: Arc::new(Indicator::new(
                    language_name.to_string(),
                    vec![],
                    color.to_string(),
                    icon.to_string(),
                    vec![],
                )),
                frameworks,
            }
        }

        /// backend: Rust (framework without a glyph), frontend: JS + React
        fn container() -> DetectionResult {
            let mut result = DetectionResult::new(None, vec![], 0.0);
            result.nested = vec![
                project(
                    "backend",
                    ("Rust", "R", "#dea584"),
                    vec![framework("Axum", None, Some("#000"))],
                ),
                project(
                    "frontend",
                    ("JavaScript", "J", "#f7df1e"),
                    vec![framework("React", Some("⚛"), Some("#61dafb"))],
                ),
            ];
            result
        }

        fn glyph(icon: &str, rgb: (u8, u8, u8)) -> String {
            if color_enabled(std::env::var_os("NO_COLOR").as_deref()) {
                format!("\x1b[38;2;{};{};{}m{icon}\x1b[0m", rgb.0, rgb.1, rgb.2)
            } else {
                icon.to_string()
            }
        }

        #[test]
        fn test_simple_shows_one_glyph_per_subproject() {
            let expected = format!(
                "{} {}",
                glyph("R", (222, 165, 132)),
                glyph("⚛", (97, 218, 251))
            );

            assert_eq!(render_simple(&container()), expected);
        }

        #[test]
        fn test_simple_shows_a_repeated_glyph_once() {
            let mut result = container();
            result.nested.truncate(1);
            result.nested.push(result.nested[0].clone());

            assert_eq!(render_simple(&result), glyph("R", (222, 165, 132)));
        }

        #[test]
        fn test_full_lists_each_subprojects_language_then_its_frameworks() {
            assert_eq!(
                render_full(&container(), &DisplayConfig::default()),
                "R|#dea584•J|#f7df1e•⚛|#61dafb"
            );
        }

        #[test]
        fn test_json_lists_the_subprojects() {
            let json: serde_json::Value =
                serde_json::from_str(&render_json(&container(), &DisplayConfig::default()))
                    .unwrap_or_default();

            assert_eq!(json["projects"][0]["name"], "backend");
            assert_eq!(json["projects"][0]["indicator"], "Rust");
            assert_eq!(json["projects"][1]["name"], "frontend");
            assert_eq!(json["projects"][1]["frameworks"][0], "React");
            assert_eq!(json["projects"][1]["icon"], "⚛");
            assert_eq!(json["indicator"], serde_json::Value::Null);
        }

        #[test]
        fn test_json_strips_terminal_characters_from_project_names_and_glyphs() {
            let mut result = DetectionResult::new(None, vec![], 0.0);
            result.nested = vec![project(
                "a\u{9b}31mb\u{7f}",
                ("Rust", "R\u{9b}", "#de\u{9b}a584"),
                vec![],
            )];

            let json = render_json(&result, &DisplayConfig::default());

            assert!(
                !json.chars().any(|c| c.is_control()),
                "raw control character in {json:?}"
            );
            assert!(json.contains(r#""name":"a31mb""#), "{json}");
        }

        #[test]
        fn test_json_has_no_projects_key_for_an_ordinary_result() {
            let json: serde_json::Value = serde_json::from_str(&render_json(
                &create_test_result(),
                &DisplayConfig::default(),
            ))
            .unwrap_or_default();

            assert!(json.get("projects").is_none());
        }

        #[test]
        fn test_debug_lists_the_subprojects() {
            let debug = render_debug(&container());

            assert!(debug.contains("Nested Projects:"), "{debug}");
            assert!(debug.contains("backend -> Rust"), "{debug}");
            assert!(debug.contains("frontend -> JavaScript (React)"), "{debug}");
        }
    }
}
