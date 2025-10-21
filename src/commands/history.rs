use anyhow::{Context, Result};
use colored::*;
use project_indicator::tracking::{format_timestamp, handle_tracking_disabled, ResultTracker};
use project_indicator::Config;
use std::path::PathBuf;

pub fn handle_history_command(
    path: Option<String>,
    limit: usize,
    changes_only: bool,
) -> Result<()> {
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
        std::env::current_dir().context("Failed to get current directory")?
    };

    let canonical = target_path.canonicalize().unwrap_or(target_path.clone());
    let path_str = canonical.to_string_lossy();

    println!("{} {}\n", "History for:".bold(), path_str.cyan());

    // Read snapshots
    let mut snapshots = tracker.read_snapshots_for_path(&path_str)?;

    if snapshots.is_empty() {
        println!("{}", "No detection history found for this path.".yellow());
        println!("\nThis means either:");
        println!("  • This path has never been detected");
        println!("  • Tracking was recently enabled");
        println!("  • Snapshots were cleared");
        return Ok(());
    }

    // Filter if changes_only
    if changes_only {
        let changes = tracker.detect_changes(&path_str)?;
        let change_snapshot_ids: std::collections::HashSet<_> = changes
            .iter()
            .flat_map(|d| vec![&d.from_snapshot, &d.to_snapshot])
            .collect();

        snapshots.retain(|s| change_snapshot_ids.contains(&s.snapshot_id));
    }

    // Limit results
    snapshots.reverse(); // Most recent first
    snapshots.truncate(limit);

    // Display
    println!(
        "{:<20} {:<15} {:<30} {:<10} {:<10}",
        "Time".bold(),
        "Language".bold(),
        "Frameworks".bold(),
        "Duration".bold(),
        "Source".bold()
    );
    println!("{}", "─".repeat(90));

    for snapshot in &snapshots {
        let time = format_timestamp(snapshot.timestamp);
        let lang = snapshot
            .language
            .as_ref()
            .map(|l| l.name.as_ref())
            .unwrap_or("None");

        let fw_str = if snapshot.frameworks.is_empty() {
            "-".to_string()
        } else {
            snapshot
                .frameworks
                .iter()
                .map(|f| f.name.as_ref())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let duration = format!("{:.1}ms", snapshot.duration_micros as f64 / 1000.0);
        let source = if snapshot.cache_info.detection_from_cache {
            "cached".dimmed()
        } else {
            "fresh".green()
        };

        println!(
            "{:<20} {:<15} {:<30} {:<10} {}",
            time,
            lang.cyan(),
            fw_str,
            duration,
            source
        );
    }

    println!("\n{} {} detections shown", "Total:".bold(), snapshots.len());

    Ok(())
}
