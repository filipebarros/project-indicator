use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_elixir_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Elixir".to_string(),
        vec![
            "*.ex".to_string(),
            "*.exs".to_string(),
            "mix.exs".to_string(),
            "mix.lock".to_string(),
        ],
        "#6e4a7e".to_string(),
        nerd_icon("e7cd"),
        8,
        vec![Ecosystem::Hex],
        vec![
            root_indicator("mix.exs", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("mix.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn elixir_frameworks() -> Vec<Framework> {
    vec![framework(
        "Phoenix",
        vec![Ecosystem::Hex],
        DetectionType::Dependencies {
            dependencies: vec![
                "phoenix".to_string(),
                "phoenix_html".to_string(),
                "phoenix_live_view".to_string(),
            ],
        },
        Some(nerd_icon("e860")),
        Some("#ff6600"),
        1,
        vec![
            root_indicator("lib/*_web", 0.9, IndicatorContext::FrameworkRoot),
            root_indicator("assets/", 0.8, IndicatorContext::FrameworkRoot),
        ],
    )]
}
