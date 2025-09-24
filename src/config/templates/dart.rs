use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_dart_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Dart".to_string(),
        vec!["*.dart".to_string(), "pubspec.yaml".to_string()],
        "#0175c2".to_string(),
        nerd_icon("e798"),
        8,
        vec![framework(
            "Flutter",
            DetectionType::DartEcosystem {
                dependencies: vec![
                    "flutter".to_string(),
                    "flutter_test".to_string(),
                    "cupertino_icons".to_string(),
                ],
            },
            Some(nerd_icon("e7dd")),
            Some("#02569b"),
            1,
            vec![
                root_indicator("lib/main.dart", 0.9, IndicatorContext::FrameworkRoot),
                root_indicator("android/", 0.8, IndicatorContext::FrameworkRoot),
                root_indicator("ios/", 0.8, IndicatorContext::FrameworkRoot),
            ],
        )],
        vec![
            root_indicator("pubspec.yaml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("pubspec.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
