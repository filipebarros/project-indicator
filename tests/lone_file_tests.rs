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

fn assert_detects(case: &str, files: &[&str], expected: Option<&str>) -> TestResult {
    assert_eq!(detected(files)?.as_deref(), expected, "{case}: {files:?}");
    Ok(())
}

#[test]
fn test_minimal_directory_detection_cases() -> TestResult {
    let cases: &[(&str, &[&str], Option<&str>)] = &[
        ("lone JavaScript", &["app.js"], Some("JavaScript")),
        ("lone TypeScript", &["index.ts"], Some("TypeScript")),
        ("lone Python", &["main.py"], Some("Python")),
        ("lone Rust", &["main.rs"], Some("Rust")),
        ("lone Go", &["main.go"], Some("Go")),
        ("lone Ruby", &["app.rb"], Some("Ruby")),
        ("Makefile alone", &["Makefile"], None),
        ("unrelated files", &["notes.txt", "photo.png"], None),
        (
            "manifest outranks a stray script",
            &["Cargo.toml", "script.py"],
            Some("Rust"),
        ),
        (
            "Kotlin source directory",
            &["src/main/kotlin/com/acme/Main.kt"],
            Some("Kotlin"),
        ),
        (
            "Java source directory",
            &["src/main/java/com/acme/Main.java"],
            Some("Java"),
        ),
        (
            "Scala source directory",
            &["src/main/scala/Main.scala"],
            Some("Scala"),
        ),
        (
            "unrelated JVM layout",
            &["src/main/resources/app.properties", "src/test/notes.txt"],
            None,
        ),
    ];

    for (case, files, expected) in cases {
        assert_detects(case, files, *expected)?;
    }
    Ok(())
}
