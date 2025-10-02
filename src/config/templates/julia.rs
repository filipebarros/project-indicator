use super::shared::root_indicator;
use crate::constants::JULIA_EXTENSION;
use crate::{
    config::templates::shared::nerd_icon,
    types::{IndicatorContext, ProjectIndicator},
};

pub fn create_julia_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Julia".to_string(),
        vec![
            JULIA_EXTENSION.to_string(),
            "Project.toml".to_string(),
            "Manifest.toml".to_string(),
        ],
        "#9558b2".to_string(),
        nerd_icon("e80d"),
        7,
        vec![],
        vec![
            root_indicator("Project.toml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("Manifest.toml", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
