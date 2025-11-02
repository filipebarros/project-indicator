use anyhow::Result;
use project_indicator::detection::DetectionEngineBuilder;
use project_indicator::tracking::{ChangeDetected, ResultTracker};
use project_indicator::Config;
use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_complete_workflow() -> Result<()> {
    // Setup
    let storage_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(storage_dir.path().to_path_buf())?);

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    // Create a Rust project
    let project = TempDir::new()?;
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )?;
    fs::create_dir_all(project.path().join("src"))?;
    fs::write(
        project.path().join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;

    // First detection (fresh)
    let result1 = engine.detect(project.path())?;
    assert!(result1.language.is_some());
    assert_eq!(
        result1.language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );

    // Modify the project slightly (add a lib.rs file)
    fs::write(project.path().join("src/lib.rs"), "pub fn test() {}")?;

    // Second detection (still Rust, but now has lib.rs)
    let result2 = engine.detect(project.path())?;
    assert_eq!(
        result2.language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );

    // Wait for background writes to complete
    tracker.flush();

    // Verify snapshots were recorded
    let canonical_path = project.path().canonicalize()?;
    let path_str = canonical_path.to_string_lossy();
    let snapshots = tracker.read_snapshots_for_path(&path_str)?;
    assert_eq!(snapshots.len(), 2);

    // Verify both snapshots detected Rust
    assert_eq!(
        snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );
    assert_eq!(
        snapshots[1].language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );

    // Test statistics
    let stats = tracker.get_path_statistics(&path_str)?;
    assert_eq!(stats.total_detections, 2);
    assert_eq!(stats.language_counts.get("Rust"), Some(&2));

    Ok(())
}

#[test]
fn test_disabled_tracking_has_zero_overhead() -> Result<()> {
    use std::time::Instant;

    let temp_dir = TempDir::new()?;
    let tracker = ResultTracker::with_path_disabled(temp_dir.path().to_path_buf())?;

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(Arc::new(tracker))
        .build();

    let project = TempDir::new()?;
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Measure detection time
    let start = Instant::now();
    engine.detect(project.path())?;
    let duration = start.elapsed();

    // Should be fast (< 50ms even on slow machines)
    assert!(duration.as_millis() < 50);

    // No files should be created
    let entries: Vec<_> = fs::read_dir(temp_dir.path())?.collect();
    assert_eq!(entries.len(), 0);

    Ok(())
}

#[test]
fn test_multiple_projects_tracked_separately() -> Result<()> {
    let storage_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(storage_dir.path().to_path_buf())?);

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    // Create first Rust project
    let project1 = TempDir::new()?;
    fs::write(
        project1.path().join("Cargo.toml"),
        "[package]\nname = \"project1\"",
    )?;
    fs::create_dir_all(project1.path().join("src"))?;
    fs::write(project1.path().join("src/main.rs"), "fn main() {}")?;

    // Create second TypeScript project
    let project2 = TempDir::new()?;
    fs::write(
        project2.path().join("package.json"),
        r#"{"name": "project2", "dependencies": {"typescript": "5.0.0"}}"#,
    )?;
    fs::write(
        project2.path().join("tsconfig.json"),
        r#"{"compilerOptions": {"target": "ES2020"}}"#,
    )?;
    fs::write(
        project2.path().join("index.ts"),
        "const msg: string = 'Hello';",
    )?;

    // Detect both projects
    engine.detect(project1.path())?;
    engine.detect(project2.path())?;

    // Wait for background writes to complete
    tracker.flush();

    // Verify they're tracked separately
    let rust_path = project1.path().canonicalize()?;
    let ts_path = project2.path().canonicalize()?;
    let rust_snapshots = tracker.read_snapshots_for_path(&rust_path.to_string_lossy())?;
    let ts_snapshots = tracker.read_snapshots_for_path(&ts_path.to_string_lossy())?;

    assert_eq!(rust_snapshots.len(), 1);
    assert_eq!(ts_snapshots.len(), 1);

    assert_eq!(
        rust_snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );
    assert_eq!(
        ts_snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
        Some("TypeScript")
    );

    Ok(())
}

