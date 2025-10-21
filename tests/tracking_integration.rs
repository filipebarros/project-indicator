use project_indicator::detection::DetectionEngineBuilder;
use project_indicator::tracking::ResultTracker;
use project_indicator::Config;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_tracking_integration() -> anyhow::Result<()> {
    // Create temp directory for tracking storage
    let temp_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(temp_dir.path().to_path_buf())?);

    // Load config
    let config = Config::load_default()?;

    // Create engine with tracker
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();

    // Create test project
    let project_dir = TempDir::new()?;
    std::fs::create_dir(project_dir.path().join(".git"))?; // Add .git to stop upward traversal
    std::fs::write(
        project_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )?;

    // Detect
    let result = engine.detect(project_dir.path())?;

    // Verify detection worked
    assert!(result.language.is_some());
    assert_eq!(
        result.language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );

    // Wait for background writes to complete
    tracker.flush();

    // Verify snapshot was recorded
    // Need to canonicalize the path to match what was recorded
    let canonical_path = project_dir.path().canonicalize()?;
    let snapshots = tracker.read_snapshots_for_path(&canonical_path.to_string_lossy())?;

    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );
    assert!(!snapshots[0].cache_info.detection_from_cache);

    Ok(())
}

#[test]
fn test_tracking_disabled_by_default() -> anyhow::Result<()> {
    // Create tracker without enabling it
    let temp_dir = TempDir::new()?;
    let tracker = ResultTracker::with_path_disabled(temp_dir.path().to_path_buf())?;

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(Arc::new(tracker))
        .build();

    let project_dir = TempDir::new()?;
    std::fs::write(
        project_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Detect
    engine.detect(project_dir.path())?;

    // Verify no snapshots were recorded
    let entries: Vec<_> = std::fs::read_dir(temp_dir.path())?.collect();
    assert_eq!(entries.len(), 0, "No files should be created when disabled");

    Ok(())
}

#[test]
fn test_multiple_detections_tracked() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let tracker = Arc::new(ResultTracker::with_path(temp_dir.path().to_path_buf())?);

    let config = Config::load_default()?;

    // Create Rust project
    let rust_project = TempDir::new()?;
    std::fs::create_dir(rust_project.path().join(".git"))?; // Add .git to stop upward traversal
    std::fs::write(
        rust_project.path().join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Create Python project
    let python_project = TempDir::new()?;
    std::fs::create_dir(python_project.path().join(".git"))?; // Add .git to stop upward traversal
    std::fs::write(python_project.path().join("requirements.txt"), "requests\n")?;

    // Detect both with separate engine instances to avoid shared cache state
    let rust_engine = DetectionEngineBuilder::new(config.languages.clone())
        .with_result_tracker(tracker.clone())
        .build();
    rust_engine.detect(rust_project.path())?;

    let python_engine = DetectionEngineBuilder::new(config.languages)
        .with_result_tracker(tracker.clone())
        .build();
    python_engine.detect(python_project.path())?;

    // Wait for background writes to complete
    tracker.flush();

    // Verify both were tracked
    // Need to canonicalize paths to match what was recorded
    let rust_canonical = rust_project.path().canonicalize()?;
    let python_canonical = python_project.path().canonicalize()?;

    let rust_snapshots = tracker.read_snapshots_for_path(&rust_canonical.to_string_lossy())?;
    let python_snapshots = tracker.read_snapshots_for_path(&python_canonical.to_string_lossy())?;

    assert_eq!(rust_snapshots.len(), 1);
    assert_eq!(python_snapshots.len(), 1);
    assert_eq!(
        rust_snapshots[0].language.as_ref().map(|l| l.name.as_ref()),
        Some("Rust")
    );
    assert_eq!(
        python_snapshots[0]
            .language
            .as_ref()
            .map(|l| l.name.as_ref()),
        Some("Python")
    );

    Ok(())
}
