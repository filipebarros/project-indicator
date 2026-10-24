use super::shared::{dependency_framework, nerd_icon, root_indicator};
use crate::constants::{BUILD_SBT, SCALA_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_scala_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Scala".to_string(),
        vec![
            SCALA_EXTENSION.to_string(),
            BUILD_SBT.to_string(),
            "build.sc".to_string(),
            "src/main/scala".to_string(),
        ],
        "#dc322f".to_string(),
        nerd_icon("e737"),
        6,
        vec![Ecosystem::Sbt],
        vec![
            root_indicator("src/main/scala", 0.9, IndicatorContext::LanguageRoot),
            root_indicator(BUILD_SBT, 0.95, IndicatorContext::BuildSystem),
            root_indicator(
                "project/build.properties",
                0.8,
                IndicatorContext::BuildSystem,
            ),
        ],
    )
}

pub fn scala_frameworks() -> Vec<Framework> {
    vec![dependency_framework(
        "Akka HTTP",
        &[Ecosystem::Sbt],
        // akka-actor / akka-stream alone are Akka, not Akka HTTP
        &["akka-http"],
        Some("e708"),
        Some("#0b5394"),
        1,
    )]
}
