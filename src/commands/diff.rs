use anyhow::{Context, Result};
use project_indicator::tracking::formatting::DiffFormatter;
use project_indicator::tracking::{
    handle_tracking_disabled, DetectionSnapshot, ResultTracker, SnapshotDiff,
};
use project_indicator::Config;

pub fn handle_diff_command(from: String, to: Option<String>) -> Result<()> {
    // Load config to get tracking settings
    let config = Config::load_default()?;
    let tracker = ResultTracker::from_config(&config.tracking)?;

    if !tracker.is_enabled() {
        return handle_tracking_disabled();
    }

    // Resolve snapshots
    // from could be either:
    // 1. A snapshot ID (UUID)
    // 2. A path (use latest snapshot for that path)

    let from_snapshot = resolve_snapshot(&tracker, &from)?;

    let to_snapshot = if let Some(to_id) = to {
        resolve_snapshot(&tracker, &to_id)?
    } else {
        // Use most recent snapshot for same path
        tracker
            .get_latest_snapshot(&from_snapshot.path)?
            .context("No recent snapshot found for path")?
    };

    // Compare
    let diff = SnapshotDiff::compare(&from_snapshot, &to_snapshot);

    // Format and display
    let formatted = DiffFormatter::format_diff(&diff);
    println!("{}", formatted);

    if !diff.has_changes() {
        println!("✅ No changes detected between snapshots.\n");
    }

    Ok(())
}

fn resolve_snapshot(tracker: &ResultTracker, identifier: &str) -> Result<DetectionSnapshot> {
    // Try as UUID first
    if identifier.len() == 36 && identifier.contains('-') {
        // Look through all snapshots for this ID
        // (This is inefficient - in production, you'd want an index)
        let all_files = std::fs::read_dir(tracker.storage_path())?;

        for entry in all_files {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                let snapshots = tracker.read_snapshots_from_file(&path)?;
                if let Some(snapshot) = snapshots.into_iter().find(|s| s.snapshot_id == identifier)
                {
                    return Ok(snapshot);
                }
            }
        }

        anyhow::bail!("Snapshot ID not found: {}", identifier);
    }

    // Treat as path
    let latest = tracker
        .get_latest_snapshot(identifier)?
        .context(format!("No snapshots found for path: {}", identifier))?;

    Ok(latest)
}
