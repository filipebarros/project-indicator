//! Snapshot-style detection tests over real project fixtures.
//!
//! Each directory in `tests/fixtures/` is a minimal real project. This test
//! runs the full detection pipeline (built-in full template, as used when no
//! user config exists) against a copy of each fixture and asserts the
//! detected language and framework set.
//!
//! Fixtures are copied to a temp directory first: detecting them in place
//! would let upward root-indicator traversal find this repo's own
//! `Cargo.toml`/`.git` and report everything as Rust.
//!
//! The table records intended behavior: framework detection runs whenever a
//! language is selected — the presence of a framework dependency in a
//! manifest is evidence in its own right, not a reward for a high language
//! confidence score.

use project_indicator::{config::TemplateGenerator, detection::DetectionEngineBuilder};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// (fixture dir, expected language, expected frameworks)
const EXPECTATIONS: &[(&str, &str, &[&str])] = &[
    ("cpp", "C++", &[]),
    ("csharp-aspnet", "C#", &["ASP.NET Core"]),
    ("dart-flutter", "Dart", &["Flutter"]),
    ("deno", "Deno", &[]),
    ("deno-react", "Deno", &["React"]),
    ("elixir", "Elixir", &["Phoenix"]),
    ("go", "Go", &[]),
    ("go-gin", "Go", &["Gin"]),
    ("java-spring", "Java", &["Spring Boot"]),
    ("js-react", "JavaScript", &["React"]),
    ("js-svelte", "JavaScript", &["Svelte", "Vite"]),
    ("kotlin", "Kotlin", &["Ktor"]),
    ("kotlin-spring", "Kotlin", &["Spring Boot"]),
    ("lua", "Lua", &["LÖVE"]),
    ("nix", "Nix", &[]),
    ("php-laravel", "PHP", &["Laravel"]),
    ("python-django", "Python", &["Django"]),
    ("python-fastapi", "Python", &["FastAPI"]),
    ("ruby-rails", "Ruby", &["Rails"]),
    ("rust", "Rust", &[]),
    ("rust-axum", "Rust", &["Axum"]),
    ("scala-akka", "Scala", &["Akka HTTP"]),
    ("swift", "Swift", &["Vapor"]),
    ("terraform", "Terraform", &[]),
    ("ts-nextjs", "TypeScript", &["React", "Next.js"]),
];

fn copy_dir(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[test]
fn test_all_fixtures_detect_as_expected() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    let config = TemplateGenerator::generate_template(Some("full"))?;
    let engine = DetectionEngineBuilder::new(config.indicators.clone(), config.frameworks.clone())
        .with_config(config.detection.clone())
        .build();

    let mut failures = Vec::new();

    for (name, expected_language, expected_frameworks) in EXPECTATIONS {
        let staged = TempDir::new()?;
        copy_dir(&fixtures_root.join(name), staged.path())?;

        let result = engine.detect(staged.path())?;

        let language = result
            .indicator
            .as_ref()
            .map(|l| l.name.as_str())
            .unwrap_or("<none>");
        if language != *expected_language {
            failures.push(format!(
                "{name}: expected language {expected_language}, got {language}"
            ));
        }

        let mut got: Vec<&str> = result
            .frameworks
            .iter()
            .map(|f| f.framework.name.as_str())
            .collect();
        got.sort_unstable();
        let mut want: Vec<&str> = expected_frameworks.to_vec();
        want.sort_unstable();
        if got != want {
            failures.push(format!("{name}: expected frameworks {want:?}, got {got:?}"));
        }
    }

    assert!(
        failures.is_empty(),
        "fixture expectations failed:\n  {}",
        failures.join("\n  ")
    );
    Ok(())
}

/// Every fixture directory must have a row in the expectations table, so new
/// fixtures can't be added without locking in their behavior.
#[test]
fn test_every_fixture_has_an_expectation() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    let mut missing = Vec::new();
    for entry in fs::read_dir(&fixtures_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !EXPECTATIONS.iter().any(|(n, _, _)| *n == name) {
                missing.push(name);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "fixtures without expectations: {missing:?}"
    );
    Ok(())
}
