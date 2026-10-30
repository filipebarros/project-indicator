use project_indicator::{
    cache::{CacheLookup, PersistentCache},
    cli::Cli,
    config::parser,
    detection::DetectionEngine,
    output::render,
    Result,
};

use super::resolve_and_validate_path;

pub fn handle_detect_command(cli: &Cli) -> Result<()> {
    let path = resolve_and_validate_path(cli.path.as_ref())?;

    let format = cli.format;
    let repository_root = project_indicator::detection::repository::find_vcs_root(&path);
    let cache_path = repository_root.clone().unwrap_or_else(|| path.clone());

    let started = project_indicator::cache::detection_started();
    let cache_eligible = !cli.no_cache;
    let config_path = parser::active_config_path();
    let cache = if cache_eligible {
        PersistentCache::default_location()
    } else {
        None
    };

    let repository_update = if let Some(cache) = &cache {
        match cache.repository_lookup_with_ttl(
            &cache_path,
            config_path.as_deref(),
            cli.cache_ttl.map(std::time::Duration::from_secs),
        ) {
            CacheLookup::Hit(result, display) => {
                log::debug!("persistent cache hit for {}", cache_path.display());
                outln!("{}", render(&result, format, &display));
                return Ok(());
            }
            CacheLookup::Update(update) => Some(update),
            CacheLookup::Miss => None,
        }
    } else {
        None
    };
    let config = parser::load_default()?;
    let engine = DetectionEngine::with_config(
        config.indicators,
        config.frameworks,
        config.detection.clone(),
    );

    let result = if let (Some(root), Some(update)) = (repository_root.as_deref(), repository_update)
    {
        project_indicator::detection::repository::detect_repository_incremental(
            &engine, root, &update,
        )
    } else {
        engine.detect_with_vcs_root(&path, repository_root.as_deref())?
    };

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
