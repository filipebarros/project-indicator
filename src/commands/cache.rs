use project_indicator::{cache::PersistentCache, cli::CacheAction, Result};

pub fn handle_cache_command(action: CacheAction) -> Result<()> {
    let cache = PersistentCache::default_location()
        .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?;

    match action {
        CacheAction::Clear => {
            cache.clear()?;
            println!("✅ Cache cleared");
            Ok(())
        }
        CacheAction::Stats => {
            let (entries, bytes) = cache.stats()?;
            println!("Cache Statistics");
            println!("================");
            println!("📊 Entries: {}", entries);
            println!("💾 Size: {} KiB", bytes / 1024);
            Ok(())
        }
    }
}
