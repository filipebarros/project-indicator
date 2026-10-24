use super::shared::{dependency_framework, nerd_icon, root_indicator, with_roots};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_elixir_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Elixir".to_string(),
        vec![
            "*.ex".to_string(),
            "*.exs".to_string(),
            "mix.exs".to_string(),
            "mix.lock".to_string(),
            ".formatter.exs".to_string(),
        ],
        "#6e4a7e".to_string(),
        nerd_icon("e7cd"),
        8,
        vec![Ecosystem::Hex],
        vec![
            root_indicator("mix.exs", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("mix.lock", 0.8, IndicatorContext::LanguageRoot),
            // Elixir's code formatter config, present in nearly all modern
            // Elixir projects.
            root_indicator(".formatter.exs", 0.65, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn elixir_frameworks() -> Vec<Framework> {
    vec![with_roots(
        dependency_framework(
            "Phoenix",
            &[Ecosystem::Hex],
            &["phoenix", "phoenix_html", "phoenix_live_view"],
            Some("e860"),
            Some("#ff6600"),
            1,
        ), // (No path-style root indicators: matching is by bare filename, so
        // `lib/*_web` or `assets/` could never match)
        vec![],
    )]
}