#[test]
fn test_language_change_detected() -> Result<()> {
    let storage_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(storage_dir.path().to_path_buf())?);

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    // Use the SAME directory path for both detections to simulate a project changing over time
    let project_dir = TempDir::new()?;
    let project_path = project_dir.path();

    // First: Create a JavaScript project
    fs::write(
        project_path.join("package.json"),
        r#"{"name": "test", "main": "index.js"}"#,
    )?;
    fs::write(project_path.join("index.js"), "console.log('Hello');")?;

    eprintln!("Project path: {:?}", project_path);
    eprintln!("Project path canonical: {:?}", project_path.canonicalize()?);

    let result1 = engine.detect(project_path)?;
    eprintln!(
        "First detection: {:?}",
        result1.language.as_ref().map(|l| &l.name)
    );

    // Debug: List files before cleanup
    eprintln!("\nFiles before cleanup:");
    for entry in fs::read_dir(project_path)?.flatten() {
        eprintln!("  - {:?}", entry.file_name());
    }

    // Clean up JavaScript project completely
    fs::remove_file(project_path.join("index.js"))?;
    fs::remove_file(project_path.join("package.json"))?;

    // Second: Create a completely fresh TypeScript project in the SAME directory
    fs::write(
        project_path.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true
  }
}"#,
    )?;
    fs::create_dir_all(project_path.join("src"))?;
    fs::write(
        project_path.join("src/index.ts"),
        "const msg: string = 'Hello';\nexport default msg;",
    )?;
    fs::write(
        project_path.join("src/types.ts"),
        "export type User = { name: string; };",
    )?;

    // Debug: List ALL files after TypeScript setup (including subdirectories)
    eprintln!("\nFiles after TypeScript setup:");
    for entry in fs::read_dir(project_path)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            eprintln!("  - {:?} (dir)", entry.file_name());
            if let Ok(subentries) = fs::read_dir(&path) {
                for subentry in subentries.flatten() {
                    eprintln!("    - {:?}", subentry.file_name());
                }
            }
        } else {
            eprintln!("  - {:?}", entry.file_name());
        }
    }

    // Sleep to ensure filesystem operations complete and fresh engine has no cached state
    // With a fresh engine, 100ms should be enough for filesystem consistency
    thread::sleep(Duration::from_millis(100));

    // Create a fresh engine to ensure no cached state from first detection
    let config2 = Config::load_default()?;
    let engine2 = DetectionEngineBuilder::new(config2.languages)
        .with_result_tracker(tracker.clone())
        .build();

    let result2 = engine2.detect(project_path)?;
    eprintln!(
        "\nSecond detection: {:?}",
        result2.language.as_ref().map(|l| &l.name)
    );
    eprintln!("Second detection confidence: {}", result2.confidence);

    // Wait for background writes to complete
    tracker.flush();

    // Debug: Check storage directory
    eprintln!("Storage dir: {:?}", storage_dir.path());
    if let Ok(entries) = fs::read_dir(storage_dir.path()) {
        eprintln!("Files in storage dir:");
        for entry in entries.flatten() {
            eprintln!("  - {:?}", entry.path());
            if let Ok(metadata) = entry.metadata() {
                eprintln!("    Size: {} bytes", metadata.len());
            }
        }
    } else {
        eprintln!("  (could not read storage dir)");
    }

    // Use canonical path for consistent querying across platforms
    let canonical_project_path = project_path.canonicalize()?;
    let canonical_path_str = canonical_project_path.to_string_lossy();

    // Debug: Check what snapshots were stored
    let snapshots = tracker.read_snapshots_for_path(&canonical_path_str)?;
    eprintln!("Found {} snapshots after flush", snapshots.len());
    for (i, snapshot) in snapshots.iter().enumerate() {
        eprintln!(
            "Snapshot {}: timestamp={}, language={:?}, path_hash={}",
            i,
            snapshot.timestamp,
            snapshot.language.as_ref().map(|l| &l.name),
            snapshot.path_hash
        );
        eprintln!("    Path: {}", snapshot.path);
    }

    // Verify we have exactly 2 snapshots
    assert_eq!(
        snapshots.len(),
        2,
        "Expected 2 snapshots but found {}. This suggests snapshots are not being stored correctly.",
        snapshots.len()
    );

    // Verify the snapshots have different languages
    let first_lang = snapshots[0].language.as_ref().map(|l| l.name.as_ref());
    let second_lang = snapshots[1].language.as_ref().map(|l| l.name.as_ref());
    eprintln!("Snapshot languages: {:?} -> {:?}", first_lang, second_lang);

    // Verify language change detected
    // Use canonical path for consistency across platforms
    eprintln!(
        "\nQuerying changes for canonical path: {}",
        canonical_path_str
    );

    let changes = tracker.detect_changes(&canonical_path_str)?;

    assert!(
        !changes.is_empty(),
        "Expected changes to be detected for path: {}. Found {} snapshots with languages {:?} -> {:?}",
        canonical_path_str,
        snapshots.len(),
        first_lang,
        second_lang
    );

    // Debug: Print what changes were actually detected
    eprintln!("Detected {} change sets", changes.len());
    for (i, change_set) in changes.iter().enumerate() {
        eprintln!("Change set {}: {} changes", i, change_set.changes.len());
        for change in &change_set.changes {
            eprintln!("  - {:?}", change);
        }
    }

    let has_language_change = changes[0].changes.iter().any(|c| {
        matches!(c, ChangeDetected::LanguageChanged { from, to }
            if from.as_deref() == Some("JavaScript") && to.as_deref() == Some("TypeScript"))
    });
    assert!(
        has_language_change,
        "Expected language change from JavaScript to TypeScript, but changes were: {:?}",
        changes[0].changes
    );

    Ok(())
}

