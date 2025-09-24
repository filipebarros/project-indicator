use project_indicator::{cli::CacheAction, config::Config, detection::DetectionCache, Result};

pub fn handle_cache_command(action: CacheAction) -> Result<()> {
    let config = Config::load_default()?;

    if !config.cache.enabled {
        println!("❌ Cache is disabled in configuration");
        println!("Enable cache by setting 'enabled = true' in [cache] section");
        return Ok(());
    }

    let cache = DetectionCache::new(config.cache);

    match action {
        CacheAction::Clear => {
            cache.clear();
            println!("✅ Cache cleared successfully");
            Ok(())
        }
        CacheAction::Stats => {
            let stats = cache.stats();
            println!("Cache Statistics");
            println!("================");
            println!("📊 Entries: {}", stats.entries);
            println!("✅ Cache hits: {}", stats.hits);
            println!("❌ Cache misses: {}", stats.misses);
            println!("🔄 Invalidations: {}", stats.invalidations);

            if stats.hits + stats.misses > 0 {
                let hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64 * 100.0;
                println!("📈 Hit rate: {:.1}%", hit_rate);
            }

            Ok(())
        }
    }
}
