use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_elixir_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Elixir".to_string(),
        vec![
            "*.ex".to_string(),
            "*.exs".to_string(),
            "mix.exs".to_string(),
            "mix.lock".to_string(),
        ],
        "#6e4a7e".to_string(),
        nerd_icon("e62d"),
        8,
        vec![framework(
            "Phoenix",
            DetectionType::FileExists {
                files: vec!["lib/*_web".to_string(), "assets".to_string()],
            },
            Some(nerd_icon("e860")),
            Some("#ff6600"),
            1,
            vec![
                root_indicator("lib/*_web", 0.9, IndicatorContext::FrameworkRoot),
                root_indicator("assets/", 0.8, IndicatorContext::FrameworkRoot),
            ],
        )],
        vec![
            root_indicator("mix.exs", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("mix.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
