use super::shared::{nerd_icon, root_indicator};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_zig_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Zig".to_string(),
        vec!["*.zig".to_string()],
        "#f7a41d".to_string(),
        nerd_icon("e6a9"),
        8,
        vec![],
        vec![
            root_indicator("build.zig", 0.95, IndicatorContext::BuildSystem),
            root_indicator("build.zig.zon", 0.8, IndicatorContext::BuildSystem),
        ],
    )
}
