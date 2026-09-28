use super::shared::{root_indicator, root_indicator_grouped};
use crate::types::{Ecosystem, Indicator, IndicatorContext};

pub fn create_deno_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Deno".to_string(),
        vec![
            "deno.json".to_string(),
            "deno.jsonc".to_string(),
            "deno.lock".to_string(),
        ],
        "#70ffaf".to_string(),
        "🦕".to_string(),
        // Higher priority than TypeScript (6): a project with deno.json is a
        // Deno project even though its *.ts files also match TypeScript
        5,
        vec![Ecosystem::Npm],
        vec![
            // JSON vs JSONC is a mutually-exclusive config-format choice —
            // max weight, not summed.
            root_indicator_grouped(
                "deno.json",
                0.95,
                IndicatorContext::RuntimeRoot,
                "deno-config",
            ),
            root_indicator_grouped(
                "deno.jsonc",
                0.95,
                IndicatorContext::RuntimeRoot,
                "deno-config",
            ),
            // Not grouped: the lockfile co-occurs with whichever config
            // format is present.
            root_indicator("deno.lock", 0.8, IndicatorContext::RuntimeRoot),
        ],
    )
}
