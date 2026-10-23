use super::shared::root_indicator;
use crate::types::{Indicator, IndicatorContext};

pub fn create_gleam_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Gleam".to_string(),
        vec!["gleam.toml".to_string(), "*.gleam".to_string()],
        "#ffaff3".to_string(),
        "✨".to_string(),
        // Below JavaScript (6): a stray *.gleam file must not outrank a real
        // package.json project when both land in the low-confidence tier
        7,
        vec![],
        vec![root_indicator(
            "gleam.toml",
            0.95,
            IndicatorContext::LanguageRoot,
        )],
    )
}
