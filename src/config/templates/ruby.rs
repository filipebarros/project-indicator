use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_ruby_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Ruby".to_string(),
        vec![
            "*.rb".to_string(),
            "Gemfile".to_string(),
            "Gemfile.lock".to_string(),
            "Rakefile".to_string(),
        ],
        "#cc342d".to_string(),
        nerd_icon("e605"),
        9,
        vec![
            framework(
                "Rails",
                DetectionType::RubyEcosystem {
                    gems: vec!["rails".to_string()],
                },
                Some(nerd_icon("e73b")),
                Some("#cc0000"),
                1,
                vec![
                    root_indicator(
                        "config/application.rb",
                        0.9,
                        IndicatorContext::FrameworkRoot,
                    ),
                    root_indicator("config/routes.rb", 0.9, IndicatorContext::FrameworkRoot),
                ],
            ),
            simple_framework(
                "Sinatra",
                DetectionType::RubyEcosystem {
                    gems: vec!["sinatra".to_string()],
                },
                Some(nerd_icon("ed03")),
                Some("#000000"),
                2,
            ),
        ],
        vec![
            root_indicator("Gemfile", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("Gemfile.lock", 0.8, IndicatorContext::LanguageRoot),
            root_indicator("Rakefile", 0.85, IndicatorContext::BuildSystem),
            root_indicator("*.gemspec", 0.9, IndicatorContext::LanguageRoot),
        ],
    )
}
