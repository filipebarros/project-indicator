//! Common test helpers for integration tests.
//!
//! This module provides shared utilities for creating test projects, configurations,
//! and other test fixtures used across multiple integration test files.

use project_indicator::{
    config::Config,
    types::{DetectionType, FrameworkDetector, ProjectIndicator},
};
use std::fs;
use tempfile::TempDir;

/// Creates a test project with the specified files and content.
///
/// # Arguments
///
/// * `files` - A slice of tuples containing (file_path, content) pairs
///
/// # Returns
///
/// A `TempDir` containing the created files. The directory will be automatically
/// cleaned up when the `TempDir` is dropped.
///
/// # Example
///
/// ```no_run
/// # use tests::common::create_test_project;
/// let temp_dir = create_test_project(&[
///     ("package.json", r#"{"name": "test"}"#),
///     ("src/main.ts", "console.log('hello');"),
/// ])?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn create_test_project(files: &[(&str, &str)]) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;

    for (file_path, content) in files {
        let full_path = temp_dir.path().join(file_path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(full_path, content)?;
    }

    Ok(temp_dir)
}

/// Creates a test configuration with TypeScript, Rust, and Python language definitions.
///
/// This configuration includes common frameworks for each language:
/// - TypeScript: React (priority 1), Next.js (priority 0)
/// - Rust: Rocket
/// - Python: Django
///
/// # Returns
///
/// A `Config` with pre-configured language and framework detectors suitable for testing.
pub fn create_test_config() -> Config {
    let typescript = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec!["tsconfig.json".to_string(), "*.ts".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![
            FrameworkDetector {
                name: "React".to_string(),
                detection: DetectionType::NodeEcosystem {
                    dependencies: vec!["react".to_string()],
                },
                icon: Some("⚛️".to_string()),
                color: Some("#61DAFB".to_string()),
                priority: 1,
                files: vec![],
                root_indicators: vec![],
            },
            FrameworkDetector {
                name: "Next.js".to_string(),
                detection: DetectionType::NodeEcosystem {
                    dependencies: vec!["next".to_string()],
                },
                icon: Some("▲".to_string()),
                color: Some("#000000".to_string()),
                priority: 0,
                files: vec![],
                root_indicators: vec![],
            },
        ],
    );

    let rust = ProjectIndicator::new(
        "Rust".to_string(),
        vec!["Cargo.toml".to_string(), "*.rs".to_string()],
        "#DEA584".to_string(),
        "".to_string(),
        1,
        vec![FrameworkDetector {
            name: "Rocket".to_string(),
            detection: DetectionType::RustEcosystem {
                dependencies: vec!["rocket".to_string()],
            },
            icon: Some("🚀".to_string()),
            color: Some("#D33847".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    let python = ProjectIndicator::new(
        "Python".to_string(),
        vec!["*.py".to_string(), "requirements.txt".to_string()],
        "#3776AB".to_string(),
        "".to_string(),
        2,
        vec![FrameworkDetector {
            name: "Django".to_string(),
            detection: DetectionType::PythonEcosystem {
                dependencies: vec!["django".to_string()],
            },
            icon: Some("📝".to_string()),
            color: Some("#1E293B".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }],
    );

    Config::new(vec![typescript, rust, python])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_project() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            create_test_project(&[("file1.txt", "content1"), ("dir/file2.txt", "content2")])?;

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("dir/file2.txt");

        assert!(file1.exists());
        assert!(file2.exists());
        assert_eq!(fs::read_to_string(file1)?, "content1");
        assert_eq!(fs::read_to_string(file2)?, "content2");
        Ok(())
    }

    #[test]
    fn test_create_test_config() {
        let config = create_test_config();

        assert_eq!(config.languages.len(), 3);
        assert_eq!(config.languages[0].name, "TypeScript");
        assert_eq!(config.languages[1].name, "Rust");
        assert_eq!(config.languages[2].name, "Python");

        // Verify TypeScript frameworks
        assert_eq!(config.languages[0].frameworks.len(), 2);
        assert_eq!(config.languages[0].frameworks[0].name, "React");
        assert_eq!(config.languages[0].frameworks[1].name, "Next.js");

        // Verify Rust frameworks
        assert_eq!(config.languages[1].frameworks.len(), 1);
        assert_eq!(config.languages[1].frameworks[0].name, "Rocket");

        // Verify Python frameworks
        assert_eq!(config.languages[2].frameworks.len(), 1);
        assert_eq!(config.languages[2].frameworks[0].name, "Django");
    }
}
