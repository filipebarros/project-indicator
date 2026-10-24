use super::shared::{dependency_framework, file_framework, nerd_icon, root_indicator, with_roots};
use crate::constants::{BUILD_GRADLE_KTS, KOTLIN_EXTENSION, KOTLIN_SCRIPT_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_kotlin_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Kotlin".to_string(),
        vec![
            KOTLIN_EXTENSION.to_string(),
            KOTLIN_SCRIPT_EXTENSION.to_string(),
            BUILD_GRADLE_KTS.to_string(),
            "src/main/kotlin".to_string(),
        ],
        "#7f52ff".to_string(),
        nerd_icon("e81b"),
        9,
        vec![Ecosystem::Gradle, Ecosystem::Maven],
        vec![
            root_indicator(KOTLIN_EXTENSION, 0.95, IndicatorContext::LanguageRoot),
            root_indicator("src/main/kotlin", 0.9, IndicatorContext::LanguageRoot),
            root_indicator(BUILD_GRADLE_KTS, 0.95, IndicatorContext::BuildSystem),
        ],
    )
}

pub fn kotlin_frameworks() -> Vec<Framework> {
    vec![
        dependency_framework(
            "Ktor",
            &[Ecosystem::Gradle],
            &["ktor-server-core", "ktor-server-netty", "ktor-server-cio"],
            Some("e81c"),
            Some("#087cfa"),
            1,
        ),
        with_roots(
            file_framework(
                "Android",
                &[Ecosystem::Gradle],
                // Gradle puts the manifest under src/main/ of the app module
                &[
                    "AndroidManifest.xml",
                    "src/main/AndroidManifest.xml",
                    "app/src/main/AndroidManifest.xml",
                ],
                Some("e70e"),
                Some("#3ddc84"),
                3,
            ),
            vec![root_indicator(
                "AndroidManifest.xml",
                0.95,
                IndicatorContext::Configuration,
            )],
        ),
    ]
}
