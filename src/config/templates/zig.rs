use super::shared::{nerd_icon, root_indicator};
use crate::constants::ZIG_EXTENSION;
use crate::types::{Indicator, IndicatorContext};

pub fn create_zig_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Zig".to_string(),
        vec![ZIG_EXTENSION.to_string()],
        "#f7a41d".to_string(),
        nerd_icon("e8ef"),
        8,
        vec![],
        vec![
            root_indicator("build.zig", 0.95, IndicatorContext::BuildSystem),
            root_indicator("build.zig.zon", 0.8, IndicatorContext::BuildSystem),
        ],
    )
}
