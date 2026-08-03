use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::constants::{CARGO_LOCK, CARGO_TOML, RS_EXTENSION};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

fn rust_framework(name: &str, dep: &str, color: &str, priority: u8) -> crate::types::Framework {
    simple_framework(
        name,
        vec![Ecosystem::Cargo],
        DetectionType::Dependencies {
            dependencies: vec![dep.to_string()],
        },
        None,
        Some(color),
        priority,
    )
}

pub fn create_rust_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Rust".to_string(),
        vec![
            RS_EXTENSION.to_string(),
            CARGO_TOML.to_string(),
            CARGO_LOCK.to_string(),
        ],
        "#dea584".to_string(),
        nerd_icon("e7a8"),
        5,
        vec![Ecosystem::Cargo],
        vec![
            root_indicator(CARGO_TOML, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(CARGO_LOCK, 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn rust_frameworks() -> Vec<Framework> {
    vec![
        rust_framework("Axum", "axum", "#7c3aed", 1),
        rust_framework("Actix Web", "actix-web", "#000000", 1),
        rust_framework("Rocket", "rocket", "#d33847", 1),
        rust_framework("Tauri", "tauri", "#24c8db", 2),
    ]
}
