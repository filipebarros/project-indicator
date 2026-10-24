use super::shared::{nerd_icon, node_root_indicators, root_indicator};
use crate::constants::{CJS_EXTENSION, JS_EXTENSION, MJS_EXTENSION};
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_javascript_indicator() -> Indicator {
    let mut root_indicators = node_root_indicators();
    root_indicators.push(root_indicator(
        "node_modules",
        0.6,
        IndicatorContext::LanguageRoot,
    ));

    let mut files = vec![
        JS_EXTENSION.to_string(),
        MJS_EXTENSION.to_string(),
        CJS_EXTENSION.to_string(),
    ];
    files.extend(node_root_indicators().into_iter().map(|r| r.pattern));

    Indicator::with_root_indicators(
        "JavaScript".to_string(),
        files,
        "#f7df1e".to_string(),
        nerd_icon("e781"),
        6,
        vec![Ecosystem::Npm],
        root_indicators,
    )
}
