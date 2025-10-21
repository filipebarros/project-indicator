use anyhow::Result;
use chrono::{Local, TimeZone};
use colored::*;
use std::path::{Path, PathBuf};

/// Format a Unix timestamp to a human-readable string
pub fn format_timestamp(timestamp: u64) -> String {
    let dt = Local
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .unwrap_or_else(Local::now);

    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Display message when tracking is not enabled and return Ok(())
pub fn handle_tracking_disabled() -> Result<()> {
    println!("{}", "⚠️  Tracking is not enabled.".yellow());
    println!("Enable in your config file:");
    println!("  [tracking]");
    println!("  enabled = true");
    Ok(())
}

/// Resolve and canonicalize a path, returning the original path if canonicalization fails
pub fn resolve_path(path: Option<String>) -> Result<PathBuf> {
    let target_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        std::env::current_dir()?
    };

    Ok(target_path.canonicalize().unwrap_or(target_path))
}

/// Get the canonical path as a string
pub fn canonical_path_string(path: &Path) -> Result<String> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Ok(canonical.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        // Test with a known timestamp (2024-01-01 00:00:00 UTC)
        let timestamp = 1704067200;
        let formatted = format_timestamp(timestamp);
        // Just check that it's formatted correctly (not empty and has expected format)
        assert!(!formatted.is_empty());
        assert!(formatted.contains('-'));
        assert!(formatted.contains(':'));
    }

    #[test]
    fn test_resolve_path_none() -> Result<()> {
        let path = resolve_path(None)?;
        assert!(path.exists());
        Ok(())
    }

    #[test]
    fn test_resolve_path_some() -> Result<()> {
        let path = resolve_path(Some(".".to_string()))?;
        assert!(path.exists());
        Ok(())
    }
}
