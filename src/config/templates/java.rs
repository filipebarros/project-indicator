use super::shared::{framework, nerd_icon, root_indicator};
use crate::constants::{BUILD_GRADLE, BUILD_GRADLE_KTS, JAVA_EXTENSION, POM_XML};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_java_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Java".to_string(),
        vec![
            JAVA_EXTENSION.to_string(),
            POM_XML.to_string(),
            BUILD_GRADLE.to_string(),
            BUILD_GRADLE_KTS.to_string(),
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
            root_indicator(POM_XML, 0.95, IndicatorContext::BuildSystem),
            root_indicator(BUILD_GRADLE, 0.95, IndicatorContext::BuildSystem),
            root_indicator(BUILD_GRADLE_KTS, 0.95, IndicatorContext::BuildSystem),
            root_indicator("settings.gradle", 0.8, IndicatorContext::BuildSystem),
        ],
    )
}
