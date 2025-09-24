use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_rust_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Rust".to_string(),
        vec![
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
            "Cargo.lock".to_string(),
        ],
        "#dea584".to_string(),
        nerd_icon("e7a8"),
        5,
        vec![
            simple_framework(
                "Actix Web",
                DetectionType::RustEcosystem {
                    dependencies: vec!["actix-web".to_string()],
                },
                Some(nerd_icon("eb01")),
                Some("#ff6b00"),
                1,
            ),
            framework(
                "Rocket",
                DetectionType::RustEcosystem {
                    dependencies: vec!["rocket".to_string()],
                },
                Some(nerd_icon("f135")),
                Some("#d33847"),
                2,
                vec![root_indicator(
                    "Rocket.toml",
                    0.9,
                    IndicatorContext::FrameworkRoot,
                )],
            ),
            simple_framework(
                "Axum",
                DetectionType::RustEcosystem {
                    dependencies: vec!["axum".to_string()],
                },
                Some(nerd_icon("f08c8")),
                Some("#4a90e2"),
                3,
            ),
            simple_framework(
                "Warp",
                DetectionType::RustEcosystem {
                    dependencies: vec!["warp".to_string()],
                },
                Some(nerd_icon("f1b3")),
                Some("#00a8ff"),
                4,
            ),
        ],
        vec![
            root_indicator("Cargo.toml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("Cargo.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
