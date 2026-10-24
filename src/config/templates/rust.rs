use super::shared::{
    dependency_framework, file_framework, nerd_icon, root_indicator, root_indicator_grouped,
};
use crate::constants::{CARGO_LOCK, CARGO_TOML, RS_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

fn rust_framework(name: &str, dep: &str, color: &str, priority: u8) -> Framework {
    dependency_framework(
        name,
        &[Ecosystem::Cargo],
        &[dep],
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
            "rust-toolchain.toml".to_string(),
            "rust-toolchain".to_string(),
        ],
        "#dea584".to_string(),
        nerd_icon("e7a8"),
        5,
        vec![Ecosystem::Cargo],
        vec![
            root_indicator(CARGO_TOML, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(CARGO_LOCK, 0.8, IndicatorContext::LanguageRoot),
            // TOML vs legacy plain-text format are alternative conventions
            // for the same rustup toolchain pin.
            root_indicator_grouped(
                "rust-toolchain.toml",
                0.6,
                IndicatorContext::LanguageRoot,
                "rust-toolchain-pin",
            ),
            root_indicator_grouped(
                "rust-toolchain",
                0.55,
                IndicatorContext::LanguageRoot,
                "rust-toolchain-pin",
            ),
        ],
    )
}

pub fn rust_frameworks() -> Vec<Framework> {
    vec![
        rust_framework("Axum", "axum", "#7c3aed", 1),
        rust_framework("Actix Web", "actix-web", "#8b949e", 1),
        rust_framework("Rocket", "rocket", "#d33847", 1),
        // Every Tauri app has a tauri.conf.json — at the root or in
        // src-tauri/ — which is more reliable than the root Cargo.toml
        // (the dependency lives in src-tauri/ for JS-rooted projects).
        // Npm too, so JavaScript-rooted projects surface it.
        file_framework(
            "Tauri",
            &[Ecosystem::Cargo, Ecosystem::Npm],
            &["tauri.conf.json", "src-tauri/tauri.conf.json"],
            None,
            Some("#24c8db"),
            2,
        ),
    ]
}
