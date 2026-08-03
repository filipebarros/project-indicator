use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::constants::{SWIFT_EXTENSION, XCWORKSPACE_EXTENSION};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_swift_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Swift".to_string(),
        vec![
            SWIFT_EXTENSION.to_string(),
            XCWORKSPACE_EXTENSION.to_string(),
        ],
        "#fa7343".to_string(),
        nerd_icon("e755"),
        8,
        vec![Ecosystem::Swiftpm],
        vec![root_indicator(
            "Package.swift",
            0.95,
            IndicatorContext::LanguageRoot,
        )],
    )
}

pub fn swift_frameworks() -> Vec<Framework> {
    vec![simple_framework(
        "Vapor",
        vec![Ecosystem::Swiftpm],
        DetectionType::Dependencies {
            dependencies: vec!["vapor".to_string()],
        },
        None,
        Some("#8d59f2"),
        1,
    )]
}
