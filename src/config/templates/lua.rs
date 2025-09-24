use super::shared::{nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

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
        nerd_icon("e620"),
        8,
        vec![
            simple_framework(
                "OpenResty",
                DetectionType::LuaEcosystem {
                    packages: vec!["lua-resty-core".to_string(), "lua-resty-http".to_string()],
                },
                Some(nerd_icon("f0f7b")),
                Some("#269539"),
                2,
            ),
            simple_framework(
                "Lapis",
                DetectionType::LuaEcosystem {
                    packages: vec!["lapis".to_string()],
                },
                Some(nerd_icon("f448")),
                Some("#ff6b35"),
                3,
            ),
        ],
        vec![
            root_indicator("init.lua", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("main.lua", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("rockspec", 0.85, IndicatorContext::LanguageRoot),
            root_indicator("luarocks.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
