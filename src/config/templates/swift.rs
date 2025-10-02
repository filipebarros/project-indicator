use super::shared::{nerd_icon, root_indicator};
use crate::constants::{SWIFT_EXTENSION, XCWORKSPACE_EXTENSION};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_swift_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Swift".to_string(),
        vec![
            SWIFT_EXTENSION.to_string(),
            XCWORKSPACE_EXTENSION.to_string(),
        ],
        "#fa7343".to_string(),
        nerd_icon("e755"),
        8,
        vec![],
        vec![root_indicator(
            "Package.swift",
            0.95,
            IndicatorContext::LanguageRoot,
        )],
    )
}
