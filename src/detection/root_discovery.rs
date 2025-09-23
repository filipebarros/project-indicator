//! Project root discovery functionality
//!
//! This module handles finding project roots by traversing upward from a given path
//! and calculating confidence scores based on configured indicators.

use crate::types::{DetectionConfig, RootIndicator};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Root discovery cache and logic
pub struct RootDiscovery {
    /// Detection configuration for root discovery
    detection_config: DetectionConfig,
    /// Cache for project root discovery to avoid repeated filesystem traversal
    root_cache: Mutex<HashMap<PathBuf, Option<PathBuf>>>,
}

impl RootDiscovery {
    /// Create a new root discovery instance
    pub fn new(detection_config: DetectionConfig) -> Self {
        Self {
            detection_config,
            root_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Find the project root by traversing upward from the given path
    pub fn find_project_root(&self, current_path: &Path) -> Option<PathBuf> {
        // Fast-path: check cache with raw path to avoid canonicalization when possible
        {
            let cache = self
                .root_cache
                .lock()
                .map_err(|_| "Root cache mutex poisoned")
                .ok()?;
            if let Some(cached_result) = cache.get(current_path) {
                return cached_result.clone();
            }
        }

        let canonical_path = match current_path.canonicalize() {
            Ok(p) => p,
            Err(_) => return None,
        };

        // Check cache with canonical path
        if let Some(cached_result) = {
            let cache = self
                .root_cache
                .lock()
                .map_err(|_| "Root cache mutex poisoned")
                .ok()?;
            cache.get(&canonical_path).cloned()
        } {
            // Populate raw-path entry as well for future quick hits
            if let Ok(mut cache) = self.root_cache.lock() {
                cache.insert(current_path.to_path_buf(), cached_result.clone());
            }
            return cached_result;
        }

        // Find root by traversing upward
        let root = self.find_project_root_uncached(&canonical_path);

        // Cache the result for both canonical and raw inputs
        if let Ok(mut cache) = self.root_cache.lock() {
            cache.insert(canonical_path, root.clone());
            cache.insert(current_path.to_path_buf(), root.clone());
        }

        root
    }

    /// Find project root without caching (internal implementation)
    fn find_project_root_uncached(&self, current_path: &Path) -> Option<PathBuf> {
        let mut path = current_path;
        let mut best_candidate: Option<(PathBuf, f32)> = None;

        // Use configured maximum traversal depth
        for _ in 0..self.detection_config.max_upward_traversal {
            let confidence = self.calculate_root_confidence(path);

            if confidence > 0.0 {
                // Update best candidate if this path has higher confidence
                if let Some((_, best_confidence)) = &best_candidate {
                    if confidence > *best_confidence {
                        best_candidate = Some((path.to_path_buf(), confidence));
                    }
                } else {
                    best_candidate = Some((path.to_path_buf(), confidence));
                }

                // If we find a very strong indicator, stop searching
                if confidence >= 1.0 {
                    break;
                }
            }

            // Move up one directory
            match path.parent() {
                Some(parent) => path = parent,
                None => break, // Reached filesystem root
            }
        }

        // Return the best candidate if confidence is above configured threshold
        best_candidate
            .filter(|(_, confidence)| *confidence >= self.detection_config.confidence_threshold)
            .map(|(path, _)| path)
    }

    /// Calculate confidence that a given path is a project root
    pub fn calculate_root_confidence(&self, path: &Path) -> f32 {
        let mut confidence = 0.0;

        // Only use root indicators if they are explicitly defined in configuration
        if !self.detection_config.root_indicators.is_empty() {
            for indicator in &self.detection_config.root_indicators {
                let indicator_path = path.join(&indicator.pattern);
                if indicator_path.exists() {
                    // Special handling for package.json - check if it has a name field
                    if indicator.pattern == "package.json" {
                        if self.has_package_json_with_name(path) {
                            confidence += indicator.weight;
                        } else {
                            confidence += indicator.weight * 0.5; // Lower weight for package.json without name
                        }
                    } else {
                        confidence += indicator.weight;
                    }
                }
            }
        }
        // If no root indicators are defined, confidence remains 0.0 (no root discovery)

        confidence.min(1.0)
    }

    /// Check if package.json exists and has a "name" field (indicates real project vs fixture)
    fn has_package_json_with_name(&self, path: &Path) -> bool {
        let package_json_path = path.join("package.json");
        if !package_json_path.exists() {
            return false;
        }

        // Try to read and parse package.json quickly
        if let Ok(content) = std::fs::read_to_string(&package_json_path) {
            // Simple string search for "name" field - faster than full JSON parsing
            content.contains("\"name\"") && content.len() > 20 // Reasonable size check
        } else {
            false
        }
    }

    /// Get the root indicators from configuration
    pub fn root_indicators(&self) -> &[RootIndicator] {
        &self.detection_config.root_indicators
    }

    /// Check if root discovery is enabled (has indicators)
    pub fn is_enabled(&self) -> bool {
        !self.detection_config.root_indicators.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RootIndicator;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_discovery() -> RootDiscovery {
        let detection_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![
                RootIndicator {
                    pattern: ".git".to_string(),
                    weight: 1.0,
                },
                RootIndicator {
                    pattern: "Cargo.toml".to_string(),
                    weight: 0.9,
                },
                RootIndicator {
                    pattern: "package.json".to_string(),
                    weight: 0.9,
                },
            ],
        };
        RootDiscovery::new(detection_config)
    }

    #[test]
    fn test_root_confidence_calculation() {
        let discovery = create_test_discovery();

        // Create project with multiple indicators
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
        fs::write(temp_dir.path().join(".git/config"), "").unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        fs::write(temp_dir.path().join("README.md"), "# Test Project").unwrap();

        let confidence = discovery.calculate_root_confidence(temp_dir.path());
        assert!(confidence >= 1.0); // Should have high confidence due to multiple indicators (capped at 1.0)
    }

    #[test]
    fn test_package_json_name_detection() {
        let discovery = create_test_discovery();

        // Create package.json with name field
        let temp_project = TempDir::new().unwrap();
        let package_content = r#"{"name": "test-project", "version": "1.0.0"}"#;
        fs::write(temp_project.path().join("package.json"), package_content).unwrap();

        assert!(discovery.has_package_json_with_name(temp_project.path()));

        // Create package.json without name field
        let temp_fixture = TempDir::new().unwrap();
        let fixture_content = r#"{"version": "1.0.0"}"#;
        fs::write(temp_fixture.path().join("package.json"), fixture_content).unwrap();

        assert!(!discovery.has_package_json_with_name(temp_fixture.path()));
    }

    #[test]
    fn test_nested_project_discovery() {
        let discovery = create_test_discovery();

        // Create nested project structure
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let deep_path = project_root.join("src").join("components");
        fs::create_dir_all(&deep_path).unwrap();

        // Place root indicator at project root
        fs::write(
            project_root.join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        // Test discovery from deep path
        let found_root = discovery.find_project_root(&deep_path);
        assert!(found_root.is_some());

        let canonical_found = found_root.unwrap().canonicalize().unwrap();
        let canonical_expected = project_root.canonicalize().unwrap();
        assert_eq!(canonical_found, canonical_expected);
    }

    #[test]
    fn test_multiple_indicator_confidence() {
        let discovery = create_test_discovery();

        // Create project with multiple indicators
        let temp_dir = TempDir::new().unwrap();

        // Test single indicator
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        let single_confidence = discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(single_confidence, 0.9);

        // Add .git directory for higher confidence
        fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
        fs::write(temp_dir.path().join(".git/config"), "").unwrap();
        let multi_confidence = discovery.calculate_root_confidence(temp_dir.path());
        assert!(multi_confidence >= 1.0); // Should be capped at 1.0
    }

    #[test]
    fn test_confidence_threshold_behavior() {
        // Test with low threshold - should accept moderate confidence
        let low_threshold_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![RootIndicator {
                pattern: "moderate.toml".to_string(),
                weight: 0.5,
            }],
        };
        let low_discovery = RootDiscovery::new(low_threshold_config);

        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("moderate.toml"), "test").unwrap();

        let confidence = low_discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(confidence, 0.5);

        let found_root = low_discovery.find_project_root(temp_dir.path());
        assert!(found_root.is_some()); // 0.5 > 0.3

        // Test with high threshold - should reject moderate confidence
        let high_threshold_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.8,
            root_indicators: vec![RootIndicator {
                pattern: "moderate.toml".to_string(),
                weight: 0.5,
            }],
        };
        let high_discovery = RootDiscovery::new(high_threshold_config);

        let found_root = high_discovery.find_project_root(temp_dir.path());
        assert!(found_root.is_none()); // 0.5 < 0.8
    }

