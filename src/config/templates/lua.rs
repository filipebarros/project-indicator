use super::shared::{file_framework, nerd_icon, root_indicator};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_lua_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Lua".to_string(),
        vec![
            "*.lua".to_string(),
            "*.luac".to_string(),
            "*.moon".to_string(),
            "init.lua".to_string(),
            "main.lua".to_string(),
            "conf.lua".to_string(),
            "*.rockspec".to_string(),
            "luarocks.lock".to_string(),
        ],
        "#000080".to_string(),
        nerd_icon("e826"),
        8,
        vec![Ecosystem::Luarocks],
        vec![
            root_indicator("init.lua", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("main.lua", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("*.rockspec", 0.85, IndicatorContext::LanguageRoot),
            root_indicator("luarocks.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn lua_frameworks() -> Vec<Framework> {
    vec![file_framework(
        "LÖVE",
        &[Ecosystem::Luarocks],
        &["conf.lua"],
        None,
        Some("#e74a99"),
        1,
    )]
}
