use super::shared::{nerd_icon, node_lockfiles, root_indicator};
use crate::constants::{CJS_EXTENSION, JS_EXTENSION, MJS_EXTENSION};
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_javascript_indicator() -> Indicator {
    let mut files = vec![
        JS_EXTENSION.to_string(),
        MJS_EXTENSION.to_string(),
        CJS_EXTENSION.to_string(),
    ];
    files.extend(node_lockfiles());

    let mut indicators = super::shared::node_lockfile_root_indicators();
    indicators.push(root_indicator(
        "node_modules",
        0.6,
        IndicatorContext::LanguageRoot,
    ));

    Indicator::with_root_indicators(
        "JavaScript".to_string(),
        files,
        "#f7df1e".to_string(),
        nerd_icon("e781"),
        6,
        vec![Ecosystem::Npm],
        indicators,
    )
}
