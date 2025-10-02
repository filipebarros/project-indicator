use super::shared::{nerd_icon, root_indicator};
use crate::types::{IndicatorContext, ProjectIndicator};

pub fn create_lua_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Lua".to_string(),
        vec![
            "*.lua".to_string(),
            "*.luac".to_string(),
            "*.moon".to_string(),
            "init.lua".to_string(),
            "main.lua".to_string(),
            "conf.lua".to_string(),
            "rockspec".to_string(),
            "luarocks.lock".to_string(),
        ],
        "#000080".to_string(),
        nerd_icon("e826"),
        8,
        vec![],
        vec![
            root_indicator("init.lua", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("main.lua", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("rockspec", 0.85, IndicatorContext::LanguageRoot),
            root_indicator("luarocks.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
