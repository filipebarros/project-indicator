use super::shared::{root_indicator, simple_framework};
use crate::{
    config::templates::shared::nerd_icon,
    types::{DetectionType, IndicatorContext, ProjectIndicator},
};

pub fn create_julia_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Julia".to_string(),
        vec![
            "*.jl".to_string(),
            "Project.toml".to_string(),
            "Manifest.toml".to_string(),
        ],
        "#9558b2".to_string(),
        nerd_icon("e80d"),
        7,
        vec![simple_framework(
            "Pluto",
            DetectionType::FileExists {
                files: vec!["*.jl".to_string()],
            },
            Some(nerd_icon("e22e")),
            Some("#9558b2"),
            1,
        )],
        vec![
            root_indicator("Project.toml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("Manifest.toml", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
