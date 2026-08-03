use super::shared::{framework, nerd_icon, root_indicator};
use crate::constants::{DART_EXTENSION, PUBSPEC_YAML};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_dart_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Dart".to_string(),
        vec![DART_EXTENSION.to_string(), PUBSPEC_YAML.to_string()],
        "#0175c2".to_string(),
        nerd_icon("e798"),
        8,
        vec![Ecosystem::Pub],
        vec![
            root_indicator("pubspec.yaml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("pubspec.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn dart_frameworks() -> Vec<Framework> {
    vec![framework(
        "Flutter",
        vec![Ecosystem::Pub],
        DetectionType::Dependencies {
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
    )]
}
