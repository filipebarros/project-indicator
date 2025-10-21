use anyhow::Result;
use colored::*;
use project_indicator::tracking::{format_timestamp, handle_tracking_disabled, ResultTracker};
use project_indicator::Config;
use std::path::PathBuf;

pub fn handle_stats_command(path: Option<String>, _since: Option<String>) -> Result<()> {
    // Load config to get tracking settings
    let config = Config::load_default()?;
    let tracker = ResultTracker::from_config(&config.tracking)?;

    if !tracker.is_enabled() {
        return handle_tracking_disabled();
    }

    // Resolve path
    let target_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        std::env::current_dir()?
    };

    let canonical = target_path.canonicalize().unwrap_or(target_path);
    let path_str = canonical.to_string_lossy();

    // Get statistics
    let stats = tracker.get_path_statistics(&path_str)?;

    // Display
    println!("\n{} {}\n", "📊 Statistics for:".bold(), path_str.cyan());
    println!("{}", "═".repeat(60));

    println!("\n{}", "Detection Summary:".bold());
    println!("  Total detections:   {}", stats.total_detections);
    println!(
        "  Cached detections:  {} ({:.1}%)",
        stats.cached_detections, stats.cache_rate
    );
    println!(
        "  Fresh detections:   {}",
        stats.total_detections - stats.cached_detections
    );

    println!("\n{}", "Performance:".bold());
    println!(
        "  Median duration:    {:.1}ms",
        stats.median_duration_micros as f64 / 1000.0
    );
    println!(
        "  Min duration:       {:.1}ms",
        stats.min_duration_micros as f64 / 1000.0
    );
    println!(
        "  Max duration:       {:.1}ms",
        stats.max_duration_micros as f64 / 1000.0
    );

    println!("\n{}", "Languages Detected:".bold());
    for (lang, count) in &stats.language_counts {
        let percentage = (*count as f64 / stats.total_detections as f64) * 100.0;
        println!("  {:<15} {} ({:.1}%)", lang.cyan(), count, percentage);
    }

    println!("\n{}", "Timeline:".bold());
    println!("  First seen: {}", format_timestamp(stats.first_seen));
    println!("  Last seen:  {}", format_timestamp(stats.last_seen));

    println!();

    Ok(())
}
