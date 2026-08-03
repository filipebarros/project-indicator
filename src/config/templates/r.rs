use super::shared::{nerd_icon, root_indicator};
use crate::types::{Indicator, IndicatorContext};

pub fn create_r_indicator() -> Indicator {
    Indicator::with_root_indicators(
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
        vec![],
        vec![
            root_indicator("DESCRIPTION", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("renv.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
