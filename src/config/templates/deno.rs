use super::shared::{dependency_framework, root_indicator, root_indicator_grouped};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_deno_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Deno".to_string(),
        vec![
            "deno.json".to_string(),
            "deno.jsonc".to_string(),
            "deno.lock".to_string(),
        ],
        "#70ffaf".to_string(),
        "".to_string(),
        // Higher priority than TypeScript (6): a project with deno.json is a
        // Deno project even though its *.ts files also match TypeScript
        5,
        // Npm: Deno projects routinely use npm packages (and package.json
        // when bridging); Deno: the deno.json imports map
        vec![Ecosystem::Npm, Ecosystem::Deno],
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

pub fn deno_frameworks() -> Vec<Framework> {
    vec![
        // Fresh 1 imports via the `$fresh/` alias, Fresh 2 via
        // `jsr:@fresh/core`; both are imports-map entries
        dependency_framework(
            "Fresh",
            &[Ecosystem::Deno],
            &["fresh", "@fresh/core"],
            None,
            Some("#FFDB1E"),
            1,
        ),
        dependency_framework(
            "Oak",
            &[Ecosystem::Deno],
            &["oak"],
            None,
            Some("#6B8E23"),
            2,
        ),
        // Hono runs on Deno and Node alike; both ecosystems surface it
        dependency_framework(
            "Hono",
            &[Ecosystem::Deno, Ecosystem::Npm],
            &["hono"],
            None,
            Some("#E36002"),
            2,
        ),
    ]
}
