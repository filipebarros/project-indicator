use crate::types::{DetectionResult, DisplayConfig};

pub struct RichFormatter {
    display_config: DisplayConfig,
}

impl RichFormatter {
    pub fn new(display_config: DisplayConfig) -> Self {
        Self { display_config }
    }

    pub fn create_detailed_table(&self, result: &DetectionResult) -> String {
        let mut output = Vec::new();

        output.push(
            "╭─ Project Analysis ──────────────────────────────────────────────────╮".to_string(),
        );

        if let Some(language) = &result.indicator {
            output.push(format!(
                "│ 🎯 Project: {} {} │",
                language.icon, language.name
            ));
            output.push(format!(
                "│    Color: {}                                              │",
                language.color
            ));
            output.push(format!(
                "│    Priority: {}                                            │",
                language.priority
            ));
            output.push(format!(
                "│    Confidence: {:.1}%                                       │",
                result.confidence * 100.0
            ));
        } else {
            output.push(
                "│ 🎯 Project: Not detected                                           │"
                    .to_string(),
            );
        }

        output.push(
            "├──────────────────────────────────────────────────────────────────────┤".to_string(),
        );

        if !result.frameworks.is_empty() {
            output.push(
                "│ 🔧 Frameworks Detected:                                           │".to_string(),
            );
            output.push(
                "│                                                                    │"
                    .to_string(),
            );
            output.push(
                "│ # │ Icon │ Name           │ Confidence │ Priority │ Evidence     │".to_string(),
            );
            output.push(
                "│───┼──────┼────────────────┼────────────┼──────────┼──────────────│".to_string(),
            );

            for (i, framework_match) in result
                .frameworks
                .iter()
                .take(self.display_config.max_frameworks)
                .enumerate()
            {
                let icon = framework_match.framework.icon.as_deref().unwrap_or("  ");
                let name = &framework_match.framework.name;
                let confidence = format!("{:.1}%", framework_match.confidence * 100.0);
                let priority = framework_match.framework.priority;
                let evidence_count = framework_match.evidence.len();

                output.push(format!(
                    "│ {} │ {}  │ {:<14} │ {:<10} │ {:<8} │ {} files     │",
                    i + 1,
                    icon,
                    if name.len() > 14 { &name[..14] } else { name },
                    confidence,
                    priority,
                    evidence_count
                ));
            }
        } else {
            output.push(
                "│ 🔧 Frameworks: None detected                                      │".to_string(),
            );
        }

        output.push(
            "╰──────────────────────────────────────────────────────────────────────╯".to_string(),
        );

        output.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DetectionType, Framework, FrameworkMatch, Indicator};
    use std::sync::Arc;

    fn create_test_result() -> DetectionResult {
        let language = Indicator::new(
            "TypeScript".to_string(),
            vec!["package.json".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            85,
            vec![],
        );

        let framework = Framework {
            name: "React".to_string(),
            ecosystems: vec![],
            detection: DetectionType::Dependencies {
                dependencies: vec!["react".to_string()],
            },
            icon: Some("⚛️".to_string()),
            color: Some("#61DAFB".to_string()),
            priority: 90,
            files: vec![],
            root_indicators: vec![],
        };

        let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

        DetectionResult::new(Some(Arc::new(language)), vec![framework_match], 0.85)
    }

    #[test]
    fn test_detailed_table() -> Result<(), Box<dyn std::error::Error>> {
        let config = DisplayConfig::default();
        let formatter = RichFormatter::new(config);

        let result = create_test_result();
        let output = formatter.create_detailed_table(&result);

        assert!(output.contains("Project Analysis"));
        assert!(output.contains("Project:"));
        assert!(output.contains("Frameworks Detected:"));
        assert!(output.contains("TypeScript"));
        assert!(output.contains("React"));
        Ok(())
    }
}
