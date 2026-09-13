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
        root_indicators: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    DetectionResult::new(Some(Arc::new(language)), vec![framework_match], 0.9)
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
    fn test_create_test_rust_project() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_rust_project()?;
        let root = temp_dir.path();

        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("main.rs").exists());
        assert!(root.join("src").join("lib.rs").exists());
        Ok(())
    }
}
