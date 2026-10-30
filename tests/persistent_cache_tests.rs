//! End-to-end tests for the persistent detection cache.

mod common;

use project_indicator::{
    cache::PersistentCache,
    detection::DetectionEngine,
    output::{render, OutputFormat},
};
use std::fs;
use tempfile::TempDir;

fn react_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let project = TempDir::new()?;
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"react": "^18.2.0"}}"#,
    )?;
    fs::write(project.path().join("tsconfig.json"), "{}")?;
    // The cache won't store a result computed from files modified within a
    // few ms of the run (coarse mtime clocks): let the files age past that
    std::thread::sleep(std::time::Duration::from_millis(40));
    Ok(project)
}

/// Detect → store → load must render identically to the fresh result.
#[test]
fn test_cached_result_renders_identically() -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));

    let (engine, config) = common::built_in_engine_and_config();

    let fresh = engine.detect(project.path())?;
    cache.store(
        project.path(),
        None,
        &fresh,
        &config.display,
        &config.detection,
        std::time::SystemTime::now(),
    );

    let (cached, display) = cache
        .load(project.path(), None)
        .ok_or("expected cache hit")?;

    for format in [OutputFormat::Simple, OutputFormat::Json, OutputFormat::Full] {
        let fresh_out = render(&fresh, format, &config.display);
        let cached_out = render(&cached, format, &display);
        assert_eq!(fresh_out, cached_out);
    }
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
        common::json_cli(home.path(), project.path(), extra).output()
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

fn test_engine() -> (DetectionEngine, project_indicator::config::Config) {
    common::built_in_engine_and_config()
}

fn store_fresh(
    cache: &PersistentCache,
    engine: &DetectionEngine,
    config: &project_indicator::config::Config,
    dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let fresh = engine.detect(dir)?;
    cache.store(
        dir,
        None,
        &fresh,
        &config.display,
        &config.detection,
        std::time::SystemTime::now(),
    );
    assert!(cache.load(dir, None).is_some(), "precondition: stored");
    // Let mtimes tick past filesystem granularity before the change
    std::thread::sleep(std::time::Duration::from_millis(30));
    Ok(())
}

/// A project holding `files` (dirs created as needed), detected and stored
/// in a private cache, ready for the test to change
struct Stored {
    _sandbox: TempDir,
    _cache_dir: TempDir,
    project: std::path::PathBuf,
    cache: PersistentCache,
}

fn stored(files: &[(&str, &str)]) -> Result<Stored, Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir_all(&project)?;
    common::write_files(&project, files)?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = test_engine();
    store_fresh(&cache, &engine, &config, &project)?;
    Ok(Stored {
        _sandbox: sandbox,
        _cache_dir: cache_dir,
        project,
        cache,
    })
}

/// A probed source directory (src/main/java) appearing later changes the
/// language, but not the project root's mtime.
#[test]
fn test_probed_directory_created_later_invalidates_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let t = stored(&[("build.gradle.kts", ""), ("src/.keep", "")])?;

    fs::create_dir_all(t.project.join("src").join("main").join("java"))?;
    assert!(
        t.cache.load(&t.project, None).is_none(),
        "a newly created src/main/java must miss"
    );
    Ok(())
}

#[test]
fn test_ignore_edit_that_reincludes_a_project_invalidates_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let t = stored(&[
        (".git/HEAD", ""),
        (".gitignore", "ignored/\n"),
        ("ignored/package.json", "{}"),
    ])?;

    fs::write(t.project.join(".gitignore"), "!ignored/\n")?;

    assert!(t.cache.load(&t.project, None).is_none());
    let (engine, _) = test_engine();
    let fresh = engine.detect(&t.project)?;
    assert_eq!(fresh.nested[0].name, "ignored");
    Ok(())
}

/// A tracked file modified after detection began may belong to a result that
/// was computed from its previous content: such a result must not be stored.
#[test]
fn test_change_during_detection_is_not_cached() -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir(&project)?;
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = test_engine();
    let fresh = engine.detect(&project)?;

    // "Detection started" before the files were last modified
    cache.store(
        &project,
        None,
        &fresh,
        &config.display,
        &config.detection,
        std::time::UNIX_EPOCH,
    );
    assert!(cache.load(&project, None).is_none());
    Ok(())
}
