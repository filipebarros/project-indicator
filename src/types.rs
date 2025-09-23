//! Core data types for project detection

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for a programming language and its framework detection rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectIndicator {
    /// Display name of the language (e.g., "TypeScript", "Python")
    pub name: String,
    /// File patterns that indicate this language (e.g., ["package.json", "tsconfig.json"])
    pub files: Vec<String>,
    /// Hex color code for display (e.g., "#3178C6")
    pub color: String,
    /// Nerd Font icon for display (e.g., "󰛦")
    pub icon: String,
    /// Detection priority (lower = higher priority, 1 = highest)
    pub priority: u8,
    /// Framework detection rules for this language
    #[serde(default)]
    pub frameworks: Vec<FrameworkDetector>,
}

/// Framework detection configuration within a language
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameworkDetector {
    /// Framework name (e.g., "React", "Django")
    pub name: String,
    /// How to detect this framework
    pub detection: DetectionType,
    /// Override icon for this framework (falls back to language icon)
    pub icon: Option<String>,
    /// Override color for this framework (falls back to language color)
    pub color: Option<String>,
    /// Framework priority within language (lower = higher priority)
    pub priority: u8,
    /// Additional files that must exist for detection
    #[serde(default)]
    pub files: Vec<String>,
}

/// Different ways to detect frameworks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DetectionType {
    /// Check package.json dependencies
    PackageJson { dependencies: Vec<String> },
    /// Check Cargo.toml dependencies
    CargoToml { dependencies: Vec<String> },
    /// Check go.mod module requirements
    GoMod { modules: Vec<String> },
    /// Check pyproject.toml dependencies
    PyProjectToml { dependencies: Vec<String> },
    /// Check Gemfile gems
    GemSpec { gems: Vec<String> },
    /// Check composer.json packages
    ComposerJson { packages: Vec<String> },
    /// Check if specific files exist
    FileExists { files: Vec<String> },
    /// Check configuration file for specific keys
    ConfigFile { file: String, keys: Vec<String> },
}

/// Result of project detection
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    /// The detected language (if any)
    pub language: Option<Arc<ProjectIndicator>>,
    /// Detected frameworks within the language
    pub frameworks: Vec<FrameworkMatch>,
    /// Overall confidence in the detection (0.0 - 1.0)
    pub confidence: f32,
}

/// A matched framework with evidence
#[derive(Debug, Clone, PartialEq)]
pub struct FrameworkMatch {
    /// The matched framework configuration
    pub framework: FrameworkDetector,
    /// Confidence in this match (0.0 - 1.0)
    pub confidence: f32,
    /// Files/evidence that triggered this detection
    pub evidence: Vec<String>,
}

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    /// Whether to show framework information
    pub show_frameworks: bool,
    /// Maximum number of frameworks to display
    pub max_frameworks: usize,
    /// Separator between frameworks (e.g., "+")
    pub framework_separator: String,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Maximum number of entries in cache
    pub max_entries: usize,
    /// Time-to-live for cache entries in seconds
    pub ttl_seconds: u64,
}

/// Metadata about the configuration format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigMeta {
    /// Configuration format version
    pub version: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_frameworks: true,
            max_frameworks: 2,
            framework_separator: "+".to_string(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            ttl_seconds: 300, // 5 minutes
        }
    }
}

impl Default for ConfigMeta {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
        }
    }
}

impl DetectionResult {
    /// Create a new detection result
    pub fn new(
        language: Option<Arc<ProjectIndicator>>,
        frameworks: Vec<FrameworkMatch>,
        confidence: f32,
    ) -> Self {
        Self {
            language,
            frameworks,
            confidence,
        }
    }

    /// Create an empty (no detection) result
    pub fn empty() -> Self {
        Self {
            language: None,
            frameworks: Vec::new(),
            confidence: 0.0,
        }
    }

    /// Check if any detection was made
    pub fn is_empty(&self) -> bool {
        self.language.is_none() && self.frameworks.is_empty()
    }

    /// Get the best framework match (highest priority, then highest confidence)
    pub fn best_framework(&self) -> Option<&FrameworkMatch> {
        self.frameworks.iter().min_by(|a, b| {
            a.framework.priority.cmp(&b.framework.priority).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        })
    }

    /// Get the display icon, preferring framework over language
    pub fn display_icon(&self) -> Option<&str> {
        self.best_framework()
            .and_then(|f| f.framework.icon.as_deref())
            .or_else(|| self.language.as_ref().map(|l| l.icon.as_str()))
    }

    /// Get the display color, preferring framework over language
    pub fn display_color(&self) -> Option<&str> {
        self.best_framework()
            .and_then(|f| f.framework.color.as_deref())
            .or_else(|| self.language.as_ref().map(|l| l.color.as_str()))
    }
}

