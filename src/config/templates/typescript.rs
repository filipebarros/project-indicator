use super::shared::{
    create_angular_framework, create_astro_framework, create_nestjs_framework,
    create_nextjs_typescript_framework, create_react_typescript_framework, nerd_icon,
    node_lockfiles, root_indicator,
};
use crate::constants::{CTS_EXTENSION, MTS_EXTENSION, TSCONFIG_JSON, TSX_EXTENSION, TS_EXTENSION};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_typescript_language() -> ProjectIndicator {
    let mut files = vec![
        TS_EXTENSION.to_string(),
        TSX_EXTENSION.to_string(),
        MTS_EXTENSION.to_string(),
        CTS_EXTENSION.to_string(),
        TSCONFIG_JSON.to_string(),
    ];
    files.extend(node_lockfiles());

    let mut indicators = vec![root_indicator(
        TSCONFIG_JSON,
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
        nerd_icon("e8ca"),
        6,
        vec![
            create_react_typescript_framework(),
            create_angular_framework(),
            create_nextjs_typescript_framework(),
            create_astro_framework(),
            create_nestjs_framework(),
        ],
        indicators,
    )
}
