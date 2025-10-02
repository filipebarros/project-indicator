use super::shared::{framework, nerd_icon, root_indicator};
use crate::constants::{GEMFILE, GEMFILE_LOCK, GEMSPEC_EXTENSION, RUBY_EXTENSION};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_ruby_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Ruby".to_string(),
        vec![
            RUBY_EXTENSION.to_string(),
            GEMFILE.to_string(),
            GEMFILE_LOCK.to_string(),
            "Rakefile".to_string(),
        ],
        "#cc342d".to_string(),
        nerd_icon("e739"),
        9,
        vec![framework(
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
        )],
        vec![
            root_indicator(GEMFILE, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(GEMFILE_LOCK, 0.8, IndicatorContext::LanguageRoot),
            root_indicator("Rakefile", 0.85, IndicatorContext::BuildSystem),
            root_indicator(GEMSPEC_EXTENSION, 0.9, IndicatorContext::LanguageRoot),
        ],
    )
}
