use super::shared::{
    create_angular_framework, create_astro_framework, create_nextjs_framework,
    create_react_framework, create_vue_framework, nerd_icon, node_lockfiles, root_indicator,
};
use crate::constants::{CJS_EXTENSION, JS_EXTENSION, MJS_EXTENSION};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_javascript_language() -> ProjectIndicator {
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

    ProjectIndicator::with_root_indicators(
        "JavaScript".to_string(),
        files,
        "#f7df1e".to_string(),
        nerd_icon("e781"),
        6,
        vec![
            create_react_framework(),
            create_vue_framework(),
            create_angular_framework(),
            create_nextjs_framework(),
            create_astro_framework(),
        ],
        indicators,
    )
}