#[test]
fn test_cache_status_tracking() -> Result<()> {
    let storage_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(storage_dir.path().to_path_buf())?);

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    let project = TempDir::new()?;
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Multiple detections should show cache behavior
    engine.detect(project.path())?;
    engine.detect(project.path())?;
    engine.detect(project.path())?;

    // Wait for background writes to complete
    tracker.flush();

    let canonical_path = project.path().canonicalize()?;
    let path_str = canonical_path.to_string_lossy();
    let stats = tracker.get_path_statistics(&path_str)?;

    assert_eq!(stats.total_detections, 3);
    // Cache rate should be between 0 and 100
    assert!(stats.cache_rate >= 0.0);
    assert!(stats.cache_rate <= 100.0);
    // The sum of cached and fresh should equal total
    let fresh_detections = stats.total_detections - stats.cached_detections;
    assert_eq!(
        stats.cached_detections + fresh_detections,
        stats.total_detections
    );

    Ok(())
}

#[test]
fn test_performance_metrics_tracked() -> Result<()> {
    let storage_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(storage_dir.path().to_path_buf())?);

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    let project = TempDir::new()?;
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Run multiple detections
    for _ in 0..5 {
        engine.detect(project.path())?;
    }

    // Wait for background writes to complete
    tracker.flush();

    let canonical_path = project.path().canonicalize()?;
    let path_str = canonical_path.to_string_lossy();
    let stats = tracker.get_path_statistics(&path_str)?;

    // Verify performance metrics are reasonable
    assert!(stats.min_duration_micros > 0);
    assert!(stats.max_duration_micros >= stats.min_duration_micros);
    assert!(stats.median_duration_micros >= stats.min_duration_micros);
    assert!(stats.median_duration_micros <= stats.max_duration_micros);

    Ok(())
}
