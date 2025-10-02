use super::shared::{framework, nerd_icon, root_indicator};
use crate::constants::{BUILD_GRADLE, BUILD_GRADLE_KTS, KOTLIN_EXTENSION, KOTLIN_SCRIPT_EXTENSION};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_kotlin_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Kotlin".to_string(),
        vec![
            KOTLIN_EXTENSION.to_string(),
            KOTLIN_SCRIPT_EXTENSION.to_string(),
            BUILD_GRADLE_KTS.to_string(),
        ],
        "#7f52ff".to_string(),
        nerd_icon("e81b"),
        9,
        vec![
            framework(
                "Ktor",
                DetectionType::KotlinEcosystem {
                    dependencies: vec![
                        "ktor-server-core".to_string(),
                        "ktor-server-netty".to_string(),
                        "ktor-server-cio".to_string(),
                    ],
                },
                Some(nerd_icon("e81c")),
                Some("#087cfa"),
                1,
                vec![],
            ),
            framework(
                "Spring Boot",
                DetectionType::KotlinEcosystem {
                    dependencies: vec![
                        "spring-boot-starter".to_string(),
                        "spring-boot-starter-web".to_string(),
                    ],
                },
                Some(nerd_icon("e8ac")),
                Some("#6db33f"),
                2,
                vec![],
            ),
            framework(
                "Android",
                DetectionType::FileExists {
                    files: vec!["AndroidManifest.xml".to_string()],
                },
                Some(nerd_icon("e70e")),
                Some("#3ddc84"),
                3,
                vec![root_indicator(
                    "AndroidManifest.xml",
                    0.95,
                    IndicatorContext::Configuration,
                )],
            ),
        ],
        vec![
            root_indicator(BUILD_GRADLE_KTS, 0.95, IndicatorContext::BuildSystem),
            root_indicator(BUILD_GRADLE, 0.9, IndicatorContext::BuildSystem),
        ],
    )
}
