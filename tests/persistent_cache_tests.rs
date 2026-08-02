//! End-to-end tests for the persistent detection cache.

use project_indicator::{
    cache::PersistentCache,
    config::TemplateGenerator,
    detection::DetectionEngineBuilder,
    output::{OutputFormat, OutputFormatter},
};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn react_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let project = TempDir::new()?;
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"react": "^18.2.0"}}"#,
    )?;
    fs::write(project.path().join("tsconfig.json"), "{}")?;
    Ok(project)
}

/// Detect → store → load must render identically to the fresh result.
#[test]
fn test_cached_result_renders_identically() -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));

    let config = TemplateGenerator::generate_template(Some("full"))?;
    let engine = DetectionEngineBuilder::new(config.languages.clone())
        .with_config(config.detection.clone())
        .build();

    let fresh = engine.detect(project.path())?;
    cache.store(project.path(), None, &fresh, &config.display);

    let (cached, display) = cache
        .load(project.path(), None)
        .ok_or("expected cache hit")?;

    for format in [OutputFormat::Simple, OutputFormat::Json, OutputFormat::Full] {
        let fresh_out = OutputFormatter::new(config.display.clone()).format(&fresh, format.clone());
        let cached_out = OutputFormatter::new(display.clone()).format(&cached, format);
        assert_eq!(fresh_out, cached_out);
    }
    Ok(())
}

/// Editing a manifest that produced the result must invalidate the entry.
#[test]
fn test_manifest_edit_invalidates_entry() -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));

    let config = TemplateGenerator::generate_template(Some("full"))?;
    let engine = DetectionEngineBuilder::new(config.languages.clone())
        .with_config(config.detection.clone())
        .build();

    let fresh = engine.detect(project.path())?;
    assert!(
        fresh.frameworks.iter().any(|f| f.framework.name == "React"),
        "precondition: React detected"
    );
    cache.store(project.path(), None, &fresh, &config.display);
    assert!(cache.load(project.path(), None).is_some());

    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"vue": "^3.0.0"}}"#,
    )?;

    assert!(
        cache.load(project.path(), None).is_none(),
        "manifest edit must invalidate"
    );
    Ok(())
}

/// Full binary: two runs with an isolated cache produce identical output,
/// populate the cache, and --no-cache leaves it untouched.
#[test]
fn test_binary_end_to_end_cache_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let home = TempDir::new()?;
    let cache_home = home.path().join("xdg-cache");

    let run = |extra: &[&str]| -> Result<std::process::Output, std::io::Error> {
        let mut args = vec![project.path().to_str().unwrap_or("."), "--format", "json"];
        args.extend_from_slice(extra);
        Command::new(env!("CARGO_BIN_EXE_project-indicator"))
            .args(&args)
            .env("HOME", home.path())
            .env("XDG_CACHE_HOME", &cache_home)
            .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
            .output()
    };

    let first = run(&[])?;
    assert!(first.status.success());
    let results_dir = cache_home.join("project-indicator").join("results");
    assert!(results_dir.exists(), "first run must populate the cache");
    let entry_count = fs::read_dir(&results_dir)?.count();
    assert_eq!(entry_count, 1);

    let second = run(&[])?;
    assert!(second.status.success());
    assert_eq!(
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&second.stdout),
        "cached output must be identical to fresh output"
    );

    // --no-cache must not read or write
    fs::remove_dir_all(&results_dir)?;
    let third = run(&["--no-cache"])?;
    assert!(third.status.success());
    assert!(
        !results_dir.exists(),
        "--no-cache must not repopulate the cache"
    );
    assert_eq!(
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&third.stdout)
    );
    Ok(())
}