    #[test]
    fn test_max_traversal_limit() {
        let detection_config = DetectionConfig {
            max_upward_traversal: 2, // Very limited
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![RootIndicator {
                pattern: "project.toml".to_string(),
                weight: 1.0,
            }],
        };
        let discovery = RootDiscovery::new(detection_config);

        // Create deep nested structure
        let temp_dir = TempDir::new().unwrap();
        let deep_path = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("level3")
            .join("level4");
        fs::create_dir_all(&deep_path).unwrap();

        // Place indicator at root (too far up)
        fs::write(temp_dir.path().join("project.toml"), "test").unwrap();

        // Should not find it due to traversal limit (4 levels up is too far)
        let found_root = discovery.find_project_root(&deep_path);
        assert!(found_root.is_none());

        // Place indicator within traversal limit (only 1 level up)
        let close_path = temp_dir.path().join("level1");
        fs::write(close_path.join("project.toml"), "test").unwrap();

        // Test from subdirectory (only 1 level down from indicator)
        let subdir = close_path.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        let found_root = discovery.find_project_root(&subdir);
        assert!(found_root.is_some());
    }

    #[test]
    fn test_package_json_special_handling() {
        let discovery = create_test_discovery();

        let temp_dir = TempDir::new().unwrap();

        // Test package.json with name (full weight)
        let package_with_name = r#"{"name": "real-project", "version": "1.0.0"}"#;
        fs::write(temp_dir.path().join("package.json"), package_with_name).unwrap();
        let confidence_with_name = discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(confidence_with_name, 0.9);

        // Test package.json without name (half weight)
        let package_without_name = r#"{"version": "1.0.0", "scripts": {"test": "echo test"}}"#;
        fs::write(temp_dir.path().join("package.json"), package_without_name).unwrap();
        let confidence_without_name = discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(confidence_without_name, 0.45); // 0.9 * 0.5

        // Test very small package.json (should be ignored due to size/content check)
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();
        let confidence_tiny = discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(confidence_tiny, 0.45); // Still gets half weight since it exists, just no name
    }

    #[test]
    fn test_caching_behavior() {
        let discovery = create_test_discovery();

        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        // First call should compute and cache
        let first_result = discovery.find_project_root(temp_dir.path());
        assert!(first_result.is_some());

        // Second call should use cache (verify by checking it still works after removing file)
        fs::remove_file(temp_dir.path().join("Cargo.toml")).unwrap();
        let cached_result = discovery.find_project_root(temp_dir.path());
        assert!(cached_result.is_some()); // Should still work due to cache

        // Different path should not benefit from cache
        let other_temp = TempDir::new().unwrap();
        let other_result = discovery.find_project_root(other_temp.path());
        assert!(other_result.is_none()); // No cache hit
    }

    #[test]
    fn test_disabled_root_discovery() {
        let detection_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![], // No indicators = disabled
        };
        let discovery = RootDiscovery::new(detection_config);

        assert!(!discovery.is_enabled());

        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

        // Even with valid indicators present, should not find root when disabled
        let found_root = discovery.find_project_root(temp_dir.path());
        assert!(found_root.is_none());

        let confidence = discovery.calculate_root_confidence(temp_dir.path());
        assert_eq!(confidence, 0.0);
    }
}
