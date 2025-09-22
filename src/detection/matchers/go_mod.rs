//! go.mod dependency scanning for Go frameworks

use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use anyhow::Context;
use std::fs;
use std::path::Path;

/// Go.mod parser for module and dependency scanning
pub struct GoMod {
    pub module: String,
    pub go_version: Option<String>,
    pub requires: Vec<GoRequire>,
}

#[derive(Debug, Clone)]
pub struct GoRequire {
    pub module: String,
    pub version: String,
    pub indirect: bool,
}

impl GoMod {
    /// Load go.mod from a directory
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Option<Self>> {
        let go_mod_path = dir_path.as_ref().join("go.mod");

        if !go_mod_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&go_mod_path)
            .with_context(|| format!("Failed to read {}", go_mod_path.display()))?;

        Self::parse_content(&content)
            .with_context(|| format!("Failed to parse {}", go_mod_path.display()))
            .map(Some)
    }

    /// Parse go.mod content
    fn parse_content(content: &str) -> Result<Self> {
        let mut module = String::new();
        let mut go_version = None;
        let mut requires = Vec::new();

        let mut in_require_block = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with("//") || line.is_empty() {
                continue;
            }

            if line.starts_with("module ") {
                module = line
                    .strip_prefix("module ")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            } else if line.starts_with("go ") {
                go_version = Some(line.strip_prefix("go ").unwrap_or("").trim().to_string());
            } else if line.starts_with("require (") {
                in_require_block = true;
            } else if line == ")" && in_require_block {
                in_require_block = false;
            } else if line.starts_with("require ") && !in_require_block {
                // Single require statement
                let req_line = line.strip_prefix("require ").unwrap_or("");
                if let Some(req) = Self::parse_require_line(req_line) {
                    requires.push(req);
                }
            } else if in_require_block {
                // Require block entry
                if let Some(req) = Self::parse_require_line(line) {
                    requires.push(req);
                }
            }
        }

        if module.is_empty() {
            anyhow::bail!("No module declaration found in go.mod");
        }

        Ok(GoMod {
            module,
            go_version,
            requires,
        })
    }

    /// Parse a single require line
    fn parse_require_line(line: &str) -> Option<GoRequire> {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() >= 2 {
            let module = parts[0].to_string();
            let version = parts[1].to_string();
            let indirect = parts.get(2) == Some(&"//") && parts.get(3) == Some(&"indirect");

            Some(GoRequire {
                module,
                version,
                indirect,
            })
        } else {
            None
        }
    }

    /// Check if a module exists in requires
    pub fn has_module(&self, module_name: &str) -> bool {
        self.requires.iter().any(|req| req.module == module_name)
    }

    /// Get all required module names
    pub fn get_all_modules(&self) -> Vec<String> {
        self.requires.iter().map(|req| req.module.clone()).collect()
    }

    /// Check if multiple modules exist
    pub fn has_modules(&self, module_names: &[String]) -> Vec<String> {
        module_names
            .iter()
            .filter(|module| self.has_module(module))
            .cloned()
            .collect()
    }
}

/// Go.mod framework matcher
pub struct GoModMatcher;

