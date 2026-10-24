use project_indicator::{
    cache::PersistentCache, cli::Cli, config::ConfigParser, detection::DetectionEngine,
    output::render, Result,
};

use super::resolve_and_validate_path;

pub fn handle_detect_command(cli: &Cli) -> Result<()> {
    let path = resolve_and_validate_path(cli.path.as_ref())?;

    let format = cli.format;

    // CLI overrides change detection results, so they bypass the cache
    // entirely (neither read nor write)
    let cache_eligible = !cli.no_cache && cli.max_depth.is_none();
    let config_path = ConfigParser::active_config_path();
    let cache = if cache_eligible {
        PersistentCache::default_location()
    } else {
        None
    };

    if let Some(cache) = &cache {
        if let Some((result, display)) = cache.load(&path, config_path.as_deref()) {
            outln!("{}", render(&result, format, &display));
            return Ok(());
        }
    }

    let started = project_indicator::cache::detection_started();
    let config = ConfigParser::load_default()?;
    let mut detection_config = config.detection;
    let cached_detection = detection_config.clone();

    if let Some(max_depth) = cli.max_depth {
        detection_config.max_depth = max_depth;
    }

    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, detection_config);

    let result = engine.detect(&path)?;

    // A timed-out scan is a partial, load-dependent sample: never persist it
    if let (Some(cache), false) = (&cache, result.evidence.scan_truncated) {
        cache.store(
            &path,
            config_path.as_deref(),
            &result,
            &config.display,
            &cached_detection,
            started,
        );
    }

    outln!("{}", render(&result, format, &config.display));

    Ok(())
}
