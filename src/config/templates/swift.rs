use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_swift_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Swift".to_string(),
        vec!["*.swift".to_string(), "*.xcworkspace".to_string()],
        "#fa7343".to_string(),
        nerd_icon("e699"),
        8,
        vec![framework(
            "SwiftUI",
            DetectionType::FileExists {
                files: vec!["*.xcodeproj".to_string()],
            },
            Some(nerd_icon("e755")),
            Some("#fa7343"),
            1,
            vec![root_indicator(
                "*.xcodeproj",
                0.9,
                IndicatorContext::LanguageRoot,
            )],
        )],
        vec![root_indicator(
            "Package.swift",
            0.95,
            IndicatorContext::LanguageRoot,
        )],
    )
}
