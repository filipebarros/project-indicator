//! pyproject.toml dependency scanning for Python frameworks

use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// pyproject.toml structure for dependency and tool scanning
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PyProjectToml {
    pub project: Option<PyProject>,
    pub tool: Option<HashMap<String, toml::Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PyProject {
    pub name: Option<String>,
    pub version: Option<String>,
    pub dependencies: Option<Vec<String>>,
    #[serde(rename = "optional-dependencies")]
    pub optional_dependencies: Option<HashMap<String, Vec<String>>>,
}

impl PyProjectToml {
    /// Load pyproject.toml from a directory
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Option<Self>> {
        let pyproject_path = dir_path.as_ref().join("pyproject.toml");

        if !pyproject_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&pyproject_path)
            .with_context(|| format!("Failed to read {}", pyproject_path.display()))?;

        let pyproject: PyProjectToml = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", pyproject_path.display()))?;

        Ok(Some(pyproject))
    }

    /// Check if a dependency exists in project dependencies
    pub fn has_dependency(&self, dep_name: &str) -> bool {
        if let Some(project) = &self.project {
            // Check regular dependencies
            if let Some(dependencies) = &project.dependencies {
                if dependencies.iter().any(|dep| {
                    // Handle dependencies like "django>=4.0" or "django==4.2.1"
                    dep.split(&['=', '>', '<', '!', '~', ' ', ';'][..])
                        .next()
                        .is_some_and(|name| name.trim() == dep_name)
                }) {
                    return true;
                }
            }

            // Check optional dependencies
            if let Some(optional_deps) = &project.optional_dependencies {
                for deps in optional_deps.values() {
                    if deps.iter().any(|dep| {
                        dep.split(&['=', '>', '<', '!', '~', ' ', ';'][..])
                            .next()
                            .is_some_and(|name| name.trim() == dep_name)
                    }) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if a tool is configured
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tool
            .as_ref()
            .is_some_and(|tools| tools.contains_key(tool_name))
    }

    /// Get all configured tool names
    pub fn get_all_tools(&self) -> Vec<String> {
        self.tool
            .as_ref()
            .map(|tools| tools.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all dependency names
    pub fn get_all_dependencies(&self) -> Vec<String> {
        let mut deps = Vec::new();

        if let Some(project) = &self.project {
            if let Some(dependencies) = &project.dependencies {
                for dep in dependencies {
                    if let Some(name) = dep.split(&['=', '>', '<', '!', '~', ' ', ';'][..]).next() {
                        deps.push(name.trim().to_string());
                    }
                }
            }

            if let Some(optional_deps) = &project.optional_dependencies {
                for dep_list in optional_deps.values() {
                    for dep in dep_list {
                        if let Some(name) =
                            dep.split(&['=', '>', '<', '!', '~', ' ', ';'][..]).next()
                        {
                            deps.push(name.trim().to_string());
                        }
                    }
                }
            }
        }

        deps.sort();
        deps.dedup();
        deps
    }

    /// Check if multiple tools exist
    pub fn has_tools(&self, tool_names: &[String]) -> Vec<String> {
        tool_names
            .iter()
            .filter(|tool| self.has_tool(tool))
            .cloned()
            .collect()
    }
}

/// pyproject.toml framework matcher
pub struct PyProjectTomlMatcher;

impl PyProjectTomlMatcher {
    /// Detect frameworks in a pyproject.toml file
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let pyproject = match PyProjectToml::load_from_dir(&path)? {
            Some(proj) => proj,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::PyProjectToml { dependencies } = &framework.detection {
                let found_deps: Vec<String> = dependencies
                    .iter()
                    .filter(|dep| pyproject.has_dependency(dep))
                    .cloned()
                    .collect();

                if !found_deps.is_empty() {
                    let confidence = Self::calculate_confidence(dependencies, &found_deps);
                    let evidence = vec!["pyproject.toml".to_string()];

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

    /// Calculate confidence based on tool matches
    fn calculate_confidence(required_tools: &[String], found_tools: &[String]) -> f32 {
        if required_tools.is_empty() {
            return 0.0;
        }

        let match_ratio = found_tools.len() as f32 / required_tools.len() as f32;

        // Base confidence from match ratio
        let base_confidence = match_ratio * 0.9;

        // Bonus for having all tools
        let completeness_bonus = if found_tools.len() == required_tools.len() {
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

    fn create_pyproject_toml(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");
        fs::write(pyproject_path, content).unwrap();
        temp_dir
    }

    fn create_test_framework(name: &str, dependencies: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::PyProjectToml {
                dependencies: dependencies.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_pyproject_toml_loading() {
        let content = r#"
[project]
name = "test-project"
version = "0.1.0"
dependencies = [
    "django>=4.0",
    "requests==2.28.1",
    "click~=8.0"
]

[project.optional-dependencies]
dev = ["pytest>=7.0", "black"]

[tool.poetry]
name = "test-project"

[tool.black]
line-length = 88
"#;

        let temp_dir = create_pyproject_toml(content);
        let pyproject = PyProjectToml::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert_eq!(
            pyproject.project.as_ref().unwrap().name,
            Some("test-project".to_string())
        );
        assert!(pyproject.has_dependency("django"));
        assert!(pyproject.has_dependency("requests"));
        assert!(pyproject.has_dependency("click"));
        assert!(pyproject.has_dependency("pytest"));
        assert!(pyproject.has_dependency("black"));
        assert!(!pyproject.has_dependency("flask"));

        assert!(pyproject.has_tool("poetry"));
        assert!(pyproject.has_tool("black"));
        assert!(!pyproject.has_tool("flake8"));

        let all_deps = pyproject.get_all_dependencies();
        assert!(all_deps.contains(&"django".to_string()));
        assert!(all_deps.contains(&"requests".to_string()));
        assert!(all_deps.contains(&"pytest".to_string()));
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"
[project]
name = "test-project"
dependencies = [
    "django>=4.0",
    "requests",
]
"#;

        let temp_dir = create_pyproject_toml(content);
        let frameworks = vec![
            create_test_framework("Django", vec!["django"]),
            create_test_framework("Flask", vec!["flask"]),
        ];

        let matches =
            PyProjectTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Django");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"pyproject.toml".to_string()));
    }

    #[test]
    fn test_no_pyproject_toml() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework("Poetry", vec!["poetry"])];

        let matches =
            PyProjectTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"
[project]
name = "multi-framework-project"
dependencies = [
    "django>=4.0",
    "fastapi",
    "requests",
]
"#;

        let temp_dir = create_pyproject_toml(content);
        let frameworks = vec![
            create_test_framework("Django", vec!["django"]),
            create_test_framework("FastAPI", vec!["fastapi"]),
            create_test_framework("Flask", vec!["flask"]),
        ];

        let matches =
            PyProjectTomlMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"Django".to_string()));
        assert!(framework_names.contains(&&"FastAPI".to_string()));
        assert!(!framework_names.contains(&&"Flask".to_string()));
    }

    #[test]
    fn test_confidence_calculation() {
        // Perfect match
        assert_eq!(
            PyProjectTomlMatcher::calculate_confidence(
                &["poetry".to_string()],
                &["poetry".to_string()]
            ),
            1.0
        );

        // Partial match
        let confidence = PyProjectTomlMatcher::calculate_confidence(
            &["poetry".to_string(), "black".to_string()],
            &["poetry".to_string()],
        );
        assert!(confidence > 0.0 && confidence < 1.0);

        // No match
        assert_eq!(
            PyProjectTomlMatcher::calculate_confidence(&["poetry".to_string()], &[]),
            0.0
        );
    }

    #[test]
    fn test_dependency_parsing() {
        let content = r#"
[project]
dependencies = [
    "django>=4.0,<5.0",
    "requests==2.28.1",
    "click~=8.0.0",
    "numpy; python_version >= '3.8'",
    "scipy>=1.0 # Scientific computing"
]
"#;

        let temp_dir = create_pyproject_toml(content);
        let pyproject = PyProjectToml::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(pyproject.has_dependency("django"));
        assert!(pyproject.has_dependency("requests"));
        assert!(pyproject.has_dependency("click"));
        assert!(pyproject.has_dependency("numpy"));
        assert!(pyproject.has_dependency("scipy"));
        assert!(!pyproject.has_dependency("flask"));
    }

    #[test]
    fn test_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");
        fs::write(pyproject_path, "invalid toml [[[").unwrap();

        let result = PyProjectToml::load_from_dir(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_minimal_pyproject() {
        let content = r#"
[tool.setuptools]
py-modules = ["mymodule"]
"#;

        let temp_dir = create_pyproject_toml(content);
        let pyproject = PyProjectToml::load_from_dir(temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(pyproject.has_tool("setuptools"));
        assert!(!pyproject.has_dependency("django"));
    }
}
