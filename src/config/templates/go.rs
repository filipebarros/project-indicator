use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::constants::{GO_EXTENSION, GO_MOD};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

fn go_framework(
    name: &str,
    modules: &[&str],
    color: &str,
    priority: u8,
) -> crate::types::Framework {
    simple_framework(
        name,
        vec![Ecosystem::Go],
        DetectionType::Dependencies {
            // Major-version module paths (…/vN) are listed explicitly: go.mod
            // matching is line-anchored, so a bare prefix won't match them
            dependencies: modules.iter().map(|m| m.to_string()).collect(),
        },
        None,
        Some(color),
        priority,
    )
}

pub fn create_go_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Go".to_string(),
        vec![
            GO_EXTENSION.to_string(),
            GO_MOD.to_string(),
            "go.sum".to_string(),
        ],
        "#00add8".to_string(),
        nerd_icon("e724"),
        7,
        vec![Ecosystem::Go],
        vec![
            root_indicator(GO_MOD, 0.9, IndicatorContext::LanguageRoot),
            root_indicator("go.sum", 0.7, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn go_frameworks() -> Vec<Framework> {
    vec![
        go_framework("Gin", &["github.com/gin-gonic/gin"], "#008ecf", 1),
        go_framework(
            "Echo",
            &["github.com/labstack/echo/v4", "github.com/labstack/echo/v5"],
            "#00abda",
            1,
        ),
        go_framework(
            "Fiber",
            &["github.com/gofiber/fiber/v2", "github.com/gofiber/fiber/v3"],
            "#28b6f6",
            1,
        ),
    ]
}