impl FrameworkMatch {
    /// Create a new framework match
    pub fn new(framework: FrameworkDetector, confidence: f32, evidence: Vec<String>) -> Self {
        Self {
            framework,
            confidence,
            evidence,
        }
    }
}

impl ProjectIndicator {
    /// Check if this language matches any of the given file patterns
    pub fn matches_files(&self, files: &[String]) -> bool {
        self.files.iter().any(|pattern| {
            files.iter().any(|file| {
                // Simple pattern matching - could be enhanced with glob patterns later
                if pattern.contains('*') {
                    // Basic wildcard support
                    let prefix = pattern.split('*').next().unwrap_or("");
                    let suffix = pattern.split('*').next_back().unwrap_or("");
                    file.starts_with(prefix) && file.ends_with(suffix)
                } else {
                    file == pattern
                }
            })
        })
    }

    /// Get frameworks sorted by priority
    pub fn frameworks_by_priority(&self) -> Vec<&FrameworkDetector> {
        let mut frameworks: Vec<&FrameworkDetector> = self.frameworks.iter().collect();
        frameworks.sort_by_key(|f| f.priority);
        frameworks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_indicator_creation() {
        let indicator = ProjectIndicator {
            name: "TypeScript".to_string(),
            files: vec!["package.json".to_string(), "tsconfig.json".to_string()],
            color: "#3178C6".to_string(),
            icon: "󰛦".to_string(),
            priority: 1,
            frameworks: vec![],
        };

        assert_eq!(indicator.name, "TypeScript");
        assert_eq!(indicator.files.len(), 2);
        assert_eq!(indicator.priority, 1);
    }

    #[test]
    fn test_file_matching() {
        let indicator = ProjectIndicator {
            name: "TypeScript".to_string(),
            files: vec!["package.json".to_string(), "*.ts".to_string()],
            color: "#3178C6".to_string(),
            icon: "󰛦".to_string(),
            priority: 1,
            frameworks: vec![],
        };

        let files = vec![
            "package.json".to_string(),
            "src/main.ts".to_string(),
            "README.md".to_string(),
        ];

        assert!(indicator.matches_files(&files));

        let no_match_files = vec!["README.md".to_string(), "main.py".to_string()];
        assert!(!indicator.matches_files(&no_match_files));
    }

    #[test]
    fn test_wildcard_matching() {
        let indicator = ProjectIndicator {
            name: "C++".to_string(),
            files: vec!["*.cpp".to_string(), "*.h".to_string()],
            color: "#00599C".to_string(),
            icon: "".to_string(),
            priority: 1,
            frameworks: vec![],
        };

        let files = vec!["main.cpp".to_string(), "header.h".to_string()];
        assert!(indicator.matches_files(&files));

        let no_match = vec!["main.py".to_string()];
        assert!(!indicator.matches_files(&no_match));
    }

    #[test]
    fn test_detection_result_empty() {
        let result = DetectionResult::empty();
        assert!(result.is_empty());
        assert_eq!(result.confidence, 0.0);
        assert!(result.best_framework().is_none());
    }

    #[test]
    fn test_detection_result_with_framework() {
        let framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: Some("⚛️".to_string()),
            color: Some("#61DAFB".to_string()),
            priority: 1,
            files: vec![],
        };

        let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

        let result = DetectionResult::new(None, vec![framework_match], 0.9);

        assert!(!result.is_empty());
        assert_eq!(result.display_icon(), Some("⚛️"));
        assert_eq!(result.display_color(), Some("#61DAFB"));
        assert!(result.best_framework().is_some());
    }

    #[test]
    fn test_framework_priority_sorting() {
        let framework1 = FrameworkDetector {
            name: "Framework1".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 3,
            files: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "Framework2".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let indicator = ProjectIndicator {
            name: "Test".to_string(),
            files: vec![],
            color: "#000000".to_string(),
            icon: "".to_string(),
            priority: 1,
            frameworks: vec![framework1, framework2],
        };

        let sorted = indicator.frameworks_by_priority();
        assert_eq!(sorted[0].name, "Framework2"); // priority 1 comes first
        assert_eq!(sorted[1].name, "Framework1"); // priority 3 comes second
    }

    #[test]
    fn test_serde_serialization() {
        let detection_type = DetectionType::PackageJson {
            dependencies: vec!["react".to_string(), "typescript".to_string()],
        };

        let json = serde_json::to_string(&detection_type).unwrap();
        let deserialized: DetectionType = serde_json::from_str(&json).unwrap();

        assert_eq!(detection_type, deserialized);
    }

    #[test]
    fn test_display_config_defaults() {
        let config = DisplayConfig::default();
        assert!(config.show_frameworks);
        assert_eq!(config.max_frameworks, 2);
        assert_eq!(config.framework_separator, "+");
    }

    #[test]
    fn test_cache_config_defaults() {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_seconds, 300);
    }
}
