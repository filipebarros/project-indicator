use super::shared::{nerd_icon, node_lockfiles, root_indicator};
use crate::constants::{CTS_EXTENSION, MTS_EXTENSION, TSCONFIG_JSON, TSX_EXTENSION, TS_EXTENSION};
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_typescript_indicator() -> Indicator {
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

    Indicator::with_root_indicators(
        "TypeScript".to_string(),
        files,
        "#3178c6".to_string(),
        nerd_icon("e8ca"),
        6,
        vec![Ecosystem::Npm],
        indicators,
    )
}
