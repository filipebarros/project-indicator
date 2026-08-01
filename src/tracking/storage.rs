use super::comparison::SnapshotDiff;
use super::path_cache::PathCache;
use super::types::DetectionSnapshot;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Tracks and persists detection results to disk.
///
/// Results are stored as JSON Lines (one JSON object per line) for:
/// - Easy appending
/// - Simple parsing
/// - Git-friendly diffs
/// - Streaming reads
///
/// **Performance Optimizations:**
/// - Background thread for non-blocking writes
/// - File handle kept open for reuse
/// - Lazy directory creation
/// - Batch flushing for better throughput
/// - Path canonicalization cache
pub struct ResultTracker {
    storage_path: PathBuf,
    enabled: bool,
    sender: Mutex<Option<Sender<DetectionSnapshot>>>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
    path_cache: PathCache,
}

impl ResultTracker {
    /// Create a new result tracker from configuration.
    ///
    /// Uses the TrackingConfig to determine if tracking is enabled and where to store data.
    pub fn from_config(config: &crate::types::TrackingConfig) -> Result<Self> {
        let storage_path = if let Some(ref path) = config.storage_path {
            PathBuf::from(path)
        } else {
            Self::default_storage_path()?
        };

        Self::new_with_path(storage_path, config.enabled)
    }

    /// Create a new result tracker with default settings (disabled).
    ///
    /// This is primarily for backward compatibility and testing.
    pub fn new() -> Result<Self> {
        Self::new_with_path(Self::default_storage_path()?, false)
    }

    /// Create tracker with custom storage path (always enabled)
    pub fn with_path(path: PathBuf) -> Result<Self> {
        Self::new_with_path(path, true)
    }

    /// Create tracker with custom storage path and disabled tracking (for testing)
    pub fn with_path_disabled(path: PathBuf) -> Result<Self> {
        Self::new_with_path(path, false)
    }

    /// Internal constructor that sets up the background writer thread
    fn new_with_path(storage_path: PathBuf, enabled: bool) -> Result<Self> {
        let dir_created = Arc::new(AtomicBool::new(false));

        let (sender, thread_handle) = if enabled {
            let (tx, rx) = mpsc::channel::<DetectionSnapshot>();
            let path_for_thread = storage_path.clone();
            let dir_flag = Arc::clone(&dir_created);

            // Spawn background writer thread
            let handle = thread::Builder::new()
                .name("tracking-writer".to_string())
                .spawn(move || {
                    Self::background_writer_thread(rx, path_for_thread, dir_flag);
                })
                .context("Failed to spawn tracking writer thread")?;

            (Some(tx), Some(handle))
        } else {
            (None, None)
        };

        Ok(Self {
            storage_path,
            enabled,
            sender: Mutex::new(sender),
            thread_handle: Mutex::new(thread_handle),
            path_cache: PathCache::new(),
        })
    }

    /// Background thread that processes snapshots from the channel
    fn background_writer_thread(
        rx: mpsc::Receiver<DetectionSnapshot>,
        storage_path: PathBuf,
        dir_created: Arc<AtomicBool>,
    ) {
        let mut file_handle: Option<BufWriter<File>> = None;
        let mut current_date: Option<String> = None;
        let mut writes_since_flush = 0;
        const FLUSH_INTERVAL: usize = 10; // Flush every 10 writes

        while let Ok(snapshot) = rx.recv() {
            // Lazy directory creation
            if !dir_created.load(Ordering::Relaxed) {
                if fs::create_dir_all(&storage_path).is_ok() {
                    dir_created.store(true, Ordering::Relaxed);
                } else {
                    continue; // Skip this snapshot if dir creation failed
                }
            }

            // Get today's date
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();

            // Check if we need to rotate the file (new day)
            if current_date.as_ref() != Some(&today) {
                // Flush and close old file
                if let Some(mut writer) = file_handle.take() {
                    let _ = writer.flush();
                }

                // Open new file
                let file_path = storage_path.join(format!("{}.jsonl", today));
                match OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file_path)
                {
                    Ok(file) => {
                        file_handle = Some(BufWriter::with_capacity(64 * 1024, file));
                        current_date = Some(today);
                    }
                    Err(_) => continue, // Skip this snapshot if file open failed
                }
            }

            // Write snapshot using pre-serialized buffer
            if let Some(writer) = file_handle.as_mut() {
                // Use pre-serialized buffer for faster writing
                // This reduces allocations and improves cache locality
                if let Ok(buffer) = snapshot.serialize_to_buffer() {
                    if writer.write_all(&buffer).is_ok() {
                        writes_since_flush += 1;

                        // Periodic flushing (every N writes)
                        if writes_since_flush >= FLUSH_INTERVAL {
                            let _ = writer.flush();
                            writes_since_flush = 0;
                        }
                    }
                }
            }
        }

