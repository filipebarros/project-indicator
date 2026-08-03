//! Common test utilities for unit tests across the codebase.
//!
//! This module provides shared test helpers for creating test fixtures like
//! detection results, temporary directories, caches, and other common test objects.

use crate::types::{DetectionResult, DetectionType, Framework, FrameworkMatch, Indicator};
use std::{fs, sync::Arc};
use tempfile::TempDir;

/// Creates a basic test `DetectionResult` with TypeScript language and React framework.
///
/// This is useful for testing output formatting, caching, and other features that
/// need a populated `DetectionResult`.
///
/// # Returns
///
/// A `DetectionResult` with:
/// - Language: TypeScript (confidence 0.9)
/// - Framework: React
/// - Default icon and color values
pub fn create_test_result() -> DetectionResult {
    let language = Indicator::new(
        "TypeScript".to_string(),
        vec!["tsconfig.json".to_string(), "*.ts".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![],
    );

    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    DetectionResult::new(Some(Arc::new(language)), vec![framework_match], 0.9)
}

/// Creates a test `DetectionResult` with custom language but no frameworks.
///
/// Useful for testing language-only detection scenarios.
///
/// # Arguments
///
/// * `language_name` - The name of the language
/// * `confidence` - The confidence score (0.0 to 1.0)
///
/// # Returns
///
/// A `DetectionResult` with the specified language and confidence, but no frameworks.
pub fn create_test_result_language_only(language_name: &str, confidence: f32) -> DetectionResult {
    let language = Indicator::new(
        language_name.to_string(),
        vec!["*.rs".to_string()],
        "#DEA584".to_string(),
        "".to_string(),
        1,
        vec![],
    );

    DetectionResult::new(Some(Arc::new(language)), vec![], confidence)
}

/// Creates a temporary directory with a basic Rust project structure.
///
/// Creates:
/// - `Cargo.toml` with basic package metadata
/// - `main.rs` in the root
/// - `src/lib.rs`
///
/// # Returns
///
/// A `TempDir` containing the test project structure. The directory will be
/// automatically cleaned up when the `TempDir` is dropped.
///
/// # Errors
///
/// Returns an error if file or directory creation fails.
pub fn create_test_rust_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"")?;
    fs::write(root.join("main.rs"), "fn main() {}")?;

    let src_dir = root.join("src");
    fs::create_dir(&src_dir)?;
    fs::write(src_dir.join("lib.rs"), "// lib")?;

    Ok(temp_dir)
}

/// Creates a temporary directory with basic test files.
///
/// Creates:
/// - `file1.rs` and `file2.rs` with simple Rust code
/// - `.gitignore` with target/ entry
/// - `src/nested/` directory structure with `deep.rs`
///
/// Useful for testing file scanning and traversal functionality.
///
/// # Returns
///
/// A `TempDir` containing the test file structure.
///
/// # Errors
///
/// Returns an error if file or directory creation fails.
pub fn create_test_directory() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create some test files
    fs::write(root.join("file1.rs"), "fn main() {}")?;
    fs::write(root.join("file2.rs"), "fn test() {}")?;
    fs::write(root.join(".gitignore"), "target/")?;

    // Create nested directory
    let nested = root.join("src").join("nested");
    fs::create_dir_all(&nested)?;
    fs::write(nested.join("deep.rs"), "// deep file")?;

    Ok(temp_dir)
}

/// Creates a test `Indicator` with the specified name and file patterns.
///
/// Uses default color (#FF0000) and icon (🔥) values.
///
/// # Arguments
///
/// * `name` - The language name
/// * `patterns` - File patterns to match
///
/// # Returns
///
/// A `Indicator` with the specified configuration.
pub fn create_test_indicator(name: &str, patterns: Vec<&str>) -> Indicator {
    Indicator::new(
        name.to_string(),
        patterns.iter().map(|s| s.to_string()).collect(),
        "#FF0000".to_string(),
        "🔥".to_string(),
        1,
        vec![],
    )
}

/// Creates a test `Framework` with the specified name and detection type.
///
/// Uses default priority (1) and no icon/color.
///
/// # Arguments
///
/// * `name` - The framework name
/// * `detection` - The detection method
///
/// # Returns
///
/// A `Framework` with the specified configuration.
pub fn create_test_framework(name: &str, detection: DetectionType) -> Framework {
    Framework {
        name: name.to_string(),
        ecosystems: vec![],
        detection,
        icon: None,
        color: None,
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_result() {
        let result = create_test_result();

        if let Some(ref language) = result.indicator {
            assert_eq!(language.name, "TypeScript");
        } else {
            panic!("Language should be present");
        }
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "React");
        assert_eq!(result.confidence, 0.9);
    }

    #[test]
    fn test_create_test_result_language_only() {
        let result = create_test_result_language_only("Rust", 0.8);

        if let Some(ref language) = result.indicator {
            assert_eq!(language.name, "Rust");
        } else {
            panic!("Language should be present");
        }
        assert!(result.frameworks.is_empty());
        assert_eq!(result.confidence, 0.8);
    }

    #[test]
    fn test_create_test_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_rust_project()?;
        let root = temp_dir.path();

        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("main.rs").exists());
        assert!(root.join("src").join("lib.rs").exists());
        Ok(())
    }

    #[test]
    fn test_create_test_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_directory()?;
        let root = temp_dir.path();

        assert!(root.join("file1.rs").exists());
        assert!(root.join("file2.rs").exists());
        assert!(root.join(".gitignore").exists());
        assert!(root.join("src").join("nested").join("deep.rs").exists());
        Ok(())
    }

    #[test]
    fn test_create_test_indicator() {
        let lang = create_test_indicator("Python", vec!["*.py", "requirements.txt"]);

        assert_eq!(lang.name, "Python");
        assert_eq!(lang.files.len(), 2);
        assert_eq!(lang.priority, 1);
    }

    #[test]
    fn test_create_test_framework() {
        let framework = create_test_framework(
            "Django",
            DetectionType::Dependencies {
                dependencies: vec!["django".to_string()],
            },
        );

        assert_eq!(framework.name, "Django");
        assert_eq!(framework.priority, 1);
    }
}
