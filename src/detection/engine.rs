//! Detection engine for project types and frameworks

use crate::detection::cache::{CachedDetection, DetectionCache};
use crate::detection::matchers::{
    CargoTomlMatcher, ComposerJsonMatcher, GemfileMatcher, GoModMatcher, PackageJsonMatcher,
    PyProjectTomlMatcher,
};
use crate::types::{DetectionResult, DetectionType, FrameworkMatch, ProjectIndicator};
use crate::Result;
use anyhow::Context;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

/// The main detection engine
pub struct DetectionEngine {
    languages: Vec<ProjectIndicator>,
}

impl DetectionEngine {
    /// Create a new detection engine with the given language configurations
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self { languages }
    }

    /// Detect project type in the given path
    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        // Scan for files in the project directory
        let project_files = self
            .scan_project_files(path)
            .with_context(|| format!("Failed to scan files in {}", path.display()))?;

        // Detect language based on file patterns
        let detected_language = self.detect_language(&project_files);

        // If no language detected, return empty result
        let language = match detected_language {
            Some(lang) => lang,
            None => return Ok(DetectionResult::empty()),
        };

        // Detect frameworks within the detected language
        let frameworks = self
            .detect_frameworks(path, language)
            .with_context(|| "Failed to detect frameworks")?;

        // Calculate confidence based on number of matching files
        let matching_files = self.count_matching_files(language, &project_files);
        let confidence = self.calculate_confidence(matching_files, language.files.len());

        Ok(DetectionResult::new(
            Some(language.clone()),
            frameworks,
            confidence,
        ))
    }

    /// Scan the project directory for relevant files (optimized with parallel processing)
    fn scan_project_files(&self, path: &Path) -> Result<Vec<String>> {
        // Get all unique file patterns we're looking for
        let patterns: Vec<String> = self
            .languages
            .iter()
            .flat_map(|lang| &lang.files)
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Collect all file entries first (single-threaded directory traversal)
        let file_entries: Vec<_> = WalkDir::new(path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        // Process file matching in parallel
        let matched_files: HashSet<String> = file_entries
            .par_iter()
            .filter_map(|entry| {
                let file_path = entry.path();
                let file_name = file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");

                // Check if this file matches any pattern we're interested in
                for pattern in &patterns {
                    if self.matches_pattern(file_name, pattern) {
                        return Some(file_name.to_string());
                    }
                }

                // Also check relative path for patterns like "src/*.rs"
                if let Ok(relative_path) = file_path.strip_prefix(path) {
                    if let Some(relative_str) = relative_path.to_str() {
                        for pattern in &patterns {
                            if self.matches_pattern(relative_str, pattern) {
                                return Some(relative_str.to_string());
                            }
                        }
                    }
                }

                None
            })
            .collect();

        Ok(matched_files.into_iter().collect())
    }

    /// Check if a file name matches a pattern
    fn matches_pattern(&self, file_name: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            // Simple wildcard matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                file_name.starts_with(prefix) && file_name.ends_with(suffix)
            } else {
                // For more complex patterns, just check if it contains the non-wildcard parts
                parts
                    .iter()
                    .all(|part| !part.is_empty() && file_name.contains(part))
            }
        } else {
            // Exact match
            file_name == pattern
        }
    }

    /// Detect the most likely language based on found files
    fn detect_language(&self, project_files: &[String]) -> Option<&ProjectIndicator> {
        let mut candidates: Vec<(&ProjectIndicator, usize)> = Vec::new();

        for language in &self.languages {
            let matching_count = self.count_matching_files(language, project_files);
            if matching_count > 0 {
                candidates.push((language, matching_count));
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Sort by priority (lower number = higher priority), then by matching file count
        candidates.sort_by(|a, b| {
            a.0.priority.cmp(&b.0.priority).then(b.1.cmp(&a.1)) // More matches = better
        });

        Some(candidates[0].0)
    }

    /// Count how many files match a language's patterns
    fn count_matching_files(&self, language: &ProjectIndicator, project_files: &[String]) -> usize {
        let mut count = 0;
        for pattern in &language.files {
            for file in project_files {
                if self.matches_pattern(file, pattern) {
                    count += 1;
                    break; // Don't count the same file multiple times for one language
                }
            }
        }
        count
    }

    /// Calculate confidence based on matching files
    fn calculate_confidence(&self, matching_files: usize, total_patterns: usize) -> f32 {
        if matching_files == 0 {
            return 0.0;
        }

        // Base confidence from pattern match ratio
        let pattern_ratio = matching_files as f32 / total_patterns as f32;
        let base_confidence = (pattern_ratio * 0.8).min(1.0);

        // Boost confidence for more matches
        let match_boost = (matching_files as f32 * 0.1).min(0.2);

        (base_confidence + match_boost).min(1.0)
    }

    /// Get languages sorted by priority
    pub fn languages_by_priority(&self) -> Vec<&ProjectIndicator> {
        let mut languages: Vec<&ProjectIndicator> = self.languages.iter().collect();
        languages.sort_by_key(|lang| lang.priority);
        languages
    }

    /// Find a language by name
    pub fn find_language(&self, name: &str) -> Option<&ProjectIndicator> {
        self.languages
            .iter()
            .find(|lang| lang.name.eq_ignore_ascii_case(name))
    }

    /// Detect frameworks for a given language
    fn detect_frameworks(
        &self,
        path: &Path,
        language: &ProjectIndicator,
    ) -> Result<Vec<FrameworkMatch>> {
        if language.frameworks.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_matches = Vec::new();

        // Try different matchers based on framework detection types
        for framework in &language.frameworks {
            match &framework.detection {
                DetectionType::PackageJson { .. } => {
                    let mut matches = PackageJsonMatcher::detect_frameworks(
                        path,
                        std::slice::from_ref(framework),
                    )?;
                    all_matches.append(&mut matches);
                }
                DetectionType::CargoToml { .. } => {
                    let mut matches =
                        CargoTomlMatcher::detect_frameworks(path, std::slice::from_ref(framework))?;
                    all_matches.append(&mut matches);
                }
                DetectionType::GoMod { .. } => {
                    let mut matches =
                        GoModMatcher::detect_frameworks(path, std::slice::from_ref(framework))?;
                    all_matches.append(&mut matches);
                }
                DetectionType::PyProjectToml { .. } => {
                    let mut matches = PyProjectTomlMatcher::detect_frameworks(
                        path,
                        std::slice::from_ref(framework),
                    )?;
                    all_matches.append(&mut matches);
                }
                DetectionType::GemSpec { .. } => {
                    let mut matches =
                        GemfileMatcher::detect_frameworks(path, std::slice::from_ref(framework))?;
                    all_matches.append(&mut matches);
                }
                DetectionType::ComposerJson { .. } => {
                    let mut matches = ComposerJsonMatcher::detect_frameworks(
                        path,
                        std::slice::from_ref(framework),
                    )?;
                    all_matches.append(&mut matches);
                }
                DetectionType::FileExists { files } => {
                    if self.check_file_exists(path, files) {
                        let evidence: Vec<String> = files
                            .iter()
                            .filter(|file| path.join(file).exists())
                            .cloned()
                            .collect();

                        if !evidence.is_empty() {
                            all_matches.push(FrameworkMatch::new(
                                framework.clone(),
                                0.8, // Fixed confidence for file existence
                                evidence,
                            ));
                        }
                    }
                }
                DetectionType::ConfigFile { file, keys } => {
                    if let Some(confidence) = self.check_config_file(path, file, keys)? {
                        all_matches.push(FrameworkMatch::new(
                            framework.clone(),
                            confidence,
                            vec![file.clone()],
                        ));
                    }
                }
            }
        }

        // Sort and deduplicate matches
        self.resolve_framework_matches(all_matches)
    }

    /// Check if files exist for FileExists detection type
    fn check_file_exists(&self, base_path: &Path, files: &[String]) -> bool {
        files.iter().any(|file| {
            let file_path = base_path.join(file);
            file_path.exists()
        })
    }

    /// Check config file for specific keys
    fn check_config_file(
        &self,
        base_path: &Path,
        file: &str,
        keys: &[String],
    ) -> Result<Option<f32>> {
        let config_path = base_path.join(file);

        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        // For TOML files, try to parse and check for keys
        if file.ends_with(".toml") {
            match toml::from_str::<toml::Value>(&content) {
                Ok(toml_value) => {
                    let mut found_keys = 0;
                    for key in keys {
                        if self.check_toml_key(&toml_value, key) {
                            found_keys += 1;
                        }
                    }

                    if found_keys > 0 {
                        let confidence = (found_keys as f32 / keys.len() as f32) * 0.9;
                        return Ok(Some(confidence));
                    }
                }
                Err(_) => {
                    // If TOML parsing fails, fall back to text search
                    return Ok(self.check_text_keys(&content, keys));
                }
            }
        } else {
            // For non-TOML files, do text search
            return Ok(self.check_text_keys(&content, keys));
        }

        Ok(None)
    }

    /// Check if a key exists in TOML value (supports nested keys like "tool.poetry")
    fn check_toml_key(&self, value: &toml::Value, key: &str) -> bool {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = value;

        for part in parts {
            match current {
                toml::Value::Table(table) => {
                    if let Some(next_value) = table.get(part) {
                        current = next_value;
                    } else {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }

    /// Check for keys in text content (fallback method)
    fn check_text_keys(&self, content: &str, keys: &[String]) -> Option<f32> {
        let mut found_keys = 0;
        for key in keys {
            if content.contains(key) {
                found_keys += 1;
            }
        }

        if found_keys > 0 {
            let confidence = (found_keys as f32 / keys.len() as f32) * 0.7; // Lower confidence for text search
            Some(confidence)
        } else {
            None
        }
    }

    /// Resolve and prioritize framework matches
    fn resolve_framework_matches(
        &self,
        mut matches: Vec<FrameworkMatch>,
    ) -> Result<Vec<FrameworkMatch>> {
        if matches.is_empty() {
            return Ok(matches);
        }

        // Sort by priority (lower = higher priority), then by confidence (higher = better)
        matches.sort_by(|a, b| {
            a.framework.priority.cmp(&b.framework.priority).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        // Remove duplicates (same framework name)
        let mut seen_names = HashSet::new();
        matches.retain(|m| seen_names.insert(m.framework.name.clone()));

        Ok(matches)
    }
}

impl CachedDetection for DetectionEngine {
    /// Detect project type with caching support
    fn detect_cached(&self, path: &Path, cache: &DetectionCache) -> Result<DetectionResult> {
        // Try to get from cache first
        if let Some(cached_result) = cache.get(path)? {
            return Ok(cached_result);
        }

        // Cache miss - perform detection
        let result = self.detect(path)?;

        // Store in cache for future use
        cache.put(path, result.clone())?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_language(name: &str, files: Vec<&str>, priority: u8) -> ProjectIndicator {
        ProjectIndicator {
            name: name.to_string(),
            files: files.into_iter().map(String::from).collect(),
            color: "#FF0000".to_string(),
            icon: "🔥".to_string(),
            priority,
            frameworks: Vec::new(),
        }
    }

    fn create_test_project(files: &[&str]) -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        for file_path in files {
            let full_path = temp_dir.path().join(file_path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            // Create the file
            fs::write(full_path, "test content").unwrap();
        }

        temp_dir
    }

    #[test]
    fn test_detection_engine_creation() {
        let languages = vec![
            create_test_language("Rust", vec!["Cargo.toml"], 1),
            create_test_language("JavaScript", vec!["package.json"], 1),
        ];

        let engine = DetectionEngine::new(languages);
        assert_eq!(engine.languages.len(), 2);
    }

    #[test]
    fn test_simple_pattern_matching() {
        let engine = DetectionEngine::new(vec![]);

        // Exact matches
        assert!(engine.matches_pattern("package.json", "package.json"));
        assert!(!engine.matches_pattern("package.json", "Cargo.toml"));

        // Wildcard matches
        assert!(engine.matches_pattern("main.rs", "*.rs"));
        assert!(engine.matches_pattern("lib.rs", "*.rs"));
        assert!(!engine.matches_pattern("main.py", "*.rs"));

        // Complex patterns
        assert!(engine.matches_pattern("src/main.cpp", "*.cpp"));
        assert!(engine.matches_pattern("test.hpp", "*.hpp"));
    }

    #[test]
    fn test_detect_rust_project() {
        let languages = vec![
            create_test_language("Rust", vec!["Cargo.toml"], 1),
            create_test_language("JavaScript", vec!["package.json"], 2),
        ];

        let engine = DetectionEngine::new(languages);
        let temp_project = create_test_project(&["Cargo.toml", "src/main.rs"]);

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "Rust");
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_detect_javascript_project() {
        let languages = vec![
            create_test_language("Rust", vec!["Cargo.toml"], 1),
            create_test_language("JavaScript", vec!["package.json"], 1),
        ];

        let engine = DetectionEngine::new(languages);
        let temp_project = create_test_project(&["package.json", "src/index.js"]);

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "JavaScript");
    }

    #[test]
    fn test_priority_based_detection() {
        let languages = vec![
            create_test_language("TypeScript", vec!["package.json", "tsconfig.json"], 1), // Higher priority
            create_test_language("JavaScript", vec!["package.json"], 2), // Lower priority
        ];

        let engine = DetectionEngine::new(languages);

        // Project with both package.json and tsconfig.json should detect TypeScript
        let ts_project = create_test_project(&["package.json", "tsconfig.json"]);
        let result = engine.detect(ts_project.path()).unwrap();
        assert_eq!(result.language.as_ref().unwrap().name, "TypeScript");

        // Project with only package.json should still detect TypeScript (higher priority)
        let js_project = create_test_project(&["package.json"]);
        let result = engine.detect(js_project.path()).unwrap();
        assert_eq!(result.language.as_ref().unwrap().name, "TypeScript");
    }

    #[test]
    fn test_no_detection() {
        let languages = vec![
            create_test_language("Rust", vec!["Cargo.toml"], 1),
            create_test_language("JavaScript", vec!["package.json"], 1),
        ];

        let engine = DetectionEngine::new(languages);
        let temp_project = create_test_project(&["README.md", "LICENSE"]);

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(result.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_confidence_calculation() {
        let engine = DetectionEngine::new(vec![]);

        // Test various scenarios
        assert_eq!(engine.calculate_confidence(0, 2), 0.0);
        assert!(engine.calculate_confidence(1, 2) > 0.0);
        assert!(engine.calculate_confidence(2, 2) > engine.calculate_confidence(1, 2));
        assert!(engine.calculate_confidence(3, 2) <= 1.0); // Should not exceed 1.0
    }

    #[test]
    fn test_wildcard_patterns() {
        let languages = vec![create_test_language("C++", vec!["*.cpp", "*.hpp"], 1)];

        let engine = DetectionEngine::new(languages);
        let temp_project =
            create_test_project(&["src/main.cpp", "include/header.hpp", "README.md"]);

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "C++");
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_file_scanning() {
        let languages = vec![create_test_language("Test", vec!["test.file"], 1)];

        let engine = DetectionEngine::new(languages);
        let temp_project = create_test_project(&["test.file", "other.file", "subdir/nested.file"]);

        let files = engine.scan_project_files(temp_project.path()).unwrap();

        // Should find the test.file
        assert!(files.contains(&"test.file".to_string()));
        // Should not contain files we're not looking for
        assert!(!files.contains(&"other.file".to_string()));
    }

    #[test]
    fn test_framework_detection_with_package_json() {
        use crate::types::{DetectionType, FrameworkDetector};

        let react_framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: Some("⚛️".to_string()),
            color: Some("#61DAFB".to_string()),
            priority: 1,
            files: vec![],
        };

        let mut typescript_lang = create_test_language("TypeScript", vec!["package.json"], 1);
        typescript_lang.frameworks = vec![react_framework];

        let engine = DetectionEngine::new(vec![typescript_lang]);

        // Create project with React dependency
        let temp_project = create_test_project(&["package.json"]);
        let package_content = r#"{"dependencies": {"react": "^18.0.0"}}"#;
        fs::write(temp_project.path().join("package.json"), package_content).unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "TypeScript");
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "React");
        assert!(result.frameworks[0].confidence > 0.0);
    }

    #[test]
    fn test_framework_detection_with_cargo_toml() {
        use crate::types::{DetectionType, FrameworkDetector};

        let rocket_framework = FrameworkDetector {
            name: "Rocket".to_string(),
            detection: DetectionType::CargoToml {
                dependencies: vec!["rocket".to_string()],
            },
            icon: Some("🚀".to_string()),
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut rust_lang = create_test_language("Rust", vec!["Cargo.toml"], 1);
        rust_lang.frameworks = vec![rocket_framework];

        let engine = DetectionEngine::new(vec![rust_lang]);

        // Create project with Rocket dependency
        let temp_project = create_test_project(&["Cargo.toml"]);
        let cargo_content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
rocket = "0.5"
"#;
        fs::write(temp_project.path().join("Cargo.toml"), cargo_content).unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "Rust");
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "Rocket");
    }

    #[test]
    fn test_framework_detection_file_exists() {
        use crate::types::{DetectionType, FrameworkDetector};

        let nextjs_framework = FrameworkDetector {
            name: "Next.js".to_string(),
            detection: DetectionType::FileExists {
                files: vec!["next.config.js".to_string()],
            },
            icon: Some("▲".to_string()),
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut typescript_lang = create_test_language("TypeScript", vec!["package.json"], 1);
        typescript_lang.frameworks = vec![nextjs_framework];

        let engine = DetectionEngine::new(vec![typescript_lang]);

        // Create project with Next.js config file
        let temp_project = create_test_project(&["package.json", "next.config.js"]);

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "Next.js");
        assert_eq!(result.frameworks[0].confidence, 0.8);
        assert!(result.frameworks[0]
            .evidence
            .contains(&"next.config.js".to_string()));
    }

    #[test]
    fn test_framework_detection_config_file() {
        use crate::types::{DetectionType, FrameworkDetector};

        let poetry_framework = FrameworkDetector {
            name: "Poetry".to_string(),
            detection: DetectionType::ConfigFile {
                file: "pyproject.toml".to_string(),
                keys: vec!["tool.poetry".to_string()],
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut python_lang = create_test_language("Python", vec!["pyproject.toml"], 1);
        python_lang.frameworks = vec![poetry_framework];

        let engine = DetectionEngine::new(vec![python_lang]);

        // Create project with Poetry config
        let temp_project = create_test_project(&["pyproject.toml"]);
        let pyproject_content = r#"
[tool.poetry]
name = "test-project"
version = "0.1.0"
"#;
        fs::write(
            temp_project.path().join("pyproject.toml"),
            pyproject_content,
        )
        .unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "Poetry");
        assert!(result.frameworks[0].confidence > 0.0);
    }

    #[test]
    fn test_multiple_frameworks_priority() {
        use crate::types::{DetectionType, FrameworkDetector};

        let react_framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: None,
            color: None,
            priority: 1, // Higher priority
            files: vec![],
        };

        let vue_framework = FrameworkDetector {
            name: "Vue".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["vue".to_string()],
            },
            icon: None,
            color: None,
            priority: 2, // Lower priority
            files: vec![],
        };

        let mut javascript_lang = create_test_language("JavaScript", vec!["package.json"], 1);
        javascript_lang.frameworks = vec![vue_framework, react_framework]; // Note: Vue first in list

        let engine = DetectionEngine::new(vec![javascript_lang]);

        // Create project with both React and Vue (should prefer React due to higher priority)
        let temp_project = create_test_project(&["package.json"]);
        let package_content = r#"{"dependencies": {"react": "^18.0.0", "vue": "^3.0.0"}}"#;
        fs::write(temp_project.path().join("package.json"), package_content).unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.frameworks.len(), 2);
        // React should be first due to higher priority (lower number)
        assert_eq!(result.frameworks[0].framework.name, "React");
        assert_eq!(result.frameworks[1].framework.name, "Vue");
    }

    #[test]
    fn test_framework_resolution_deduplication() {
        use crate::types::{DetectionType, FrameworkDetector};

        // Two frameworks with same name but different detection methods
        let react_framework1 = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let react_framework2 = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::FileExists {
                files: vec!["src/App.js".to_string()],
            },
            icon: None,
            color: None,
            priority: 2,
            files: vec![],
        };

        let mut javascript_lang = create_test_language("JavaScript", vec!["package.json"], 1);
        javascript_lang.frameworks = vec![react_framework1, react_framework2];

        let engine = DetectionEngine::new(vec![javascript_lang]);

        // Create project that matches both React detection methods
        let temp_project = create_test_project(&["package.json", "src/App.js"]);
        let package_content = r#"{"dependencies": {"react": "^18.0.0"}}"#;
        fs::write(temp_project.path().join("package.json"), package_content).unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        // Should only have one React framework (deduplicated)
        assert_eq!(result.frameworks.len(), 1);
        assert_eq!(result.frameworks[0].framework.name, "React");
        // Should prefer the higher priority one (priority 1)
        assert_eq!(result.frameworks[0].framework.priority, 1);
    }

    #[test]
    fn test_no_frameworks_detected() {
        use crate::types::{DetectionType, FrameworkDetector};

        let react_framework = FrameworkDetector {
            name: "React".to_string(),
            detection: DetectionType::PackageJson {
                dependencies: vec!["react".to_string()],
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut javascript_lang = create_test_language("JavaScript", vec!["package.json"], 1);
        javascript_lang.frameworks = vec![react_framework];

        let engine = DetectionEngine::new(vec![javascript_lang]);

        // Create project without React
        let temp_project = create_test_project(&["package.json"]);
        let package_content = r#"{"dependencies": {"lodash": "^4.17.21"}}"#;
        fs::write(temp_project.path().join("package.json"), package_content).unwrap();

        let result = engine.detect(temp_project.path()).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "JavaScript");
        assert!(result.frameworks.is_empty()); // No frameworks detected
    }

    #[test]
    fn test_toml_key_checking() {
        let engine = DetectionEngine::new(vec![]);

        let toml_content = r#"
[tool.poetry]
name = "test"

[tool.black]
line-length = 88

[build-system]
requires = ["poetry"]
"#;

        let toml_value: toml::Value = toml::from_str(toml_content).unwrap();

        assert!(engine.check_toml_key(&toml_value, "tool.poetry"));
        assert!(engine.check_toml_key(&toml_value, "tool.black"));
        assert!(engine.check_toml_key(&toml_value, "build-system"));
        assert!(!engine.check_toml_key(&toml_value, "tool.nonexistent"));
        assert!(!engine.check_toml_key(&toml_value, "tool.poetry.nonexistent"));
    }

    #[test]
    fn test_text_key_checking() {
        let engine = DetectionEngine::new(vec![]);

        let text_content = "This is a test file\nwith some keywords\nincluding Django and Flask";

        let result = engine
            .check_text_keys(text_content, &["Django".to_string(), "Flask".to_string()])
            .unwrap();

        assert!(result > 0.0);
        assert!(result <= 0.7); // Should be <= 0.7 due to text search penalty

        let no_match = engine.check_text_keys(text_content, &["NonExistent".to_string()]);

        assert!(no_match.is_none());
    }
}
