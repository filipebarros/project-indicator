use super::shared::{dependency_framework, nerd_icon, root_indicator, with_roots};
use crate::constants::{GEMFILE, GEMFILE_LOCK, GEMSPEC_EXTENSION, RUBY_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_ruby_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Ruby".to_string(),
        vec![
            RUBY_EXTENSION.to_string(),
            GEMFILE.to_string(),
            GEMFILE_LOCK.to_string(),
            GEMSPEC_EXTENSION.to_string(),
            "Rakefile".to_string(),
            ".ruby-version".to_string(),
        ],
        "#cc342d".to_string(),
        nerd_icon("e739"),
        9,
        vec![Ecosystem::Rubygems],
        vec![
            root_indicator(GEMFILE, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(GEMFILE_LOCK, 0.8, IndicatorContext::LanguageRoot),
            root_indicator("Rakefile", 0.85, IndicatorContext::BuildSystem),
            root_indicator(GEMSPEC_EXTENSION, 0.9, IndicatorContext::LanguageRoot),
            root_indicator(".ruby-version", 0.5, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn ruby_frameworks() -> Vec<Framework> {
    vec![with_roots(
        dependency_framework(
            "Rails",
            &[Ecosystem::Rubygems],
            &["rails"],
            Some("e73b"),
            Some("#cc0000"),
            1,
        ),
        vec![
            root_indicator(
                "config/application.rb",
                0.9,
                IndicatorContext::FrameworkRoot,
            ),
            root_indicator("config/routes.rb", 0.9, IndicatorContext::FrameworkRoot),
        ],
    )]
}
