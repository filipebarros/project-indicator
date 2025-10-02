use super::shared::{nerd_icon, root_indicator};
use crate::constants::{GO_EXTENSION, GO_MOD};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_go_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Go".to_string(),
        vec![
            GO_EXTENSION.to_string(),
            GO_MOD.to_string(),
            "go.sum".to_string(),
        ],
        "#00add8".to_string(),
        nerd_icon("e724"),
        7,
        vec![],
        vec![
            root_indicator(GO_MOD, 0.9, IndicatorContext::LanguageRoot),
            root_indicator("go.sum", 0.7, IndicatorContext::LanguageRoot),
        ],
    )
}
