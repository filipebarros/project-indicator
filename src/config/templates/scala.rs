use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::constants::{BUILD_SBT, SCALA_EXTENSION};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_scala_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Scala".to_string(),
        vec![
            SCALA_EXTENSION.to_string(),
            BUILD_SBT.to_string(),
            "build.sc".to_string(),
        ],
        "#dc322f".to_string(),
        nerd_icon("e737"),
        6,
        vec![Ecosystem::Sbt],
        vec![
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
    vec![simple_framework(
        "Akka HTTP",
        vec![Ecosystem::Sbt],
        DetectionType::Dependencies {
            dependencies: vec![
                "akka-http".to_string(),
                "akka-stream".to_string(),
                "akka-actor".to_string(),
            ],
        },
        Some(nerd_icon("e708")),
        Some("#0b5394"),
        1,
    )]
}
