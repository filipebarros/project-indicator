//! A directory holding a single source file is that language's directory:
//! the glyph must not depend on how many patterns the language declares.
//! A generic exact-name file (Makefile) is NOT evidence on its own.

mod common;

use project_indicator::config::built_in_config;
use project_indicator::detection::DetectionEngine;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn detected(files: &[&str]) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);
    let dir = TempDir::new()?;
    let entries: Vec<(&str, &str)> = files.iter().map(|f| (*f, "")).collect();
    common::write_files(dir.path(), &entries)?;
    Ok(engine
        .detect(dir.path())?
        .indicator
        .map(|indicator| indicator.name.clone()))
}

fn assert_detects(files: &[&str], expected: Option<&str>) -> TestResult {
    assert_eq!(detected(files)?.as_deref(), expected, "{files:?}");
    Ok(())
}

#[test]
fn test_lone_javascript_file() -> TestResult {
    assert_detects(&["app.js"], Some("JavaScript"))
}

#[test]
fn test_lone_typescript_file() -> TestResult {
    assert_detects(&["index.ts"], Some("TypeScript"))
}

#[test]
fn test_lone_python_file() -> TestResult {
    assert_detects(&["main.py"], Some("Python"))
}

#[test]
fn test_lone_rust_file() -> TestResult {
    assert_detects(&["main.rs"], Some("Rust"))
}

#[test]
fn test_lone_go_file() -> TestResult {
    assert_detects(&["main.go"], Some("Go"))
}

#[test]
fn test_lone_ruby_file() -> TestResult {
    assert_detects(&["app.rb"], Some("Ruby"))
}

#[test]
fn test_a_makefile_alone_is_not_a_language() -> TestResult {
    assert_detects(&["Makefile"], None)
}

#[test]
fn test_unrelated_files_are_not_a_language() -> TestResult {
    assert_detects(&["notes.txt", "photo.png"], None)
}

#[test]
fn test_a_real_manifest_still_outranks_a_stray_script() -> TestResult {
    assert_detects(&["Cargo.toml", "script.py"], Some("Rust"))
}

/// A standard JVM source directory is a language signal even with no build
/// file (a sources-only checkout): it is found by a `stat`, whatever the
/// depth, so the `.kt` files five levels down never need to be scanned.
#[test]
fn test_sources_only_kotlin_checkout() -> TestResult {
    assert_detects(&["src/main/kotlin/com/acme/Main.kt"], Some("Kotlin"))
}

#[test]
fn test_sources_only_java_checkout() -> TestResult {
    assert_detects(&["src/main/java/com/acme/Main.java"], Some("Java"))
}

#[test]
fn test_sources_only_scala_checkout() -> TestResult {
    assert_detects(&["src/main/scala/Main.scala"], Some("Scala"))
}

#[test]
fn test_an_unrelated_src_layout_is_not_a_jvm_project() -> TestResult {
    assert_detects(
        &["src/main/resources/app.properties", "src/test/notes.txt"],
        None,
    )
}
