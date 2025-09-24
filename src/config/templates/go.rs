use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_go_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Go".to_string(),
        vec![
            "*.go".to_string(),
            "go.mod".to_string(),
            "go.sum".to_string(),
        ],
        "#00add8".to_string(),
        nerd_icon("e627"),
        7,
        vec![
            simple_framework(
                "Gin",
                DetectionType::GoEcosystem {
                    modules: vec!["github.com/gin-gonic/gin".to_string()],
                },
                Some(nerd_icon("ee44")),
                Some("#00add8"),
                1,
            ),
            simple_framework(
                "Echo",
                DetectionType::GoEcosystem {
                    modules: vec!["github.com/labstack/echo".to_string()],
                },
                Some(nerd_icon("f45f")),
                Some("#00add8"),
                2,
            ),
            simple_framework(
                "Fiber",
                DetectionType::GoEcosystem {
                    modules: vec!["github.com/gofiber/fiber".to_string()],
                },
                Some(nerd_icon("f0788")),
                Some("#00add8"),
                3,
            ),
            simple_framework(
                "Gorilla Mux",
                DetectionType::GoEcosystem {
                    modules: vec!["github.com/gorilla/mux".to_string()],
                },
                Some(nerd_icon("f0b0e")),
                Some("#00add8"),
                4,
            ),
            simple_framework(
                "Beego",
                DetectionType::GoEcosystem {
                    modules: vec!["github.com/beego/beego".to_string()],
                },
                Some(nerd_icon("f0fa1")),
                Some("#00add8"),
                5,
            ),
        ],
        vec![
            root_indicator("go.mod", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("go.sum", 0.7, IndicatorContext::LanguageRoot),
        ],
    )
}
