//! End-to-end tests for the persistent detection cache.

use project_indicator::{
    cache::PersistentCache,
    config::TemplateGenerator,
    detection::DetectionEngine,
    output::{render, OutputFormat},
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

    let config = TemplateGenerator::generate_template(Some("full"))?;
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );

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
        let fresh_out = render(&fresh, format.clone(), &config.display);
        let cached_out = render(&cached, format, &display);
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
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );

    let fresh = engine.detect(project.path())?;
    assert!(
        fresh.frameworks.iter().any(|f| f.framework.name == "React"),
        "precondition: React detected"
    );
    cache.store(
        project.path(),
        None,
        &fresh,
        &config.display,
        &config.detection,
        std::time::SystemTime::now(),
    );
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
            .env("USERPROFILE", home.path())
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

/// Scan evidence is relative to the DISCOVERED project root, which can be an
/// ancestor of the invocation directory. The cache must track those files at
/// their real location: editing/removing one must invalidate the entry for
/// the subdirectory invocation.
#[test]
fn test_subdir_invocation_invalidates_on_root_evidence_change(
) -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let subdir = project.path().join("app");
    fs::create_dir(&subdir)?;
    let home = TempDir::new()?;

    let run = || -> Result<std::process::Output, std::io::Error> {
        Command::new(env!("CARGO_BIN_EXE_project-indicator"))
            .args([subdir.to_str().unwrap_or("."), "--format", "json"])
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
            .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
            .output()
    };

    let first = run()?;
    let first_out = String::from_utf8_lossy(&first.stdout);
    assert!(
        first_out.contains("React"),
        "subdir invocation must detect the root project, got: {first_out}"
    );

    // Editing a root manifest that is scan/framework evidence (NOT the
    // root-discovery indicator, which is tracked absolutely) must still
    // invalidate the subdirectory's cache entry
    fs::write(
        project.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"vue": "^3.4.0"}}"#,
    )?;
    let second = run()?;
    let second_out = String::from_utf8_lossy(&second.stdout);
    assert!(
        second_out.contains("Vue") && !second_out.contains("React"),
        "stale cache served after root evidence changed, got: {second_out}"
    );
    Ok(())
}

/// A config file that fails to parse (old schema, garbage) must not break
/// detection: the binary falls back to the built-in template and exits 0.
#[test]
fn test_unparsable_config_soft_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let project = react_project()?;
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join("config.toml"),
        "this is not [ valid toml {{{",
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_project-indicator"))
        .args([project.path().to_str().unwrap_or("."), "--format", "json"])
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .output()?;

    assert!(
        output.status.success(),
        "detection must succeed despite the broken config"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"indicator\":\"TypeScript\""),
        "fallback template must detect the project, got: {stdout}"
    );
    Ok(())
}

fn engine_with_depth(
    max_depth: usize,
) -> Result<(DetectionEngine, project_indicator::config::Config), Box<dyn std::error::Error>> {
    let mut config = TemplateGenerator::generate_template(Some("full"))?;
    config.detection.max_depth = max_depth;
    // Track only the sandbox parent: the shared temp dir above it churns
    // under parallel tests and would make every load miss for the wrong reason
    config.detection.max_upward_traversal = 1;
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );
    Ok((engine, config))
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

/// With max_depth > 1 the scan lists subdirectories, so a change inside one
/// of them can change the result and must invalidate the entry.
#[test]
fn test_change_in_scanned_subdirectory_invalidates_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir_all(project.join(".git"))?;
    fs::create_dir(project.join("src"))?;
    fs::write(project.join("src").join("App.csproj"), "<Project/>")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = engine_with_depth(3)?;
    store_fresh(&cache, &engine, &config, &project)?;

    fs::write(project.join("src").join("Cargo.toml"), "[package]")?;
    assert!(
        cache.load(&project, None).is_none(),
        "a new file in a scanned subdirectory must miss"
    );
    Ok(())
}

/// Ignore files shape the walk; editing one in place leaves every directory
/// mtime untouched.
#[test]
fn test_in_place_gitignore_edit_invalidates_entry() -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir(&project)?;
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
    fs::write(project.join(".gitignore"), "target\n")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = engine_with_depth(1)?;
    store_fresh(&cache, &engine, &config, &project)?;

    fs::write(project.join(".gitignore"), "target\nbuild\n")?;
    assert!(
        cache.load(&project, None).is_none(),
        "an edited .gitignore must miss"
    );
    Ok(())
}

/// A probed source directory (src/main/java) appearing later changes the
/// language, but not the project root's mtime.
#[test]
fn test_probed_directory_created_later_invalidates_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir_all(project.join("src"))?;
    fs::write(project.join("build.gradle.kts"), "")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = engine_with_depth(1)?;
    store_fresh(&cache, &engine, &config, &project)?;

    fs::create_dir_all(project.join("src").join("main").join("java"))?;
    assert!(
        cache.load(&project, None).is_none(),
        "a newly created src/main/java must miss"
    );
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
    let (engine, config) = engine_with_depth(1)?;
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

/// A framework detected by a file in a subdirectory (Tauri's `src-tauri/`)
/// appears without touching the project root or any listed directory.
#[test]
fn test_file_exists_probe_in_a_subdirectory_invalidates_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir_all(project.join("src-tauri"))?;
    fs::write(
        project.join("package.json"),
        r#"{"dependencies":{"react":"18"}}"#,
    )?;
    fs::write(project.join("src-tauri").join("main.rs"), "fn main() {}")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = engine_with_depth(1)?;
    store_fresh(&cache, &engine, &config, &project)?;

    fs::write(project.join("src-tauri").join("tauri.conf.json"), "{}")?;
    assert!(
        cache.load(&project, None).is_none(),
        "a new src-tauri/tauri.conf.json must miss"
    );
    Ok(())
}

/// A lockfile only a matcher reads (no indicator pattern lists pubspec.lock)
/// can change the frameworks when edited in place.
#[test]
fn test_file_read_only_by_a_matcher_invalidates_entry() -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let project = sandbox.path().join("proj");
    fs::create_dir(&project)?;
    fs::write(project.join("pubspec.yaml"), "name: x\n")?;
    fs::write(
        project.join("pubspec.lock"),
        "packages:\n  http:\n    dependency: direct\n",
    )?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (engine, config) = engine_with_depth(1)?;
    store_fresh(&cache, &engine, &config, &project)?;

    fs::write(
        project.join("pubspec.lock"),
        "packages:\n  flutter:\n    dependency: direct\n",
    )?;
    assert!(
        cache.load(&project, None).is_none(),
        "an edited pubspec.lock must miss"
    );
    Ok(())
}

/// With require_vcs_root the VCS marker above the root decides whether the
/// root counts, so removing it must invalidate even a root found at `dir`.
#[test]
fn test_vcs_marker_above_the_root_is_watched() -> Result<(), Box<dyn std::error::Error>> {
    let sandbox = TempDir::new()?;
    let start = sandbox.path().join("a").join("b");
    fs::create_dir_all(&start)?;
    fs::create_dir(sandbox.path().join("a").join(".git"))?;
    fs::write(start.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
    let cache_dir = TempDir::new()?;
    let cache = PersistentCache::at_base(cache_dir.path().join("results"));
    let (_, mut config) = engine_with_depth(1)?;
    config.detection.require_vcs_root = true;
    config.detection.max_upward_traversal = 2;
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );
    store_fresh(&cache, &engine, &config, &start)?;

    fs::remove_dir(sandbox.path().join("a").join(".git"))?;
    assert!(
        cache.load(&start, None).is_none(),
        "removing the .git that validated the root must miss"
    );
    Ok(())
}