impl GoModMatcher {
    /// Detect frameworks in a go.mod file
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let go_mod = match GoMod::load_from_dir(&path)? {
            Some(mod_file) => mod_file,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::GoMod { modules } = &framework.detection {
                let found_modules = go_mod.has_modules(modules);

                if !found_modules.is_empty() {
                    let confidence = Self::calculate_confidence(modules, &found_modules);
                    let evidence = vec!["go.mod".to_string()];

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

    /// Calculate confidence based on module matches
    fn calculate_confidence(required_modules: &[String], found_modules: &[String]) -> f32 {
        if required_modules.is_empty() {
            return 0.0;
        }

        let match_ratio = found_modules.len() as f32 / required_modules.len() as f32;

        // Base confidence from match ratio
        let base_confidence = match_ratio * 0.9;

        // Bonus for having all modules
        let completeness_bonus = if found_modules.len() == required_modules.len() {
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

    fn create_go_mod(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        fs::write(go_mod_path, content).unwrap();
        temp_dir
    }

    fn create_test_framework(name: &str, modules: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::GoMod {
                modules: modules.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_go_mod_loading() {
        let content = r#"module example.com/myapp

go 1.19

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/gorilla/mux v1.8.0
    github.com/stretchr/testify v1.8.4 // indirect
)
"#;

        let temp_dir = create_go_mod(content);
        let go_mod = GoMod::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert_eq!(go_mod.module, "example.com/myapp");
        assert_eq!(go_mod.go_version, Some("1.19".to_string()));
        assert!(go_mod.has_module("github.com/gin-gonic/gin"));
        assert!(go_mod.has_module("github.com/gorilla/mux"));
        assert!(go_mod.has_module("github.com/stretchr/testify"));
        assert!(!go_mod.has_module("github.com/labstack/echo"));

        let all_modules = go_mod.get_all_modules();
        assert!(all_modules.contains(&"github.com/gin-gonic/gin".to_string()));
        assert!(all_modules.contains(&"github.com/gorilla/mux".to_string()));
    }

    #[test]
    fn test_single_require_statement() {
        let content = r#"module example.com/myapp

require github.com/gin-gonic/gin v1.9.1
"#;

        let temp_dir = create_go_mod(content);
        let go_mod = GoMod::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert!(go_mod.has_module("github.com/gin-gonic/gin"));
        assert_eq!(go_mod.requires.len(), 1);
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"module gin-app

require github.com/gin-gonic/gin v1.9.1
"#;

        let temp_dir = create_go_mod(content);
        let frameworks = vec![
            create_test_framework("Gin", vec!["github.com/gin-gonic/gin"]),
            create_test_framework("Echo", vec!["github.com/labstack/echo"]),
        ];

        let matches = GoModMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Gin");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"go.mod".to_string()));
    }

    #[test]
    fn test_no_go_mod() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework(
            "Gin",
            vec!["github.com/gin-gonic/gin"],
        )];

        let matches = GoModMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"module multi-framework-app

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/gorilla/mux v1.8.0
)
"#;

        let temp_dir = create_go_mod(content);
        let frameworks = vec![
            create_test_framework("Gin", vec!["github.com/gin-gonic/gin"]),
            create_test_framework("Gorilla Mux", vec!["github.com/gorilla/mux"]),
            create_test_framework("Echo", vec!["github.com/labstack/echo"]),
        ];

        let matches = GoModMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"Gin".to_string()));
        assert!(framework_names.contains(&&"Gorilla Mux".to_string()));
        assert!(!framework_names.contains(&&"Echo".to_string()));
    }

    #[test]
    fn test_confidence_calculation() {
        // Perfect match
        assert_eq!(
            GoModMatcher::calculate_confidence(
                &["github.com/gin-gonic/gin".to_string()],
                &["github.com/gin-gonic/gin".to_string()]
            ),
            1.0
        );

        // Partial match
        let confidence = GoModMatcher::calculate_confidence(
            &[
                "github.com/gin-gonic/gin".to_string(),
                "github.com/other/lib".to_string(),
            ],
            &["github.com/gin-gonic/gin".to_string()],
        );
        assert!(confidence > 0.0 && confidence < 1.0);

        // No match
        assert_eq!(
            GoModMatcher::calculate_confidence(&["github.com/gin-gonic/gin".to_string()], &[]),
            0.0
        );
    }

    #[test]
    fn test_comments_and_indirect() {
        let content = r#"module example.com/myapp

// This is a comment
go 1.19

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/stretchr/testify v1.8.4 // indirect
    // Another comment
    github.com/direct/dep v2.0.0
)
"#;

        let temp_dir = create_go_mod(content);
        let go_mod = GoMod::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert!(go_mod.has_module("github.com/gin-gonic/gin"));
        assert!(go_mod.has_module("github.com/stretchr/testify"));
        assert!(go_mod.has_module("github.com/direct/dep"));

        // Check indirect flag
        let testify_req = go_mod
            .requires
            .iter()
            .find(|req| req.module == "github.com/stretchr/testify")
            .unwrap();
        assert!(testify_req.indirect);

        let gin_req = go_mod
            .requires
            .iter()
            .find(|req| req.module == "github.com/gin-gonic/gin")
            .unwrap();
        assert!(!gin_req.indirect);
    }

    #[test]
    fn test_invalid_go_mod() {
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        fs::write(go_mod_path, "invalid go.mod content without module").unwrap();

        let result = GoMod::load_from_dir(temp_dir.path());
        assert!(result.is_err());
    }
}
