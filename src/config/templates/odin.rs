use super::shared::{nerd_icon, root_indicator};
use crate::types::{Indicator, IndicatorContext};

pub fn create_odin_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Odin".to_string(),
        vec!["*.odin".to_string(), "ols.json".to_string()],
        "#3c73a8".to_string(),
        nerd_icon("e6b0"),
        8,
        vec![],
        // Odin has no canonical manifest; ols.json (the language-server
        // config) is the closest common root marker, weighted as tooling
        // configuration rather than a language root
        vec![root_indicator(
            "ols.json",
            0.75,
            IndicatorContext::Configuration,
        )],
    )
}
