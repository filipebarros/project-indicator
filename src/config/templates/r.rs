use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_r_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "R".to_string(),
        vec![
            "*.r".to_string(),
            "*.R".to_string(),
            "*.Rmd".to_string(),
            "*.Rnw".to_string(),
            "DESCRIPTION".to_string(),
            "NAMESPACE".to_string(),
        ],
        "#276dc3".to_string(),
        nerd_icon("e881"),
        8,
        vec![simple_framework(
            "Shiny",
            DetectionType::FileExists {
                files: vec![
                    "app.R".to_string(),
                    "ui.R".to_string(),
                    "server.R".to_string(),
                ],
            },
            Some(nerd_icon("f02d8")),
            Some("#276dc3"),
            1,
        )],
        vec![
            root_indicator("DESCRIPTION", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("renv.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
