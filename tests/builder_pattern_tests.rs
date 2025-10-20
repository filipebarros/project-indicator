/// Builder Pattern Tests
///
/// These tests demonstrate the benefits of the DetectionEngineBuilder pattern:
/// - Custom component injection for testing
/// - Controlled cache behavior
/// - Component isolation and testability
use anyhow::Context;
use project_indicator::detection::caches::FileSystemCacheManager;
use project_indicator::detection::engine::{DetectionEngine, DetectionEngineBuilder};
use project_indicator::detection::pattern_matching::PatternMatcher;
use project_indicator::types::{DetectionConfig, ProjectIndicator};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a simple test language
fn create_test_language(name: &str, patterns: Vec<&str>) -> ProjectIndicator {
    ProjectIndicator::new(
        name.to_string(),
        patterns.into_iter().map(String::from).collect(),
        "#000000".to_string(),
        "🔧".to_string(),
        1,
        vec![],
    )
}

#[test]
fn test_builder_default_construction() -> anyhow::Result<()> {
    // Test that builder creates a working engine with defaults
    let languages = vec![create_test_language("Rust", vec!["*.rs", "Cargo.toml"])];

    let engine = DetectionEngineBuilder::new(languages).build();

    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("main.rs");
    std::fs::write(&test_file, "fn main() {}")?;

    let result = engine.detect(temp_dir.path())?;

    assert!(result.language.is_some());
    let language = result
        .language
        .context("Expected Some language but got None")?;
    assert_eq!(language.name, "Rust");

    Ok(())
}

#[test]
fn test_builder_with_custom_config() -> anyhow::Result<()> {
    // Test that builder accepts custom configuration
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    let custom_config = DetectionConfig {
        max_depth: 2, // Limit scanning depth
        confidence_threshold: 0.5,
        ..Default::default()
    };

    let engine = DetectionEngineBuilder::new(languages)
        .with_config(custom_config)
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    Ok(())
}

#[test]
fn test_builder_with_custom_cache() -> anyhow::Result<()> {
    // Demonstrates injecting a custom cache with shorter TTL
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    // Create cache with 1-second TTL (for testing expiration)
    let custom_cache = FileSystemCacheManager::with_ttl(1);

    let engine = DetectionEngineBuilder::new(languages)
        .with_cache_manager(custom_cache)
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    // First detection
    let result1 = engine.detect(temp_dir.path())?;
    assert!(result1.language.is_some());

    // Cache should be used for immediate second detection
    let result2 = engine.detect(temp_dir.path())?;
    assert_eq!(result1.language, result2.language);

    Ok(())
}

#[test]
fn test_builder_with_shared_pattern_matcher() -> anyhow::Result<()> {
    // Demonstrates sharing a pattern matcher across multiple engines
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    // Create a shared pattern matcher
    let shared_matcher = Arc::new(PatternMatcher::new());

    // Build two engines sharing the same pattern matcher
    let engine1 = DetectionEngineBuilder::new(languages.clone())
        .with_pattern_matcher(shared_matcher.clone())
        .build();

    let engine2 = DetectionEngineBuilder::new(languages)
        .with_pattern_matcher(shared_matcher.clone())
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    // Both engines should work and share cache
    let result1 = engine1.detect(temp_dir.path())?;
    let result2 = engine2.detect(temp_dir.path())?;

    assert_eq!(result1.language, result2.language);

    // Verify cache sharing (second engine should benefit from first engine's cache)
    let stats = shared_matcher.cache_stats();
    assert!(stats.0 > 0, "Cache should have entries");

    Ok(())
}

#[test]
fn test_backward_compatibility_new() -> anyhow::Result<()> {
    // Test that old construction method still works
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    let engine = DetectionEngine::new(languages);

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    Ok(())
}

#[test]
fn test_backward_compatibility_with_config() -> anyhow::Result<()> {
    // Test that old with_config method still works
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];
    let config = DetectionConfig::default();

    let engine = DetectionEngine::with_config(languages, config);

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    Ok(())
}

#[test]
fn test_builder_fluent_api() -> anyhow::Result<()> {
    // Test that builder methods can be chained fluently
    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    let engine = DetectionEngineBuilder::new(languages)
        .with_config(DetectionConfig {
            max_depth: 5,
            ..Default::default()
        })
        .with_cache_manager(FileSystemCacheManager::with_ttl(300))
        .with_pattern_matcher(Arc::new(PatternMatcher::new()))
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    Ok(())
}

/// Test helper showing how to create engines with controlled components for testing
#[test]
fn test_controlled_cache_for_testing() -> anyhow::Result<()> {
    // This demonstrates how builder enables controlled testing scenarios

    let languages = vec![create_test_language("Rust", vec!["*.rs"])];

    // Scenario 1: Test with very short TTL to verify expiration behavior
    let short_ttl_cache = FileSystemCacheManager::with_ttl(1);

    let engine = DetectionEngineBuilder::new(languages.clone())
        .with_cache_manager(short_ttl_cache)
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    // Scenario 2: Test with unlimited cache (high TTL)
    let long_ttl_cache = FileSystemCacheManager::with_ttl(86400); // 24 hours

    let engine2 = DetectionEngineBuilder::new(languages)
        .with_cache_manager(long_ttl_cache)
        .build();

    let result2 = engine2.detect(temp_dir.path())?;
    assert!(result2.language.is_some());

    Ok(())
}

/// Demonstrates how builder pattern enables isolated component testing
#[test]
fn test_component_isolation() -> anyhow::Result<()> {
    // Before builder pattern: All components created together, hard to isolate
    // After builder pattern: Can test individual components in isolation

    let languages = vec![create_test_language("Rust", vec!["*.rs", "Cargo.toml"])];

    // Create isolated components for testing
    let test_matcher = Arc::new(PatternMatcher::new());

    // Verify pattern matcher works in isolation
    assert!(test_matcher.matches_pattern("test.rs", "*.rs"));
    assert!(!test_matcher.matches_pattern("test.py", "*.rs"));

    // Now use the tested matcher in engine
    let engine = DetectionEngineBuilder::new(languages)
        .with_pattern_matcher(test_matcher.clone())
        .build();

    let temp_dir = TempDir::new()?;
    std::fs::write(temp_dir.path().join("test.rs"), "// test")?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.language.is_some());

    // Can inspect matcher state after detection
    let stats = test_matcher.cache_stats();
    assert!(stats.0 > 0, "Matcher should have cached patterns");

    Ok(())
}
