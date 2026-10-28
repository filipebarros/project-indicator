use project_indicator::{
    cache::PersistentCache, cli::Cli, config::parser, detection::DetectionEngine, output::render,
    Result,
};

use super::resolve_and_validate_path;

pub fn handle_detect_command(cli: &Cli) -> Result<()> {
    let path = resolve_and_validate_path(cli.path.as_ref())?;

    let format = cli.format;
    let repository_root = project_indicator::detection::repository::find_vcs_root(&path);
    let cache_path = repository_root.clone().unwrap_or_else(|| path.clone());

    let cache_eligible = !cli.no_cache;
    let config_path = parser::active_config_path();
    let cache = if cache_eligible {
        PersistentCache::default_location()
    } else {
        None
    };

    if let Some(cache) = &cache {
        if let Some((result, display)) = cache.load(&cache_path, config_path.as_deref()) {
            log::debug!("persistent cache hit for {}", cache_path.display());
            outln!("{}", render(&result, format, &display));
            return Ok(());
        }
    }

    let started = project_indicator::cache::detection_started();
    let config = parser::load_default()?;
    let engine = DetectionEngine::with_config(
        config.indicators,
        config.frameworks,
        config.detection.clone(),
    );

    let result = engine.detect_with_vcs_root(&path, repository_root.as_deref())?;

    if let Some(cache) = &cache {
        cache.store(
            &cache_path,
            config_path.as_deref(),
            &result,
            &config.display,
            &config.detection,
            started,
        );
    }

    outln!("{}", render(&result, format, &config.display));

    Ok(())
}