        // Final flush when channel closes
        if let Some(mut writer) = file_handle {
            let _ = writer.flush();
        }
    }

    /// Get the default storage path based on platform
    ///
    /// - macOS: ~/.cache/project-indicator/snapshots/
    /// - Linux: ~/.cache/project-indicator/snapshots/
    /// - Windows: C:\Users\<name>\AppData\Local\project-indicator\snapshots\
    fn default_storage_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir().context("Could not determine cache directory")?;

        let storage_dir = cache_dir.join("project-indicator").join("snapshots");

        Ok(storage_dir)
    }

    /// Record a detection snapshot to disk (non-blocking)
    ///
    /// Sends the snapshot to a background thread for processing.
    /// This method returns immediately without blocking on I/O.
    pub fn record(&self, snapshot: DetectionSnapshot) -> Result<()> {
        if !self.enabled {
            return Ok(()); // No-op if disabled
        }

        // Send snapshot to background thread (non-blocking)
        let sender_guard = self
            .sender
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock sender"))?;
        if let Some(ref sender) = *sender_guard {
            // Use send instead of blocking - if channel is full, snapshot is dropped
            // This prevents detection from blocking on a slow disk
            sender
                .send(snapshot)
                .context("Failed to send snapshot to background writer")?;
        }

        Ok(())
    }

    /// Check if tracking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the storage path
    pub fn storage_path(&self) -> &PathBuf {
        &self.storage_path
    }

    /// Get a reference to the path cache for use in snapshot creation
    pub fn path_cache(&self) -> &PathCache {
        &self.path_cache
    }

    /// Wait for all pending writes to complete (useful for testing)
    ///
    /// Drops the sender to signal the background thread to finish,
    /// then waits for the thread to complete using the join handle.
    pub fn flush(&self) {
        // Drop the sender to signal the thread to finish
        if let Ok(mut sender_guard) = self.sender.lock() {
            sender_guard.take();
        }

        // Wait for the thread to complete
        if let Ok(mut handle_guard) = self.thread_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                // Join will block until the thread completes
                // We ignore the result since the thread might have already finished
                let _ = handle.join();
            }
        }
    }

    /// Read all snapshots from a specific date
    pub fn read_snapshots_for_date(&self, date: &str) -> Result<Vec<DetectionSnapshot>> {
        let file_path = self.storage_path.join(format!("{}.jsonl", date));

        if !file_path.exists() {
            return Ok(Vec::new()); // No snapshots for this date
        }

        self.read_snapshots_from_file(&file_path)
    }

    /// Read all snapshots from a file
    pub fn read_snapshots_from_file(&self, file_path: &Path) -> Result<Vec<DetectionSnapshot>> {
        let file = File::open(file_path).context("Failed to open snapshot file")?;

        let reader = BufReader::new(file);
        let mut snapshots = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            let line =
                line_result.with_context(|| format!("Failed to read line {}", line_num + 1))?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse JSON line
            let snapshot: DetectionSnapshot = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse snapshot at line {}", line_num + 1))?;

            snapshots.push(snapshot);
        }

        Ok(snapshots)
    }

    /// Read all snapshots for a specific path
    pub fn read_snapshots_for_path(&self, path: &str) -> Result<Vec<DetectionSnapshot>> {
        // Canonicalize the path to match how snapshots are stored
        let canonical_path = self.path_cache.get_canonical(Path::new(path));
        let path_hash = DetectionSnapshot::hash_path(&canonical_path);
        let mut all_snapshots = Vec::new();

        // Read all snapshot files
        let entries =
            fs::read_dir(&self.storage_path).context("Failed to read snapshot directory")?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Only process .jsonl files
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }

            // Read and filter snapshots
            let snapshots = self.read_snapshots_from_file(&path)?;
            all_snapshots.extend(snapshots.into_iter().filter(|s| s.path_hash == path_hash));
        }

        // Sort by timestamp (oldest first)
        all_snapshots.sort_by_key(|s| s.timestamp);

        Ok(all_snapshots)
    }

    /// Get the most recent snapshot for a path
    pub fn get_latest_snapshot(&self, path: &str) -> Result<Option<DetectionSnapshot>> {
        let snapshots = self.read_snapshots_for_path(path)?;
        Ok(snapshots.into_iter().last())
    }

    /// Find changes for a specific path
    ///
    /// Accepts either canonical or non-canonical paths. The path will be
    /// canonicalized to match how snapshots are stored.
    pub fn detect_changes(&self, path: &str) -> Result<Vec<SnapshotDiff>> {
        let snapshots = self.read_snapshots_for_path(path)?;

        if snapshots.len() < 2 {
            return Ok(Vec::new()); // Need at least 2 snapshots to compare
        }

        let mut diffs = Vec::new();

        // Compare consecutive snapshots
        for window in snapshots.windows(2) {
            let from = &window[0];
            let to = &window[1];

            let diff = SnapshotDiff::compare(from, to);

            // Only include if there are actual changes
            if diff.has_changes() {
                diffs.push(diff);
            }
        }

        Ok(diffs)
    }

    /// Get snapshots within a time range
    /// Get statistics for a path
    pub fn get_path_statistics(&self, path: &str) -> Result<PathStatistics> {
        let snapshots = self.read_snapshots_for_path(path)?;

        if snapshots.is_empty() {
            return Err(anyhow::anyhow!("No snapshots found for path: {}", path));
        }

        let durations: Vec<_> = snapshots.iter().map(|s| s.duration_micros).collect();

        let total_detections = snapshots.len();
        let cached_detections = snapshots
            .iter()
            .filter(|s| s.cache_info.detection_from_cache)
            .count();

        // Count language frequencies
        let mut language_counts: HashMap<String, usize> = HashMap::new();
        for snapshot in &snapshots {
            if let Some(ref lang) = snapshot.language {
                *language_counts.entry(lang.name.to_string()).or_insert(0) += 1;
            }
        }

        // We know snapshots is not empty because we checked above
        let first_seen = snapshots
            .first()
            .ok_or_else(|| anyhow::anyhow!("No snapshots found"))?
            .timestamp;
        let last_seen = snapshots
            .last()
            .ok_or_else(|| anyhow::anyhow!("No snapshots found"))?
            .timestamp;

        Ok(PathStatistics {
            path: path.to_string(),
            total_detections,
            cached_detections,
            cache_rate: (cached_detections as f64 / total_detections as f64) * 100.0,
            median_duration_micros: Self::median(&durations),
            min_duration_micros: durations.iter().copied().min().unwrap_or(0),
            max_duration_micros: durations.iter().copied().max().unwrap_or(0),
            language_counts,
            first_seen,
            last_seen,
        })
    }

    /// Calculate median (helper function)
    fn median(numbers: &[u64]) -> u64 {
        if numbers.is_empty() {
            return 0;
        }

        let mut sorted = numbers.to_vec();
        sorted.sort_unstable();

        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            (sorted[mid - 1] + sorted[mid]) / 2
        } else {
            sorted[mid]
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStatistics {
    pub path: String,
    pub total_detections: usize,
    pub cached_detections: usize,
    pub cache_rate: f64,
    pub median_duration_micros: u64,
    pub min_duration_micros: u64,
    pub max_duration_micros: u64,
    pub language_counts: HashMap<String, usize>,
    pub first_seen: u64,
    pub last_seen: u64,
}

impl Default for ResultTracker {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            storage_path: PathBuf::from("."),
            enabled: false,
            sender: Mutex::new(None),
            thread_handle: Mutex::new(None),
            path_cache: PathCache::new(),
        })
    }
}

