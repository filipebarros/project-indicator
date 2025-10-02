use super::shared::{nerd_icon, root_indicator};
use crate::constants::{CARGO_LOCK, CARGO_TOML, RS_EXTENSION};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_rust_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Rust".to_string(),
        vec![
            RS_EXTENSION.to_string(),
            CARGO_TOML.to_string(),
            CARGO_LOCK.to_string(),
        ],
        "#dea584".to_string(),
        nerd_icon("e7a8"),
        5,
        vec![],
        vec![
            root_indicator(CARGO_TOML, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(CARGO_LOCK, 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
