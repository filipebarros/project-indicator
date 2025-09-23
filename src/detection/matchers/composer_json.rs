use super::common::{calculate_dependency_confidence, sort_framework_matches};
use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Composer.json structure for PHP projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposerJson {
    pub name: Option<String>,
    pub description: Option<String>,
    pub require: Option<HashMap<String, String>>,
    #[serde(rename = "require-dev")]
    pub require_dev: Option<HashMap<String, String>>,
    pub autoload: Option<HashMap<String, serde_json::Value>>,
    pub scripts: Option<HashMap<String, serde_json::Value>>,
}

impl ComposerJson {
    /// Load composer.json from a directory
    pub fn load_from_dir(dir: &Path) -> Result<Option<Self>> {
        let composer_path = dir.join("composer.json");
        if !composer_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&composer_path)?;
        let composer: ComposerJson = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse composer.json: {}", e))?;

        Ok(Some(composer))
    }

    /// Check if a package exists in composer dependencies
    pub fn has_package(&self, package_name: &str) -> bool {
        // Check regular dependencies
        if let Some(require) = &self.require {
            if require.keys().any(|dep| {
                // Handle packages like "laravel/framework" or "symfony/console"
                dep.eq_ignore_ascii_case(package_name)
            }) {
                return true;
            }
        }

        // Check dev dependencies
        if let Some(require_dev) = &self.require_dev {
            if require_dev
                .keys()
                .any(|dep| dep.eq_ignore_ascii_case(package_name))
            {
                return true;
            }
        }

        false
    }

    /// Get all packages from composer.json
    pub fn get_all_packages(&self) -> Vec<String> {
        let mut packages = Vec::new();

        // Add regular dependencies
        if let Some(require) = &self.require {
            packages.extend(require.keys().cloned());
        }

        // Add dev dependencies
        if let Some(require_dev) = &self.require_dev {
            packages.extend(require_dev.keys().cloned());
        }

        packages
    }

    /// Get package version
    pub fn get_package_version(&self, package_name: &str) -> Option<&String> {
        if let Some(require) = &self.require {
            if let Some(version) = require.get(package_name) {
                return Some(version);
            }
        }

        if let Some(require_dev) = &self.require_dev {
            if let Some(version) = require_dev.get(package_name) {
                return Some(version);
            }
        }

        None
    }
}

/// Composer.json-based framework matcher
pub struct ComposerJsonMatcher;

impl ComposerJsonMatcher {
    /// Detect frameworks in a directory using composer.json
    pub fn detect_frameworks(
        path: &Path,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let composer = match ComposerJson::load_from_dir(path)? {
            Some(composer) => composer,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::ComposerJson { packages } = &framework.detection {
                let found_packages: Vec<String> = packages
                    .iter()
                    .filter(|package| composer.has_package(package))
                    .cloned()
                    .collect();

                if !found_packages.is_empty() {
                    let confidence = calculate_dependency_confidence(packages, &found_packages);
                    let evidence = vec!["composer.json".to_string()];

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
    use std::fs;
    use tempfile::TempDir;

    fn create_composer_json(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let composer_path = temp_dir.path().join("composer.json");
        fs::write(composer_path, content).unwrap();
        temp_dir
    }

    fn create_test_framework(name: &str, packages: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::ComposerJson {
                packages: packages.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_composer_json_loading() {
        let content = r#"{
    "name": "my-app/laravel-app",
    "description": "A Laravel application",
    "require": {
        "php": "^8.1",
        "laravel/framework": "^10.0",
        "symfony/console": "^6.0"
    },
    "require-dev": {
        "phpunit/phpunit": "^10.0",
        "mockery/mockery": "^1.4"
    }
}"#;

        let temp_dir = create_composer_json(content);
        let composer = ComposerJson::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(composer.has_package("laravel/framework"));
        assert!(composer.has_package("symfony/console"));
        assert!(composer.has_package("phpunit/phpunit"));
        assert!(!composer.has_package("codeigniter4/framework"));

        let all_packages = composer.get_all_packages();
        assert!(all_packages.contains(&"laravel/framework".to_string()));
        assert!(all_packages.contains(&"phpunit/phpunit".to_string()));

        assert_eq!(
            composer.get_package_version("laravel/framework"),
            Some(&"^10.0".to_string())
        );
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"{
    "name": "my-app/laravel-app",
    "require": {
        "php": "^8.1",
        "laravel/framework": "^10.0"
    }
}"#;

        let temp_dir = create_composer_json(content);
        let frameworks = vec![
            create_test_framework("Laravel", vec!["laravel/framework"]),
            create_test_framework("Symfony", vec!["symfony/framework-bundle"]),
        ];

        let matches = ComposerJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Laravel");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"composer.json".to_string()));
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"{
    "name": "my-app/multi-framework",
    "require": {
        "laravel/framework": "^10.0",
        "symfony/console": "^6.0",
        "guzzlehttp/guzzle": "^7.0"
    }
}"#;

        let temp_dir = create_composer_json(content);
        let frameworks = vec![
            create_test_framework("Laravel", vec!["laravel/framework"]),
            create_test_framework("Guzzle", vec!["guzzlehttp/guzzle"]),
            create_test_framework("CodeIgniter", vec!["codeigniter4/framework"]),
        ];

        let matches = ComposerJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"Laravel".to_string()));
        assert!(framework_names.contains(&&"Guzzle".to_string()));
        assert!(!framework_names.contains(&&"CodeIgniter".to_string()));
    }

    #[test]
    fn test_no_composer_json() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework("Laravel", vec!["laravel/framework"])];

        let matches = ComposerJsonMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let content = "{ invalid json }";
        let temp_dir = create_composer_json(content);

        let result = ComposerJson::load_from_dir(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_minimal_composer() {
        let content = r#"{
    "name": "test/app"
}"#;

        let temp_dir = create_composer_json(content);
        let composer = ComposerJson::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert_eq!(composer.name, Some("test/app".to_string()));
        assert!(!composer.has_package("laravel/framework"));
        assert!(composer.get_all_packages().is_empty());
    }
}