impl Drop for ResultTracker {
    fn drop(&mut self) {
        // Drop the sender to signal the background thread to exit
        if let Ok(mut sender_guard) = self.sender.lock() {
            sender_guard.take();
        }

        // Wait for the background thread to finish (with timeout to avoid blocking forever)
        if let Ok(mut handle_guard) = self.thread_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                // The background thread will flush and exit gracefully when the sender is dropped
                // We ignore join errors (e.g., if the thread panicked or already finished)
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_snapshot(path: &str, language: &str) -> DetectionSnapshot {
        use std::sync::Arc;

        DetectionSnapshot {
            snapshot_id: DetectionSnapshot::new_id(),
            timestamp: 1234567890, // Use fixed timestamp to avoid unwrap()
            path: path.to_string(),
            path_hash: DetectionSnapshot::hash_path(path),
            language: Some(super::super::types::LanguageResult {
                name: Arc::from(language),
                confidence: 0.95,
                sample_files: vec![],
            }),
            frameworks: vec![],
            confidence: 0.95,
            evidence: super::super::types::EvidenceSnapshot {
                sample_matched_files: vec![],
                root_indicators: vec![],
                early_termination: None,
                total_files_matched: 0,
            },
            cache_info: super::super::types::CacheInfo {
                detection_from_cache: false,
                cached_at: None,
                pattern_cache_hits: 0,
                pattern_cache_misses: 0,
                fs_cache_hits: 0,
                fs_cache_misses: 0,
            },
            duration_micros: 3200,
            files_scanned: 42,
        }
    }

    #[test]
    fn test_record_and_read_snapshot() -> Result<()> {
        // Create temporary directory for testing
        let temp_dir = TempDir::new()?;
        let tracker = ResultTracker::with_path(temp_dir.path().to_path_buf())?;

        // Create and record a snapshot
        let snapshot = create_test_snapshot("/test/path", "Rust");
        tracker.record(snapshot.clone())?;

        // Wait for background thread to finish writing
        tracker.flush();

        // Read back snapshots for today
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let read_snapshots = tracker.read_snapshots_for_date(&today)?;

        // Should have one snapshot
        assert_eq!(read_snapshots.len(), 1);
        assert_eq!(read_snapshots[0].path, "/test/path");
        assert_eq!(
            read_snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
            Some("Rust")
        );

        Ok(())
    }

    #[test]
    fn test_multiple_snapshots() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let tracker = ResultTracker::with_path(temp_dir.path().to_path_buf())?;

        // Record multiple snapshots
        tracker.record(create_test_snapshot("/path1", "Rust"))?;
        tracker.record(create_test_snapshot("/path2", "TypeScript"))?;
        tracker.record(create_test_snapshot("/path1", "Rust"))?; // Same path again

        // Wait for background thread to finish writing
        tracker.flush();

        // Read all for today
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let all_snapshots = tracker.read_snapshots_for_date(&today)?;
        assert_eq!(all_snapshots.len(), 3);

        // Read for specific path
        let path1_snapshots = tracker.read_snapshots_for_path("/path1")?;
        assert_eq!(path1_snapshots.len(), 2);

        Ok(())
    }

    #[test]
    fn test_disabled_tracker_is_noop() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let tracker = ResultTracker::with_path_disabled(temp_dir.path().to_path_buf())?;

        // Recording should succeed but do nothing
        let snapshot = create_test_snapshot("/test", "Rust");
        tracker.record(snapshot)?;

        // No files should be created
        let entries: Vec<_> = fs::read_dir(temp_dir.path())?.collect();
        assert_eq!(entries.len(), 0);

        Ok(())
    }
}
