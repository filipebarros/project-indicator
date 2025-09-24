use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_kotlin_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Kotlin".to_string(),
        vec![
            "*.kt".to_string(),
            "*.kts".to_string(),
            "build.gradle.kts".to_string(),
        ],
        "#7f52ff".to_string(),
        nerd_icon("e634"),
        9,
        vec![framework(
            "Android",
            DetectionType::FileExists {
                files: vec!["AndroidManifest.xml".to_string()],
            },
            Some(nerd_icon("e70e")),
            Some("#3ddc84"),
            1,
            vec![root_indicator(
                "AndroidManifest.xml",
                0.95,
                IndicatorContext::Configuration,
            )],
        )],
        vec![
            root_indicator("build.gradle.kts", 0.95, IndicatorContext::BuildSystem),
            root_indicator("build.gradle", 0.9, IndicatorContext::BuildSystem),
        ],
    )
}
