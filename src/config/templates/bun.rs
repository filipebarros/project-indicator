use super::shared::{root_indicator, root_indicator_grouped};
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
            // Text vs binary lockfile format is a mutually-exclusive choice
            // (Bun switched formats between versions) — max weight, not summed.
            root_indicator_grouped(
                "bun.lock",
                0.95,
                IndicatorContext::RuntimeRoot,
                "bun-lockfile",
            ),
            root_indicator_grouped(
                "bun.lockb",
                0.95,
                IndicatorContext::RuntimeRoot,
                "bun-lockfile",
            ),
            // Not grouped: bunfig.toml co-occurs with either lockfile format
            // (see the bun-react fixture, which has both bun.lock and
            // bunfig.toml together).
            root_indicator("bunfig.toml", 0.9, IndicatorContext::RuntimeRoot),
        ],
    )
}
