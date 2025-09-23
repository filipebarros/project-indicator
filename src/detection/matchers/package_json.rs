//! Package.json dependency scanning for JavaScript/TypeScript frameworks

use super::common::{calculate_dependency_confidence, sort_framework_matches};
use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Package.json structure for dependency scanning
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageJson {
    pub name: Option<String>,
    pub version: Option<String>,
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "peerDependencies")]
    pub peer_dependencies: Option<HashMap<String, String>>,
}

impl PackageJson {
    /// Load package.json from a directory
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Option<Self>> {
        let package_json_path = dir_path.as_ref().join("package.json");

        if !package_json_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&package_json_path)
            .with_context(|| format!("Failed to read {}", package_json_path.display()))?;

        let package_json: PackageJson = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", package_json_path.display()))?;

        Ok(Some(package_json))
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
                .peer_dependencies
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
        if let Some(peer_dependencies) = &self.peer_dependencies {
            deps.extend(peer_dependencies.keys().cloned());
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

/// Package.json framework matcher
pub struct PackageJsonMatcher;

impl PackageJsonMatcher {
    /// Detect frameworks in a package.json file
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let package_json = match PackageJson::load_from_dir(&path)? {
            Some(pkg) => pkg,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::PackageJson { dependencies } = &framework.detection {
                let found_deps = package_json.has_dependencies(dependencies);

                if !found_deps.is_empty() {
                    let confidence = calculate_dependency_confidence(dependencies, &found_deps);
                    let evidence = vec!["package.json".to_string()];

                    matches.push(FrameworkMatch::new(framework.clone(), confidence, evidence));
                }
            }
        }

        // Sort by confidence (highest first), then by priority
        sort_framework_matches(&mut matches);

        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Use shared test helpers
    use crate::detection::matchers::test_helpers::helpers::{
        create_package_json, create_package_json_framework as create_test_framework,
    };

    #[test]
    fn test_package_json_loading() {
        let content = r#"{
            "name": "test-project",
            "version": "1.0.0",
            "dependencies": {
                "react": "^18.0.0",
                "lodash": "^4.17.21"
            },
            "devDependencies": {
                "typescript": "^4.9.0"
            }
        }"#;

        let temp_dir = create_package_json(content);
        let package_json = PackageJson::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert_eq!(package_json.name, Some("test-project".to_string()));
        assert!(package_json.has_dependency("react"));
        assert!(package_json.has_dependency("typescript"));
        assert!(!package_json.has_dependency("vue"));

        let all_deps = package_json.get_all_dependencies();
        assert!(all_deps.contains(&"react".to_string()));
        assert!(all_deps.contains(&"lodash".to_string()));
        assert!(all_deps.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"{
            "name": "react-app",
            "dependencies": {
                "react": "^18.0.0",
                "react-dom": "^18.0.0"
            }
        }"#;

        let temp_dir = create_package_json(content);
        let frameworks = vec![
            create_test_framework("React", vec!["react"]),
            create_test_framework("Vue", vec!["vue"]),
        ];

        let matches = PackageJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"package.json".to_string()));
    }

    #[test]
    fn test_no_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework("React", vec!["react"])];

        let matches = PackageJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"{
            "dependencies": {
                "react": "^18.0.0",
                "next": "^13.0.0",
                "typescript": "^4.9.0"
            }
        }"#;

        let temp_dir = create_package_json(content);
        let frameworks = vec![
            create_test_framework("React", vec!["react"]),
            create_test_framework("Next.js", vec!["next"]),
            create_test_framework("Vue", vec!["vue"]),
        ];

        let matches = PackageJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"React".to_string()));
        assert!(framework_names.contains(&&"Next.js".to_string()));
        assert!(!framework_names.contains(&&"Vue".to_string()));
    }

    #[test]
    fn test_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let package_path = temp_dir.path().join("package.json");
        fs::write(package_path, "invalid json {").unwrap();

        let result = PackageJson::load_from_dir(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_peer_dependencies() {
        let content = r#"{
            "peerDependencies": {
                "react": ">=16.0.0"
            }
        }"#;

        let temp_dir = create_package_json(content);
        let package_json = PackageJson::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(package_json.has_dependency("react"));
        let all_deps = package_json.get_all_dependencies();
        assert!(all_deps.contains(&"react".to_string()));
    }
}
