use super::shared::root_indicator;
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_bun_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Bun".to_string(),
        vec![
            "bun.lock".to_string(),
            "bun.lockb".to_string(),
            "bunfig.toml".to_string(),
        ],
        "#fbf0df".to_string(),
        "🥟".to_string(),
        // Higher priority than TypeScript/JavaScript (6): a project with a
        // bun lockfile is a Bun project even though its sources also match
        5,
        vec![Ecosystem::Npm],
        vec![
            root_indicator("bun.lock", 0.95, IndicatorContext::RuntimeRoot),
            root_indicator("bun.lockb", 0.95, IndicatorContext::RuntimeRoot),
            root_indicator("bunfig.toml", 0.9, IndicatorContext::RuntimeRoot),
        ],
    )
}
