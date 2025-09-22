//! Cargo.toml dependency scanning for Rust frameworks

use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Cargo.toml structure for dependency scanning
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CargoToml {
    pub package: Option<CargoPackage>,
    pub dependencies: Option<HashMap<String, CargoDepValue>>,
    #[serde(rename = "dev-dependencies")]
    pub dev_dependencies: Option<HashMap<String, CargoDepValue>>,
    #[serde(rename = "build-dependencies")]
    pub build_dependencies: Option<HashMap<String, CargoDepValue>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CargoPackage {
    pub name: String,
    pub version: String,
    pub edition: Option<String>,
}

/// Cargo dependency can be a string or detailed object
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CargoDepValue {
    Simple(String),
    Detailed(CargoDepDetails),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CargoDepDetails {
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub features: Option<Vec<String>>,
    pub optional: Option<bool>,
}

impl CargoToml {
    /// Load Cargo.toml from a directory
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Option<Self>> {
        let cargo_path = dir_path.as_ref().join("Cargo.toml");

        if !cargo_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&cargo_path)
            .with_context(|| format!("Failed to read {}", cargo_path.display()))?;

        let cargo_toml: CargoToml = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", cargo_path.display()))?;

        Ok(Some(cargo_toml))
    }

    /// Check if a dependency exists in any dependency section
    pub fn has_dependency(&self, dep_name: &str) -> bool {
        self.dependencies
            .as_ref()
            .is_some_and(|deps| deps.contains_key(dep_name))
            || self
                .dev_dependencies
                .as_ref()
                .is_some_and(|deps| deps.contains_key(dep_name))
            || self
                .build_dependencies
                .as_ref()
                .is_some_and(|deps| deps.contains_key(dep_name))
    }

    /// Get all dependency names
    pub fn get_all_dependencies(&self) -> Vec<String> {
        let mut deps = Vec::new();

        if let Some(dependencies) = &self.dependencies {
            deps.extend(dependencies.keys().cloned());
        }
        if let Some(dev_dependencies) = &self.dev_dependencies {
            deps.extend(dev_dependencies.keys().cloned());
        }
        if let Some(build_dependencies) = &self.build_dependencies {
            deps.extend(build_dependencies.keys().cloned());
        }

        deps.sort();
        deps.dedup();
        deps
    }

    /// Check if multiple dependencies exist
    pub fn has_dependencies(&self, dep_names: &[String]) -> Vec<String> {
        dep_names
            .iter()
            .filter(|dep| self.has_dependency(dep))
            .cloned()
            .collect()
    }
}

/// Cargo.toml framework matcher
pub struct CargoTomlMatcher;

impl CargoTomlMatcher {
    /// Detect frameworks in a Cargo.toml file
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let cargo_toml = match CargoToml::load_from_dir(&path)? {
            Some(cargo) => cargo,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::CargoToml { dependencies } = &framework.detection {
                let found_deps = cargo_toml.has_dependencies(dependencies);

                if !found_deps.is_empty() {
                    let confidence = Self::calculate_confidence(dependencies, &found_deps);
                    let evidence = vec!["Cargo.toml".to_string()];

                    matches.push(FrameworkMatch::new(framework.clone(), confidence, evidence));
                }
            }
        }

        // Sort by confidence (highest first), then by priority
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.framework.priority.cmp(&b.framework.priority))
        });

        Ok(matches)
    }

    /// Calculate confidence based on dependency matches
    fn calculate_confidence(required_deps: &[String], found_deps: &[String]) -> f32 {
        if required_deps.is_empty() {
            return 0.0;
        }

        let match_ratio = found_deps.len() as f32 / required_deps.len() as f32;

        // Base confidence from match ratio
        let base_confidence = match_ratio * 0.9;

        // Bonus for having all dependencies
        let completeness_bonus = if found_deps.len() == required_deps.len() {
            0.1
        } else {
            0.0
        };

        (base_confidence + completeness_bonus).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_cargo_toml(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let cargo_path = temp_dir.path().join("Cargo.toml");
        fs::write(cargo_path, content).unwrap();
        temp_dir
    }

    fn create_test_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::CargoToml {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_cargo_toml_loading() {
        let content = r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }

[dev-dependencies]
criterion = "0.5"
"#;

        let temp_dir = create_cargo_toml(content);
        let cargo_toml = CargoToml::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert_eq!(cargo_toml.package.as_ref().unwrap().name, "test-project");
        assert!(cargo_toml.has_dependency("serde"));
        assert!(cargo_toml.has_dependency("tokio"));
        assert!(cargo_toml.has_dependency("criterion"));
        assert!(!cargo_toml.has_dependency("rocket"));

        let all_deps = cargo_toml.get_all_dependencies();
        assert!(all_deps.contains(&"serde".to_string()));
        assert!(all_deps.contains(&"tokio".to_string()));
        assert!(all_deps.contains(&"criterion".to_string()));
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"
[package]
name = "rocket-app"
version = "0.1.0"

[dependencies]
rocket = "0.5"
serde = "1.0"
"#;

        let temp_dir = create_cargo_toml(content);
        let frameworks = vec![
            create_test_framework("Rocket", vec!["rocket"]),
            create_test_framework("Actix", vec!["actix-web"]),
        ];

        let matches = CargoTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Rocket");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn test_no_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework("Rocket", vec!["rocket"])];

        let matches = CargoTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"
[dependencies]
rocket = "0.5"
tokio = "1.0"
serde = "1.0"
"#;

        let temp_dir = create_cargo_toml(content);
        let frameworks = vec![
            create_test_framework("Rocket", vec!["rocket"]),
            create_test_framework("Tokio", vec!["tokio"]),
            create_test_framework("Actix", vec!["actix-web"]),
        ];

        let matches = CargoTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"Rocket".to_string()));
        assert!(framework_names.contains(&&"Tokio".to_string()));
        assert!(!framework_names.contains(&&"Actix".to_string()));
    }

    #[test]
    fn test_confidence_calculation() {
        // Perfect match
        assert_eq!(
            CargoTomlMatcher::calculate_confidence(
                &["rocket".to_string()],
                &["rocket".to_string()]
            ),
            1.0
        );

        // Partial match
        let confidence = CargoTomlMatcher::calculate_confidence(
            &["rocket".to_string(), "serde".to_string()],
            &["rocket".to_string()],
        );
        assert!(confidence > 0.0 && confidence < 1.0);

        // No match
        assert_eq!(
            CargoTomlMatcher::calculate_confidence(&["rocket".to_string()], &[]),
            0.0
        );
    }

    #[test]
    fn test_dependency_value_types() {
        let content = r#"
[dependencies]
simple = "1.0"
detailed = { version = "1.0", features = ["full"] }
git_dep = { git = "https://github.com/user/repo" }
"#;

        let temp_dir = create_cargo_toml(content);
        let cargo_toml = CargoToml::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert!(cargo_toml.has_dependency("simple"));
        assert!(cargo_toml.has_dependency("detailed"));
        assert!(cargo_toml.has_dependency("git_dep"));
    }

    #[test]
    fn test_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_path = temp_dir.path().join("Cargo.toml");
        fs::write(cargo_path, "invalid toml [[[").unwrap();

        let result = CargoToml::load_from_dir(temp_dir.path());
        assert!(result.is_err());
    }
}
