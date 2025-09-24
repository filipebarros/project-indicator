use crate::patterns::{pattern_to_regex, simple_wildcard_match};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndicator {
    pub name: String,
    pub files: Vec<String>,
    pub color: String,
    pub icon: String,
    pub priority: u8,
    #[serde(default)]
    pub frameworks: Vec<FrameworkDetector>,
    #[serde(default)]
    pub root_indicators: Vec<RootIndicator>,
    #[serde(skip)]
    compiled_patterns: OnceLock<HashMap<String, Option<Regex>>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameworkDetector {
    pub name: String,
    pub detection: DetectionType,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub priority: u8,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub root_indicators: Vec<RootIndicator>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DetectionType {
    NodeEcosystem { dependencies: Vec<String> },
    PythonEcosystem { dependencies: Vec<String> },
    RustEcosystem { dependencies: Vec<String> },
    GoEcosystem { modules: Vec<String> },
    PHPEcosystem { packages: Vec<String> },
    RubyEcosystem { gems: Vec<String> },
    JavaEcosystem { dependencies: Vec<String> },
    DotNetEcosystem { packages: Vec<String> },
    ScalaEcosystem { dependencies: Vec<String> },
    DartEcosystem { dependencies: Vec<String> },
    LuaEcosystem { packages: Vec<String> },
    FileExists { files: Vec<String> },
    ConfigFile { file: String, keys: Vec<String> },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionEvidence {
    pub language_evidence: Vec<EvidenceItem>,
    pub framework_evidence: Vec<EvidenceItem>,
    pub root_discovery: Vec<EvidenceItem>,
    pub confidence_factors: Vec<ConfidenceFactor>,
    pub files_scanned: usize,
    pub total_scan_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub evidence_type: EvidenceType,
    pub file_path: String,
    pub pattern_matched: String,
    pub weight: f32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFactor {
    pub factor_type: String,
    pub value: f32,
    pub weight: f32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvidenceType {
    LanguageFile,
    ManifestFile,
    ConfigFile,
    FrameworkDependency,
    RootIndicator,
    DirectoryStructure,
}

impl DetectionEvidence {
    pub fn new() -> Self {
        Self {
            language_evidence: Vec::new(),
            framework_evidence: Vec::new(),
            root_discovery: Vec::new(),
            confidence_factors: Vec::new(),
            files_scanned: 0,
            total_scan_time_ms: 0,
        }
    }

    pub fn add_language_evidence(&mut self, evidence: EvidenceItem) {
        self.language_evidence.push(evidence);
    }

    pub fn add_framework_evidence(&mut self, evidence: EvidenceItem) {
        self.framework_evidence.push(evidence);
    }

    pub fn add_root_evidence(&mut self, evidence: EvidenceItem) {
        self.root_discovery.push(evidence);
    }

    pub fn add_confidence_factor(&mut self, factor: ConfidenceFactor) {
        self.confidence_factors.push(factor);
    }

    pub fn set_scan_metrics(&mut self, files_scanned: usize, scan_time_ms: u64) {
        self.files_scanned = files_scanned;
        self.total_scan_time_ms = scan_time_ms;
    }
}

impl EvidenceItem {
    pub fn new(
        evidence_type: EvidenceType,
        file_path: String,
        pattern_matched: String,
        weight: f32,
        description: String,
    ) -> Self {
        Self {
            evidence_type,
            file_path,
            pattern_matched,
            weight,
            description,
        }
    }

    pub fn language_file(file_path: String, pattern: String, weight: f32) -> Self {
        Self {
            evidence_type: EvidenceType::LanguageFile,
            file_path: file_path.clone(),
            pattern_matched: pattern.clone(),
            weight,
            description: format!("Matched {} against pattern {}", file_path, pattern),
        }
    }

    pub fn manifest_file(file_path: String, pattern: String, weight: f32) -> Self {
        Self {
            evidence_type: EvidenceType::ManifestFile,
            file_path: file_path.clone(),
            pattern_matched: pattern.clone(),
            weight,
            description: format!("Found manifest file: {}", file_path),
        }
    }

    pub fn config_file(file_path: String, pattern: String, weight: f32) -> Self {
        Self {
            evidence_type: EvidenceType::ConfigFile,
            file_path: file_path.clone(),
            pattern_matched: pattern.clone(),
            weight,
            description: format!("Found config file: {}", file_path),
        }
    }

    pub fn framework_dependency(file_path: String, framework_name: &str, confidence: f32) -> Self {
        Self {
            evidence_type: EvidenceType::FrameworkDependency,
            file_path,
            pattern_matched: "FRAMEWORK_DETECTION".to_owned(),
            weight: confidence,
            description: format!("Detected {} framework", framework_name),
        }
    }

    pub fn root_indicator(file_path: String, pattern: String, weight: f32) -> Self {
        Self {
            evidence_type: EvidenceType::RootIndicator,
            file_path: file_path.clone(),
            pattern_matched: pattern.clone(),
            weight,
            description: format!("Root indicator found: {}", file_path),
        }
    }
}

impl ConfidenceFactor {
    pub fn new(factor_type: String, value: f32, weight: f32, description: String) -> Self {
        Self {
            factor_type,
            value,
            weight,
            description,
        }
    }
}

impl Default for DetectionEvidence {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    pub language: Option<Arc<ProjectIndicator>>,
    pub frameworks: Vec<FrameworkMatch>,
    pub confidence: f32,
    pub evidence: DetectionEvidence,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameworkMatch {
    pub framework: FrameworkDetector,
    pub confidence: f32,
    pub evidence: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    pub show_frameworks: bool,
    pub max_frameworks: usize,
    pub framework_separator: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub ttl_seconds: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigMeta {
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum IndicatorContext {
    VersionControl,
    #[default]
    LanguageRoot,
    FrameworkRoot,
    BuildSystem,
    Configuration,
}

impl IndicatorContext {
    pub fn base_priority(&self) -> f32 {
        match self {
            IndicatorContext::VersionControl => 1.0,
            IndicatorContext::LanguageRoot => 0.9,
            IndicatorContext::FrameworkRoot => 0.8,
            IndicatorContext::BuildSystem => 0.7,
            IndicatorContext::Configuration => 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RootIndicator {
    pub pattern: String,
    pub weight: f32,
    #[serde(default)]
    pub context: IndicatorContext,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetectionMode {
    Fast,
    Thorough,
}

impl Default for DetectionMode {
    fn default() -> Self {
        Self::Thorough
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionConfig {
    pub max_upward_traversal: usize,
    pub require_vcs_root: bool,
    pub confidence_threshold: f32,
    pub root_indicators: Vec<RootIndicator>,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub detection_mode: DetectionMode,
}

fn default_max_depth() -> usize {
    1
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
            ttl_seconds: 300,
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
            root_indicators: vec![],
            max_depth: default_max_depth(),
            detection_mode: DetectionMode::default(),
        }
    }
}

impl DetectionConfig {
    pub fn all_root_indicators(&self) -> Vec<&RootIndicator> {
        self.root_indicators.iter().collect()
    }
}

impl DetectionResult {
    pub fn new(
        language: Option<Arc<ProjectIndicator>>,
        frameworks: Vec<FrameworkMatch>,
        confidence: f32,
    ) -> Self {
        Self {
            language,
            frameworks,
            confidence,
            evidence: DetectionEvidence::new(),
        }
    }

    pub fn new_with_evidence(
        language: Option<Arc<ProjectIndicator>>,
        frameworks: Vec<FrameworkMatch>,
        confidence: f32,
        evidence: DetectionEvidence,
    ) -> Self {
        Self {
            language,
            frameworks,
            confidence,
            evidence,
        }
    }

    pub fn empty() -> Self {
        Self {
            language: None,
            frameworks: Vec::new(),
            confidence: 0.0,
            evidence: DetectionEvidence::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.language.is_none() && self.frameworks.is_empty()
    }
    pub fn best_framework(&self) -> Option<&FrameworkMatch> {
        self.frameworks.iter().min_by(|a, b| {
            a.framework.priority.cmp(&b.framework.priority).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        })
    }
    pub fn display_icon(&self) -> Option<&str> {
        self.best_framework()
            .and_then(|f| f.framework.icon.as_deref())
            .or_else(|| self.language.as_ref().map(|l| l.icon.as_str()))
    }
    pub fn display_color(&self) -> Option<&str> {
        self.best_framework()
            .and_then(|f| f.framework.color.as_deref())
            .or_else(|| self.language.as_ref().map(|l| l.color.as_str()))
    }
}

impl FrameworkMatch {
    pub fn new(framework: FrameworkDetector, confidence: f32, evidence: Vec<String>) -> Self {
        Self {
            framework,
            confidence,
            evidence,
        }
    }
}

impl PartialEq for ProjectIndicator {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.files == other.files
            && self.color == other.color
            && self.icon == other.icon
            && self.priority == other.priority
            && self.frameworks == other.frameworks
            && self.root_indicators == other.root_indicators
    }
}

impl Default for ProjectIndicator {
    fn default() -> Self {
        Self {
            name: String::new(),
            files: Vec::new(),
            color: String::new(),
            icon: String::new(),
            priority: 1,
            frameworks: Vec::new(),
            root_indicators: Vec::new(),
            compiled_patterns: OnceLock::new(),
        }
    }
}

impl ProjectIndicator {
    pub fn new(
        name: String,
        files: Vec<String>,
        color: String,
        icon: String,
        priority: u8,
        frameworks: Vec<FrameworkDetector>,
    ) -> Self {
        Self {
            name,
            files,
            color,
            icon,
            priority,
            frameworks,
            root_indicators: Vec::new(),
            compiled_patterns: OnceLock::new(),
        }
    }

    pub fn with_root_indicators(
        name: String,
        files: Vec<String>,
        color: String,
        icon: String,
        priority: u8,
        frameworks: Vec<FrameworkDetector>,
        root_indicators: Vec<RootIndicator>,
    ) -> Self {
        Self {
            name,
            files,
            color,
            icon,
            priority,
            frameworks,
            root_indicators,
            compiled_patterns: OnceLock::new(),
        }
    }
    fn get_compiled_patterns(&self) -> &HashMap<String, Option<Regex>> {
        self.compiled_patterns.get_or_init(|| {
            let mut patterns = HashMap::new();
            for pattern in &self.files {
                let regex_opt = if pattern.contains('*') {
                    pattern_to_regex(pattern).and_then(|s| Regex::new(&s).ok())
                } else {
                    None
                };
                patterns.insert(pattern.clone(), regex_opt);
            }
            patterns
        })
    }
    pub fn matches_files(&self, files: &[String]) -> bool {
        let compiled_patterns = self.get_compiled_patterns();

        files.iter().any(|file| {
            self.files.iter().any(|pattern| {
                if let Some(Some(re)) = compiled_patterns.get(pattern) {
                    re.is_match(file)
                } else if pattern.contains('*') {
                    simple_wildcard_match(file, pattern)
                } else {
                    file == pattern
                }
            })
        })
    }
    pub fn frameworks_by_priority(&self) -> Vec<&FrameworkDetector> {
        let mut frameworks: Vec<&FrameworkDetector> = self.frameworks.iter().collect();
        frameworks.sort_by_key(|f| f.priority);
        frameworks
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DirectoryType {
    Root,
    Source,
    Config,
    Test,
    Build,
    Dependencies,
    Documentation,
    Examples,
    Unknown,
}

impl DirectoryType {
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
    pub fn classify(path_component: &str) -> Self {
        match path_component.to_lowercase().as_str() {
            "src" | "lib" | "app" | "source" | "code" => DirectoryType::Source,

            "test" | "tests" | "spec" | "specs" | "__tests__" | "fixtures" | "test-fixtures"
            | "__test__" | "__spec__" | "testing" => DirectoryType::Test,

            "dist" | "build" | "target" | "out" | "output" | "bin" | "release" | "debug" => {
                DirectoryType::Build
            }

            "node_modules" | "vendor" | ".git" | "packages" | "deps" => DirectoryType::Dependencies,

            "docs" | "doc" | "documentation" | "manual" | "guide" => DirectoryType::Documentation,

            "examples" | "example" | "samples" | "sample" | "demo" | "demos" => {
                DirectoryType::Examples
            }

            ".github" | ".vscode" | ".idea" | "config" | "configuration" | "configs"
            | "settings" | ".config" => DirectoryType::Config,

            _ => DirectoryType::Unknown,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedFile {
    pub filename: String,
    pub relative_path: String,
    pub depth: usize,
    pub directory_type: DirectoryType,
}

impl MatchedFile {
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
    pub fn weight(&self) -> f32 {
        Self::calculate_weight(self.depth, self.directory_type)
    }
    fn calculate_depth(relative_path: &str) -> usize {
        if relative_path.is_empty() {
            return 0;
        }

        relative_path.matches('/').count()
    }
    fn classify_directory(relative_path: &str) -> DirectoryType {
        if relative_path.is_empty() || !relative_path.contains('/') {
            return DirectoryType::Root;
        }

        let first_component = relative_path.split('/').next().unwrap_or("");
        DirectoryType::classify(first_component)
    }
    fn calculate_weight(depth: usize, directory_type: DirectoryType) -> f32 {
        let depth_weight = match depth {
            0 => 1.0,
            1 => 0.7,
            2 => 0.4,
            3 => 0.1,
            _ => 0.05,
        };

        depth_weight * directory_type.weight()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_indicator_creation() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "TypeScript".to_string(),
            vec!["package.json".to_string(), "tsconfig.json".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            1,
            vec![],
        );

        assert_eq!(indicator.name, "TypeScript");
        assert_eq!(indicator.files.len(), 2);
        assert_eq!(indicator.priority, 1);
        Ok(())
    }

    #[test]
    fn test_file_matching() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "TypeScript".to_string(),
            vec!["package.json".to_string(), "*.ts".to_string()],
            "#3178C6".to_string(),
            "󰛦".to_string(),
            1,
            vec![],
        );

        let files = vec![
            "package.json".to_string(),
            "src/main.ts".to_string(),
            "README.md".to_string(),
        ];

        assert!(indicator.matches_files(&files));

        let no_match_files = vec!["README.md".to_string(), "main.py".to_string()];
        assert!(!indicator.matches_files(&no_match_files));
        Ok(())
    }

    #[test]
    fn test_wildcard_matching() -> Result<(), Box<dyn std::error::Error>> {
        let indicator = ProjectIndicator::new(
            "C++".to_string(),
            vec!["*.cpp".to_string(), "*.h".to_string()],
            "#00599C".to_string(),
            "".to_string(),
            1,
            vec![],
        );

        let files = vec!["main.cpp".to_string(), "header.h".to_string()];
        assert!(indicator.matches_files(&files));

        let no_match = vec!["main.py".to_string()];
        assert!(!indicator.matches_files(&no_match));
        Ok(())
    }

    #[test]
    fn test_detection_result_empty() -> Result<(), Box<dyn std::error::Error>> {
        let result = DetectionResult::empty();
        assert!(result.is_empty());
        assert_eq!(result.confidence, 0.0);
        assert!(result.best_framework().is_none());
        Ok(())
    }

    #[test]
    fn test_detection_result_with_framework() -> Result<(), Box<dyn std::error::Error>> {
        let framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
            icon: Some("⚛️".to_string()),
            color: Some("#61DAFB".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

        let result = DetectionResult::new(None, vec![framework_match], 0.9);

        assert!(!result.is_empty());
        assert_eq!(result.display_icon(), Some("⚛️"));
        assert_eq!(result.display_color(), Some("#61DAFB"));
        assert!(result.best_framework().is_some());
        Ok(())
    }

    #[test]
    fn test_framework_priority_sorting() -> Result<(), Box<dyn std::error::Error>> {
        let framework1 = FrameworkDetector {
            name: "Framework1".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 3,
            files: vec![],
            root_indicators: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "Framework2".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let indicator = ProjectIndicator::new(
            "Test".to_string(),
            vec![],
            "#000000".to_string(),
            "".to_string(),
            1,
            vec![framework1, framework2],
        );

        let sorted = indicator.frameworks_by_priority();
        assert_eq!(sorted[0].name, "Framework2");
        assert_eq!(sorted[1].name, "Framework1");
        Ok(())
    }

    #[test]
    fn test_serde_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let detection_type = DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string(), "typescript".to_string()],
        };

        let json = serde_json::to_string(&detection_type)?;
        let deserialized: DetectionType = serde_json::from_str(&json)?;

        assert_eq!(detection_type, deserialized);
        Ok(())
    }

    #[test]
    fn test_display_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config = DisplayConfig::default();
        assert!(config.show_frameworks);
        assert_eq!(config.max_frameworks, 2);
        assert_eq!(config.framework_separator, "+");
        Ok(())
    }

    #[test]
    fn test_cache_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_seconds, 300);
        Ok(())
    }

    #[test]
    fn test_directory_type_classification() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::classify("src"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("lib"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("app"), DirectoryType::Source);

        assert_eq!(DirectoryType::classify("test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("tests"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("__tests__"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("fixtures"), DirectoryType::Test);

        assert_eq!(DirectoryType::classify("dist"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("build"), DirectoryType::Build);
        assert_eq!(DirectoryType::classify("target"), DirectoryType::Build);

        assert_eq!(
            DirectoryType::classify("node_modules"),
            DirectoryType::Dependencies
        );
        assert_eq!(
            DirectoryType::classify("vendor"),
            DirectoryType::Dependencies
        );

        assert_eq!(DirectoryType::classify("random"), DirectoryType::Unknown);
        Ok(())
    }

    #[test]
    fn test_directory_type_weights() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::Root.weight(), 1.0);
        assert_eq!(DirectoryType::Source.weight(), 1.2);
        assert_eq!(DirectoryType::Config.weight(), 1.1);
        assert_eq!(DirectoryType::Test.weight(), 0.2);
        assert_eq!(DirectoryType::Build.weight(), 0.1);
        assert_eq!(DirectoryType::Dependencies.weight(), 0.05);
        assert_eq!(DirectoryType::Documentation.weight(), 0.6);
        assert_eq!(DirectoryType::Examples.weight(), 0.3);
        assert_eq!(DirectoryType::Unknown.weight(), 0.8);
        Ok(())
    }

    #[test]
    fn test_matched_file_root() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("package.json".to_string(), "package.json".to_string());

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "package.json");
        assert_eq!(file.depth, 0);
        assert_eq!(file.directory_type, DirectoryType::Root);
        assert_eq!(file.weight(), 1.0);
        Ok(())
    }

    #[test]
    fn test_matched_file_source_directory() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());

        assert_eq!(file.filename, "main.rs");
        assert_eq!(file.relative_path, "src/main.rs");
        assert_eq!(file.depth, 1);
        assert_eq!(file.directory_type, DirectoryType::Source);
        assert!((file.weight() - 0.84).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_matched_file_test_directory() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );

        assert_eq!(file.filename, "package.json");
        assert_eq!(file.relative_path, "test/fixtures/package.json");
        assert_eq!(file.depth, 2);
        assert_eq!(file.directory_type, DirectoryType::Test);
        assert!((file.weight() - 0.08).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_matched_file_deep_nesting() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new(
            "config.json".to_string(),
            "very/deep/nested/path/config.json".to_string(),
        );

        assert_eq!(file.depth, 4);
        assert_eq!(file.directory_type, DirectoryType::Unknown);
        assert!((file.weight() - 0.04).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_depth_calculation_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let file = MatchedFile::new("file.txt".to_string(), "".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "file.txt".to_string());
        assert_eq!(file.depth, 0);

        let file = MatchedFile::new("file.txt".to_string(), "dir/file.txt".to_string());
        assert_eq!(file.depth, 1);
        Ok(())
    }

    #[test]
    fn test_weight_calculation_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let root_package = MatchedFile::new("package.json".to_string(), "package.json".to_string());
        assert_eq!(root_package.weight(), 1.0);

        let src_file = MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string());
        assert!((src_file.weight() - 0.84).abs() < f32::EPSILON);

        let test_fixture = MatchedFile::new(
            "package.json".to_string(),
            "test/fixtures/package.json".to_string(),
        );
        assert!((test_fixture.weight() - 0.08).abs() < f32::EPSILON);

        let node_modules_file = MatchedFile::new(
            "package.json".to_string(),
            "node_modules/some-lib/package.json".to_string(),
        );
        assert!((node_modules_file.weight() - 0.02).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn test_directory_classification_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DirectoryType::classify("SRC"), DirectoryType::Source);
        assert_eq!(DirectoryType::classify("Test"), DirectoryType::Test);
        assert_eq!(DirectoryType::classify("DIST"), DirectoryType::Build);
        assert_eq!(
            DirectoryType::classify("Node_Modules"),
            DirectoryType::Dependencies
        );
        Ok(())
    }
}
