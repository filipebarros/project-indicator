use super::shared::{
    create_angular_framework, create_nestjs_framework, create_nextjs_framework,
    create_react_framework, nerd_icon, node_lockfiles, root_indicator,
};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_typescript_language() -> ProjectIndicator {
    let mut files = vec![
        "*.ts".to_string(),
        "*.tsx".to_string(),
        "*.mts".to_string(),
        "*.cts".to_string(),
        "tsconfig.json".to_string(),
    ];
    files.extend(node_lockfiles());

    let mut indicators = vec![root_indicator(
        "tsconfig.json",
        0.95,
        IndicatorContext::LanguageRoot,
    )];

    let mut node_indicators = super::shared::node_lockfile_root_indicators();
    for indicator in &mut node_indicators {
        if indicator.pattern == "package.json" {
            indicator.weight = 0.9;
        }
    }
    indicators.extend(node_indicators);

    ProjectIndicator::with_root_indicators(
        "TypeScript".to_string(),
        files,
        "#3178c6".to_string(),
        nerd_icon("e628"),
        6,
        vec![
            create_react_framework(true),
            create_angular_framework(),
            create_nextjs_framework(true),
            create_nestjs_framework(),
        ],
        indicators,
    )
}
