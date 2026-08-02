use project_indicator::{
    cache::PersistentCache,
    cli::Cli,
    config::{Config, ConfigParser},
    detection::DetectionEngineBuilder,
    output::{OutputFormat, OutputFormatter},
    tracking::ResultTracker,
    types::DetectionMode,
    Result,
};
use std::sync::Arc;

use super::resolve_and_validate_path;

pub fn handle_detect_command(cli: &Cli) -> Result<()> {
    let path = resolve_and_validate_path(cli.path.as_ref())?;

    let format: OutputFormat = cli
        .format
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid format: {}", e))?;

    // CLI overrides change detection results, so they bypass the cache
    // entirely (neither read nor write)
    let cache_eligible = !cli.no_cache && cli.max_depth.is_none() && cli.mode.is_none();
    let config_path = ConfigParser::active_config_path();
    let cache = if cache_eligible {
        PersistentCache::default_location()
    } else {
        None
    };

    if let Some(cache) = &cache {
        if let Some((result, display)) = cache.load(&path, config_path.as_deref()) {
            // Cache hit: render from the cached result and display snapshot,
            // skipping config parsing and engine construction. Note that
            // tracking snapshots are not recorded for cache hits.
            println!("{}", OutputFormatter::new(display).format(&result, format));
            return Ok(());
        }
    }

    let config = Config::load_default()?;
    let mut detection_config = config.detection;

    if let Some(max_depth) = cli.max_depth {
        detection_config.max_depth = max_depth;
    }

    if let Some(mode_str) = &cli.mode {
        detection_config.detection_mode = match mode_str.to_lowercase().as_str() {
            "fast" => DetectionMode::Fast,
            "thorough" => DetectionMode::Thorough,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid detection mode: '{}'. Valid options are 'fast' or 'thorough'",
                    mode_str
                ));
            }
        };
    }

    // Create tracker from config (respects user's tracking settings)
    let tracker = Arc::new(ResultTracker::from_config(&config.tracking)?);

    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(detection_config)
        .with_result_tracker(tracker)
        .build();

    let result = engine.detect(&path)?;

    let display_config = config.display;

    if let Some(cache) = &cache {
        cache.store(&path, config_path.as_deref(), &result, &display_config);
    }

    let formatter = OutputFormatter::new(display_config);

    let output = formatter.format(&result, format);
    println!("{}", output);

    Ok(())
}
