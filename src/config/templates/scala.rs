use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_scala_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Scala".to_string(),
        vec![
            "*.scala".to_string(),
            "build.sbt".to_string(),
            "build.sc".to_string(),
        ],
        "#dc322f".to_string(),
        nerd_icon("e737"),
        6,
        vec![
            framework(
                "Play Framework",
                DetectionType::ScalaEcosystem {
                    dependencies: vec![
                        "play-server".to_string(),
                        "play-ws".to_string(),
                        "play-json".to_string(),
                    ],
                },
                Some(nerd_icon("f04b")),
                Some("#92d13d"),
                1,
                vec![
                    root_indicator(
                        "conf/application.conf",
                        0.9,
                        IndicatorContext::Configuration,
                    ),
                    root_indicator("conf/routes", 0.9, IndicatorContext::Configuration),
                ],
            ),
            simple_framework(
                "Akka HTTP",
                DetectionType::ScalaEcosystem {
                    dependencies: vec![
                        "akka-http".to_string(),
                        "akka-stream".to_string(),
                        "akka-actor".to_string(),
                    ],
                },
                Some(nerd_icon("e708")),
                Some("#0b5394"),
                2,
            ),
            simple_framework(
                "Finagle",
                DetectionType::ScalaEcosystem {
                    dependencies: vec!["finagle-core".to_string(), "finagle-http".to_string()],
                },
                Some(nerd_icon("f15c6")),
                Some("#1da1f2"),
                3,
            ),
            simple_framework(
                "Spark",
                DetectionType::ScalaEcosystem {
                    dependencies: vec!["spark-core".to_string(), "spark-sql".to_string()],
                },
                Some(nerd_icon("ec10")),
                Some("#e25a1c"),
                4,
            ),
        ],
        vec![
            root_indicator("build.sbt", 0.95, IndicatorContext::BuildSystem),
            root_indicator(
                "project/build.properties",
                0.8,
                IndicatorContext::BuildSystem,
            ),
        ],
    )
}
