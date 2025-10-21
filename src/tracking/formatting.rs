use super::comparison::{ChangeDetected, SnapshotDiff};
use super::utils::format_timestamp;
use colored::*;
use std::fmt::Write;

pub struct DiffFormatter;

impl DiffFormatter {
    /// Format a diff for terminal display
    pub fn format_diff(diff: &SnapshotDiff) -> String {
        let mut output = String::new();

        let _ = writeln!(output, "\n{}", "═".repeat(60));
        output.push_str("📊 Detection Change Report\n");
        let _ = writeln!(output, "{}", "═".repeat(60));
        let _ = writeln!(output, "Path: {}", diff.path.cyan());
        let _ = writeln!(
            output,
            "From: {} ({})",
            diff.from_snapshot,
            format_timestamp(diff.from_timestamp)
        );
        let _ = writeln!(
            output,
            "To:   {} ({})",
            diff.to_snapshot,
            format_timestamp(diff.to_timestamp)
        );
        let _ = writeln!(output, "{}\n", "─".repeat(60));

        if diff.changes.is_empty() {
            let _ = writeln!(output, "{}", "✓ No changes detected".green());
            return output;
        }

        let _ = writeln!(output, "Changes Detected: {}\n", diff.changes.len());

        for (i, change) in diff.changes.iter().enumerate() {
            let _ = writeln!(output, "{}. {}", i + 1, Self::format_change(change));
        }

        output
    }

    fn format_change(change: &ChangeDetected) -> String {
        match change {
            ChangeDetected::LanguageChanged { from, to } => {
                format!(
                    "{} Language changed: {} → {}",
                    "⚠️".yellow(),
                    from.as_deref().unwrap_or("None").red(),
                    to.as_deref().unwrap_or("None").green()
                )
            }
            ChangeDetected::FrameworkAdded { name, confidence } => {
                format!(
                    "{} Framework added: {} (confidence: {:.2})",
                    "➕".green(),
                    name.green(),
                    confidence
                )
            }
            ChangeDetected::FrameworkRemoved { name } => {
                format!("{} Framework removed: {}", "➖".red(), name.red())
            }
            ChangeDetected::ConfidenceChanged { from, to, delta } => {
                let arrow = if *delta > 0.0 {
                    "↑".green()
                } else {
                    "↓".red()
                };
                format!(
                    "{} Confidence changed: {:.2} → {:.2} ({})",
                    arrow,
                    from,
                    to,
                    if *delta > 0.0 {
                        format!("+{:.2}", delta).green()
                    } else {
                        format!("{:.2}", delta).red()
                    }
                )
            }
            ChangeDetected::CacheStatusChanged {
                from_cached,
                to_cached,
            } => {
                format!(
                    "💾 Cache status: {} → {}",
                    if *from_cached { "cached" } else { "fresh" },
                    if *to_cached { "cached" } else { "fresh" }
                )
            }
            ChangeDetected::PerformanceChanged {
                from_micros,
                to_micros,
                delta_micros,
            } => {
                let arrow = if *delta_micros < 0 {
                    "↑".green()
                } else {
                    "↓".red()
                };
                format!(
                    "{} Performance: {}µs → {}µs ({:+}µs)",
                    arrow, from_micros, to_micros, delta_micros
                )
            }
        }
    }
}
