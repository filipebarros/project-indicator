use super::shared::{framework, nerd_icon, root_indicator, root_indicator_grouped};
use crate::constants::{BUILD_GRADLE, BUILD_GRADLE_KTS, JAVA_EXTENSION, POM_XML};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_java_indicator() -> Indicator {
    Indicator::with_root_indicators(
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
        vec![Ecosystem::Maven],
        vec![
            root_indicator(JAVA_EXTENSION, 0.95, IndicatorContext::LanguageRoot),
            // pom.xml vs build.gradle are alternative build systems: a real
            // Java project uses exactly one, so they contribute their max
            // weight rather than being summed.
            root_indicator_grouped(
                POM_XML,
                0.95,
                IndicatorContext::BuildSystem,
                "jvm-build-system",
            ),
            root_indicator_grouped(
                BUILD_GRADLE,
                0.95,
                IndicatorContext::BuildSystem,
                "jvm-build-system",
            ),
            // Not grouped: settings.gradle commonly co-occurs with
            // build.gradle in multi-module projects, it isn't an alternative
            // to it.
            root_indicator("settings.gradle", 0.8, IndicatorContext::BuildSystem),
        ],
    )
}

pub fn java_frameworks() -> Vec<Framework> {
    vec![framework(
        "Spring Boot",
        vec![Ecosystem::Maven, Ecosystem::Gradle],
        DetectionType::Dependencies {
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
    )]
}
