use super::shared::{nerd_icon, root_indicator};
use crate::types::{Indicator, IndicatorContext};

pub fn create_nix_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Nix".to_string(),
        vec!["*.nix".to_string(), "flake.lock".to_string()],
        "#5277c3".to_string(),
        nerd_icon("f313"),
        // Low priority: a flake.nix dev shell should not outrank the
        // application language it provisions
        13,
        vec![],
        vec![
            root_indicator("flake.nix", 0.95, IndicatorContext::ToolchainRoot),
            root_indicator("flake.lock", 0.85, IndicatorContext::ToolchainRoot),
            root_indicator("default.nix", 0.8, IndicatorContext::ToolchainRoot),
            root_indicator("shell.nix", 0.8, IndicatorContext::ToolchainRoot),
        ],
    )
}
