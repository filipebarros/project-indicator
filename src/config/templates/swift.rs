use super::shared::{dependency_framework, nerd_icon, root_indicator};
use crate::constants::{SWIFT_EXTENSION, XCWORKSPACE_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_swift_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Swift".to_string(),
        vec![
            SWIFT_EXTENSION.to_string(),
            XCWORKSPACE_EXTENSION.to_string(),
            "Package.resolved".to_string(),
            ".swift-version".to_string(),
        ],
        "#fa7343".to_string(),
        nerd_icon("e755"),
        8,
        vec![Ecosystem::Swiftpm],
        vec![
            root_indicator("Package.swift", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("Package.resolved", 0.6, IndicatorContext::LanguageRoot),
            root_indicator(".swift-version", 0.45, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn swift_frameworks() -> Vec<Framework> {
    vec![dependency_framework(
        "Vapor",
        &[Ecosystem::Swiftpm],
        &["vapor"],
        None,
        Some("#8d59f2"),
        1,
    )]
}
