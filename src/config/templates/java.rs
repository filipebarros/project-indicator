use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_java_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Java".to_string(),
        vec![
            "*.java".to_string(),
            "pom.xml".to_string(),
            "build.gradle".to_string(),
            "build.gradle.kts".to_string(),
        ],
        "#ed8b00".to_string(),
        nerd_icon("e738"),
        11,
        vec![framework(
            "Spring Boot",
            DetectionType::JavaEcosystem {
                dependencies: vec![
                    "spring-boot-starter".to_string(),
                    "spring-boot-starter-web".to_string(),
                    "spring-boot-starter-data-jpa".to_string(),
                    "spring-boot-starter-test".to_string(),
                ],
            },
            Some(nerd_icon("e8ac")),
            Some("#6db33f"),
            1,
            vec![
                root_indicator(
                    "src/main/resources/application.properties",
                    0.9,
                    IndicatorContext::Configuration,
                ),
                root_indicator(
                    "src/main/resources/application.yml",
                    0.9,
                    IndicatorContext::Configuration,
                ),
            ],
        )],
        vec![
            root_indicator("pom.xml", 0.95, IndicatorContext::BuildSystem),
            root_indicator("build.gradle", 0.95, IndicatorContext::BuildSystem),
            root_indicator("build.gradle.kts", 0.95, IndicatorContext::BuildSystem),
            root_indicator("settings.gradle", 0.8, IndicatorContext::BuildSystem),
        ],
    )
}
