//! Core data types for project detection

use crate::patterns::{pattern_to_regex, simple_wildcard_match};
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

/// Root indicator for project root detection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RootIndicator {
    /// File or directory pattern to look for
    pub pattern: String,
    /// Confidence weight (0.0 - 1.0)
    pub weight: f32,
}

/// Detection configuration for project root discovery and behavior
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionConfig {
    /// Maximum number of directories to traverse upward
    pub max_upward_traversal: usize,
    /// Require version control system (.git, .hg, .svn) to be present
    pub require_vcs_root: bool,
    /// Minimum confidence threshold for considering a directory as project root
    pub confidence_threshold: f32,
    /// Custom root indicators in addition to built-in ones
    pub root_indicators: Vec<RootIndicator>,
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

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![], // No default indicators - must be explicitly defined
        }
    }
}

impl DetectionConfig {
    /// Get all root indicators from configuration
    pub fn all_root_indicators(&self) -> Vec<&RootIndicator> {
        self.root_indicators.iter().collect()
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
        // Compile wildcard patterns once for efficiency
        let mut compiled: Vec<(Option<regex::Regex>, &str)> = Vec::new();
        for pattern in &self.files {
            let regex_opt = pattern_to_regex(pattern).and_then(|s| regex::Regex::new(&s).ok());
            compiled.push((regex_opt, pattern.as_str()));
        }

        files.iter().any(|file| {
            compiled.iter().any(|(re_opt, pat)| {
                if let Some(re) = re_opt {
                    re.is_match(file)
                } else if pat.contains('*') {
                    simple_wildcard_match(file, pat)
                } else {
                    file == pat
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

// =============================================================================
// Depth-based file weighting data structures
// =============================================================================

/// Classification of directory types for weighting purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DirectoryType {
    /// Project root directory (highest weight)
    Root,
    /// Source code directories (src/, lib/, app/)
    Source,
    /// Configuration directories (.github/, config/)
    Config,
    /// Test directories (test/, spec/, __tests__, fixtures/)
    Test,
    /// Build output directories (dist/, build/, target/)
    Build,
    /// Dependency directories (node_modules/, vendor/)
    Dependencies,
    /// Documentation directories (docs/, doc/)
    Documentation,
    /// Example directories (examples/, sample/)
    Examples,
    /// Unknown directory type
    Unknown,
}

impl DirectoryType {
    /// Get the weight multiplier for this directory type
    pub fn weight(&self) -> f32 {
        match self {
            DirectoryType::Root => 1.0,
            DirectoryType::Source => 1.2,
            DirectoryType::Config => 1.1,
            DirectoryType::Test => 0.2,
            DirectoryType::Build => 0.1,
            DirectoryType::Dependencies => 0.05,
            DirectoryType::Documentation => 0.6,
            DirectoryType::Examples => 0.3,
            DirectoryType::Unknown => 0.8,
        }
    }

    /// Classify a directory path component
    pub fn classify(path_component: &str) -> Self {
        match path_component.to_lowercase().as_str() {
            // Source directories
            "src" | "lib" | "app" | "source" | "code" => DirectoryType::Source,

            // Test directories
            "test" | "tests" | "spec" | "specs" | "__tests__" | "fixtures" | "test-fixtures"
            | "__test__" | "__spec__" | "testing" => DirectoryType::Test,

            // Build directories
            "dist" | "build" | "target" | "out" | "output" | "bin" | "release" | "debug" => {
                DirectoryType::Build
            }

            // Dependencies
            "node_modules" | "vendor" | ".git" | "packages" | "deps" => DirectoryType::Dependencies,

            // Documentation
            "docs" | "doc" | "documentation" | "manual" | "guide" => DirectoryType::Documentation,

            // Examples
            "examples" | "example" | "samples" | "sample" | "demo" | "demos" => {
                DirectoryType::Examples
            }

            // Configuration
            ".github" | ".vscode" | ".idea" | "config" | "configuration" | "configs"
            | "settings" | ".config" => DirectoryType::Config,

            _ => DirectoryType::Unknown,
        }
    }
}

/// A file matched during project scanning with depth and weighting metadata
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedFile {
    /// Base filename (e.g., "package.json")
    pub filename: String,

    /// Full relative path from project root (e.g., "test/fixtures/package.json")
    pub relative_path: String,

    /// Directory depth from project root (0 = root, 1 = first level, etc.)
    pub depth: usize,

    /// Classification of the directory containing this file
    pub directory_type: DirectoryType,
}

impl MatchedFile {
    /// Create a new MatchedFile with fast constructor (no weight calculation)
    pub fn new(filename: String, relative_path: String) -> Self {
        let depth = Self::calculate_depth(&relative_path);
        let directory_type = Self::classify_directory(&relative_path);

        Self {
            filename,
            relative_path,
            depth,
            directory_type,
        }
    }

    /// Calculate weight lazily when needed
    pub fn weight(&self) -> f32 {
        Self::calculate_weight(self.depth, self.directory_type)
    }

    /// Calculate depth from relative path
    fn calculate_depth(relative_path: &str) -> usize {
        if relative_path.is_empty() {
            return 0;
        }

        // Count directory separators to determine depth
        relative_path.matches('/').count()
    }

    /// Classify directory type based on path components
    fn classify_directory(relative_path: &str) -> DirectoryType {
        if relative_path.is_empty() || !relative_path.contains('/') {
            return DirectoryType::Root;
        }

        // Get the first directory component in the path
        let first_component = relative_path.split('/').next().unwrap_or("");
        DirectoryType::classify(first_component)
    }

    /// Calculate weight based on depth and directory type
    fn calculate_weight(depth: usize, directory_type: DirectoryType) -> f32 {
        // Base depth weighting
        let depth_weight = match depth {
            0 => 1.0,  // Root files: full weight
            1 => 0.7,  // Depth 1: 70% weight
            2 => 0.4,  // Depth 2: 40% weight
            3 => 0.1,  // Depth 3: 10% weight
            _ => 0.05, // Deeper: minimal weight
        };

        // Apply directory type multiplier
        depth_weight * directory_type.weight()
    }
}

/// Helper functions for pattern importance and file classification
impl MatchedFile {
    /// Get importance weight for a specific file pattern
    pub fn get_pattern_importance(pattern: &str) -> f32 {
        match pattern.to_lowercase().as_str() {
            // Configuration files get extra weight
            "package.json" | "cargo.toml" | "pyproject.toml" | "pom.xml" | "build.gradle"
            | "composer.json" | "go.mod" => 2.0,

            // Build/config files
            "makefile" | "dockerfile" | "docker-compose.yml" | "tsconfig.json"
            | "webpack.config.js" | "vite.config.js" => 1.5,

            // Source files (patterns)
            pattern if pattern.starts_with("*.") => 1.0,

            // Default weight for other patterns
            _ => 1.0,
        }
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

    // =============================================================================
    // Tests for depth weighting data structures
    // =============================================================================

    #[test]
    fn test_directory_type_classification() {
        // Source directories
        assert_eq!(DirectoryType::classify("src"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("lib"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("app"), DirectoryType::Source);

        // Test directories
        assert_eq!(DirectoryType::classify("test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("tests"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("__tests__"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("fixtures"), DirectoryType::Test);

        // Build directories
        assert_eq!(DirectoryType::classify("dist"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("build"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("target"), DirectoryType::Build);

        // Dependencies
        assert_eq!(
            DirectoryType::classify("node_modules"),
            DirectoryType::Dependencies
        );
        assert_eq!(
            DirectoryType::classify("vendor"),
            DirectoryType::Dependencies
        );

        // Unknown
        assert_eq!(DirectoryType::classify("random"), DirectoryType::Unknown);
    }

    #[test]
    fn test_directory_type_weights() {
        assert_eq!(DirectoryType::Root.weight(), 1.0);
        assert_eq!(DirectoryType::Source.weight(), 1.2);
        assert_eq!(DirectoryType::Config.weight(), 1.1);
        assert_eq!(DirectoryType::Test.weight(), 0.2);
        assert_eq!(DirectoryType::Build.weight(), 0.1);
        assert_eq!(DirectoryType::Dependencies.weight(), 0.05);
        assert_eq!(DirectoryType::Documentation.weight(), 0.6);
        assert_eq!(DirectoryType::Examples.weight(), 0.3);
        assert_eq!(DirectoryType::Unknown.weight(), 0.8);
    }

    #[test]
    fn test_matched_file_root() {
        let file = MatchedFile::new("package.json".to_string(), "package.json".to_string());

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "package.json");
        assert_eq!(file.depth, 0);
        assert_eq!(file.directory_type, DirectoryType::Root);
        assert_eq!(file.weight(), 1.0); // Root depth weight (1.0) * Root type weight (1.0)
    }

    #[test]
    fn test_matched_file_source_directory() {
        let file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());

        assert_eq!(file.filename, "main.rs");
        assert_eq!(file.relative_path, "src/main.rs");
        assert_eq!(file.depth, 1);
        assert_eq!(file.directory_type, DirectoryType::Source);
        assert!((file.weight() - 0.84).abs() < f32::EPSILON); // Depth 1 weight (0.7) * Source type weight (1.2)
    }

    #[test]
    fn test_matched_file_test_directory() {
        let file = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "test/fixtures/package.json");
        assert_eq!(file.depth, 2);
        assert_eq!(file.directory_type, DirectoryType::Test);
        assert!((file.weight() - 0.08).abs() < f32::EPSILON); // Depth 2 weight (0.4) * Test type weight (0.2)
    }

    #[test]
    fn test_matched_file_deep_nesting() {
        let file = MatchedFile::new(
            "config.json".to_string(),
            "very/deep/nested/path/config.json".to_string(),
        );

        assert_eq!(file.depth, 4);
        assert_eq!(file.directory_type, DirectoryType::Unknown);
        assert!((file.weight() - 0.04).abs() < f32::EPSILON); // Deep depth weight (0.05) * Unknown type weight (0.8)
    }

    #[test]
    fn test_pattern_importance() {
        // Configuration files get high importance
        assert_eq!(MatchedFile::get_pattern_importance("package.json"), 2.0);
        assert_eq!(MatchedFile::get_pattern_importance("Cargo.toml"), 2.0);
        assert_eq!(MatchedFile::get_pattern_importance("pyproject.toml"), 2.0);

        // Build/config files get medium importance
        assert_eq!(MatchedFile::get_pattern_importance("Makefile"), 1.5);
        assert_eq!(MatchedFile::get_pattern_importance("tsconfig.json"), 1.5);

        // Source file patterns get normal importance
        assert_eq!(MatchedFile::get_pattern_importance("*.rs"), 1.0);
        assert_eq!(MatchedFile::get_pattern_importance("*.ts"), 1.0);

        // Other patterns get default importance
        assert_eq!(MatchedFile::get_pattern_importance("README.md"), 1.0);
    }

    #[test]
    fn test_depth_calculation_edge_cases() {
        // Empty path
        let file = MatchedFile::new("file.txt".to_string(), "".to_string());
        assert_eq!(file.depth, 0);

        // Single file at root
        let file = MatchedFile::new("file.txt".to_string(), "file.txt".to_string());
        assert_eq!(file.depth, 0);

        // Single directory level
        let file = MatchedFile::new("file.txt".to_string(), "dir/file.txt".to_string());
        assert_eq!(file.depth, 1);
    }

    #[test]
    fn test_weight_calculation_scenarios() {
        // Root package.json (should have maximum weight)
        let root_package = MatchedFile::new("package.json".to_string(), "package.json".to_string());
        assert_eq!(root_package.weight(), 1.0);

        // Source file (should have good weight)
        let src_file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());
        assert!((src_file.weight() - 0.84).abs() < f32::EPSILON);

        // Test fixture file (should have low weight)
        let test_fixture = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );
        assert!((test_fixture.weight() - 0.08).abs() < f32::EPSILON);

        // Node modules file (should have very low weight)
        let node_modules_file = MatchedFile::new(
            "package.json".to_string(),
            "node_modules/some-lib/package.json".to_string(),
        );
        assert!((node_modules_file.weight() - 0.02).abs() < f32::EPSILON); // Depth 2 (0.4) * Dependencies (0.05)
    }

    #[test]
    fn test_directory_classification_case_insensitive() {
        assert_eq!(DirectoryType::classify("SRC"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("Test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("DIST"), DirectoryType::Build);
        assert_eq!(
            DirectoryType::classify("Node_Modules"),
            DirectoryType::Dependencies
        );
    }
}
