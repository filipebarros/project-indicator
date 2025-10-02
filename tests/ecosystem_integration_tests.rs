use project_indicator::detection::caches::ParsedFileCache;
use project_indicator::detection::matchers::ecosystems::{
    check_elixir_ecosystem, check_kotlin_ecosystem, check_swift_ecosystem,
};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn test_kotlin_ktor_detection() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("kotlin");
    let dependencies = vec![
        "ktor-server-core".to_string(),
        "kotlinx-coroutines-core".to_string(),
    ];

    let (found_deps, evidence) = check_kotlin_ecosystem(&path, &dependencies, &cache)?;

    assert!(!found_deps.is_empty(), "Should find Kotlin dependencies");
    assert!(
        found_deps.contains(&"ktor-server-core".to_string()),
        "Should find ktor-server-core"
    );
    assert!(
        found_deps.contains(&"kotlinx-coroutines-core".to_string()),
        "Should find kotlinx-coroutines-core"
    );
    assert!(
        evidence.contains(&"build.gradle.kts".to_string()),
        "Should detect build.gradle.kts"
    );

    Ok(())
}

#[test]
fn test_kotlin_missing_dependency() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("kotlin");
    let dependencies = vec!["spring-boot".to_string()];

    let (found_deps, _evidence) = check_kotlin_ecosystem(&path, &dependencies, &cache)?;

    assert!(
        found_deps.is_empty(),
        "Should not find non-existent dependencies"
    );

    Ok(())
}

#[test]
fn test_swift_vapor_detection() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("swift");
    let dependencies = vec!["vapor".to_string(), "Alamofire".to_string()];

    let (found_deps, evidence) = check_swift_ecosystem(&path, &dependencies, &cache)?;

    assert!(!found_deps.is_empty(), "Should find Swift dependencies");
    assert!(
        found_deps.contains(&"vapor".to_string()),
        "Should find vapor"
    );
    assert!(
        found_deps.contains(&"Alamofire".to_string()),
        "Should find Alamofire"
    );
    assert!(
        evidence.contains(&"Package.swift".to_string()),
        "Should detect Package.swift"
    );

    Ok(())
}

#[test]
fn test_swift_case_sensitivity() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("swift");
    // Test with different casing - should still match
    let dependencies = vec!["Vapor".to_string()];

    let (found_deps, _evidence) = check_swift_ecosystem(&path, &dependencies, &cache)?;

    assert!(!found_deps.is_empty(), "Should find Vapor (capitalized)");

    Ok(())
}

#[test]
fn test_elixir_phoenix_detection() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("elixir");
    let dependencies = vec![
        "phoenix".to_string(),
        "ecto_sql".to_string(),
        "phoenix_live_view".to_string(),
    ];

    let (found_deps, evidence) = check_elixir_ecosystem(&path, &dependencies, &cache)?;

    assert!(!found_deps.is_empty(), "Should find Elixir dependencies");
    assert!(
        found_deps.contains(&"phoenix".to_string()),
        "Should find phoenix"
    );
    assert!(
        found_deps.contains(&"ecto_sql".to_string()),
        "Should find ecto_sql"
    );
    assert!(
        found_deps.contains(&"phoenix_live_view".to_string()),
        "Should find phoenix_live_view"
    );
    assert!(
        evidence.contains(&"mix.exs".to_string()),
        "Should detect mix.exs"
    );

    Ok(())
}

#[test]
fn test_elixir_missing_dependency() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("elixir");
    let dependencies = vec!["rails".to_string()];

    let (found_deps, _evidence) = check_elixir_ecosystem(&path, &dependencies, &cache)?;

    assert!(
        found_deps.is_empty(),
        "Should not find non-Elixir dependencies"
    );

    Ok(())
}

#[test]
fn test_nonexistent_project_path() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = PathBuf::from("/nonexistent/path");
    let dependencies = vec!["phoenix".to_string()];

    let (found_deps, evidence) = check_elixir_ecosystem(&path, &dependencies, &cache)?;

    assert!(
        found_deps.is_empty(),
        "Should return empty for non-existent path"
    );
    assert!(
        evidence.is_empty(),
        "Should have no evidence for non-existent path"
    );

    Ok(())
}

#[test]
fn test_empty_dependency_list() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();
    let path = fixture_path("kotlin");
    let dependencies = vec![];

    let (found_deps, _evidence) = check_kotlin_ecosystem(&path, &dependencies, &cache)?;

    assert!(
        found_deps.is_empty(),
        "Should return empty for empty dependency list"
    );

    Ok(())
}

#[test]
fn test_all_ecosystems_with_partial_matches() -> anyhow::Result<()> {
    let cache = ParsedFileCache::new();

    // Kotlin: some deps match, some don't
    let kotlin_path = fixture_path("kotlin");
    let kotlin_deps = vec![
        "ktor-server-core".to_string(),
        "nonexistent-dep".to_string(),
    ];
    let (found, _) = check_kotlin_ecosystem(&kotlin_path, &kotlin_deps, &cache)?;
    assert_eq!(
        found.len(),
        1,
        "Should find only matching Kotlin dependency"
    );

    // Swift: some deps match, some don't
    let swift_path = fixture_path("swift");
    let swift_deps = vec!["vapor".to_string(), "missing-lib".to_string()];
    let (found, _) = check_swift_ecosystem(&swift_path, &swift_deps, &cache)?;
    assert_eq!(found.len(), 1, "Should find only matching Swift dependency");

    // Elixir: some deps match, some don't
    let elixir_path = fixture_path("elixir");
    let elixir_deps = vec!["phoenix".to_string(), "fake_package".to_string()];
    let (found, _) = check_elixir_ecosystem(&elixir_path, &elixir_deps, &cache)?;
    assert_eq!(
        found.len(),
        1,
        "Should find only matching Elixir dependency"
    );

    Ok(())
}
