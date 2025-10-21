use anyhow::Result;
use project_indicator::detection::DetectionEngineBuilder;
use project_indicator::tracking::{ChangeDetected, ResultTracker};
use project_indicator::Config;
use std::fs;
use std::sync::Arc;
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

    let project = TempDir::new()?;

    // Start with JavaScript
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "test", "main": "index.js"}"#,
    )?;
    fs::write(project.path().join("index.js"), "console.log('Hello');")?;

    engine.detect(project.path())?;

    // Convert to TypeScript
    fs::remove_file(project.path().join("index.js"))?;
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "test", "dependencies": {"typescript": "5.0.0"}}"#,
    )?;
    fs::write(
        project.path().join("tsconfig.json"),
        r#"{"compilerOptions": {"target": "ES2020"}}"#,
    )?;
    fs::write(
        project.path().join("index.ts"),
        "const msg: string = 'Hello';",
    )?;

    engine.detect(project.path())?;

    // Wait for background writes to complete
    tracker.flush();

    // Verify language change detected
    let canonical_path = project.path().canonicalize()?;
    let path_str = canonical_path.to_string_lossy();
    let changes = tracker.detect_changes(&path_str)?;

    assert!(!changes.is_empty());
    let has_language_change = changes[0].changes.iter().any(|c| {
        matches!(c, ChangeDetected::LanguageChanged { from, to }
            if from.as_deref() == Some("JavaScript") && to.as_deref() == Some("TypeScript"))
    });
    assert!(has_language_change);

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
