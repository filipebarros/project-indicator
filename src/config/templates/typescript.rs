use super::shared::{nerd_icon, node_root_indicators, root_indicator};
use crate::constants::{CTS_EXTENSION, MTS_EXTENSION, TSCONFIG_JSON, TSX_EXTENSION, TS_EXTENSION};
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_typescript_indicator() -> Indicator {
    let node = node_root_indicators();
    let mut files = vec![
        TS_EXTENSION.to_string(),
        TSX_EXTENSION.to_string(),
        MTS_EXTENSION.to_string(),
        CTS_EXTENSION.to_string(),
        TSCONFIG_JSON.to_string(),
    ];
    files.extend(node.iter().map(|r| r.pattern.clone()));

    let mut root_indicators = vec![root_indicator(
        TSCONFIG_JSON,
        0.95,
        IndicatorContext::LanguageRoot,
    )];
    root_indicators.extend(node.into_iter().map(|mut r| {
        // tsconfig.json is the stronger TypeScript marker
        if r.pattern == "package.json" {
            r.weight = 0.9;
        }
        r
    }));

    Indicator::with_root_indicators(
        "TypeScript".to_string(),
        files,
        "#3178c6".to_string(),
        nerd_icon("e8ca"),
        6,
        vec![Ecosystem::Npm],
        root_indicators,
    )
}
