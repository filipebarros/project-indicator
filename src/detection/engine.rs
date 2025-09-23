//! Detection engine for project types and frameworks

use crate::detection::cache::{CachedDetection, DetectionCache};
use crate::detection::matchers::{
    CargoTomlMatcher, ComposerJsonMatcher, GemfileMatcher, GoModMatcher, PackageJsonMatcher,
    PyProjectTomlMatcher,
};
use crate::detection::root_discovery::RootDiscovery;
use crate::patterns::{pattern_to_regex, simple_wildcard_match};
use crate::types::{
    DetectionConfig, DetectionResult, DetectionType, FrameworkMatch, ProjectIndicator,
};
use crate::Result;
use anyhow::Context;
use rayon::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;

/// The main detection engine
pub struct DetectionEngine {
    languages: Vec<Arc<ProjectIndicator>>,
    // Root discovery module
    root_discovery: RootDiscovery,
    // Compiled regex patterns for efficient matching
    pattern_cache: HashMap<String, Regex>,
    // Pre-computed unique patterns to avoid repeated allocation
    unique_patterns: Vec<String>,
}

impl DetectionEngine {
    /// Create a new detection engine with the given language configurations
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self::with_config(languages, DetectionConfig::default())
    }

    /// Create a new detection engine with specific configuration
    pub fn with_config(
        languages: Vec<ProjectIndicator>,
        detection_config: DetectionConfig,
    ) -> Self {
        let languages: Vec<Arc<ProjectIndicator>> = languages.into_iter().map(Arc::new).collect();
        let mut engine = Self {
            languages,
            root_discovery: RootDiscovery::new(detection_config),
            pattern_cache: HashMap::new(),
            unique_patterns: Vec::new(),
        };
        engine.precompute_patterns();
        engine
    }

    /// Pre-compute patterns and compile regex for efficient matching
    fn precompute_patterns(&mut self) {
        // Collect unique patterns (avoiding repeated allocations during scanning)
        let unique_patterns: HashSet<String> = self
            .languages
            .iter()
            .flat_map(|lang| lang.files.iter())
            .cloned()
            .collect();

        self.unique_patterns = unique_patterns.into_iter().collect();

        // Compile regex patterns for wildcard patterns
        for pattern in &self.unique_patterns {
            if pattern.contains('*') {
                if let Some(regex_pattern) = pattern_to_regex(pattern) {
                    if let Ok(compiled) = Regex::new(&regex_pattern) {
                        self.pattern_cache.insert(pattern.clone(), compiled);
                    }
                }
            }
        }
    }

    /// Detect project type in the given path
    pub fn detect(&self, path: &Path) -> Result<DetectionResult> {
        // Determine if we should attempt root discovery based on configured indicators
        let should_discover_root = self.root_discovery.is_enabled();

        if should_discover_root {
            // Strategy 1: Try to find project root and detect from there
            if let Some(project_root) = self.root_discovery.find_project_root(path) {
                if project_root != path {
                    let result = self.detect_from_path(&project_root)?;
                    if !result.is_empty() {
                        return Ok(result);
                    }
                }
            }
        }

        // Strategy 2: Detect from current directory (fallback)
        let result = self.detect_from_path(path)?;
        if !result.is_empty() {
            return Ok(result);
        }

        // Strategy 3: Try parent directories (limited upward search) as last resort
        if should_discover_root {
            for ancestor in path.ancestors().skip(1).take(3) {
                let result = self.detect_from_path(ancestor)?;
                if !result.is_empty() {
                    return Ok(result);
                }
            }
        }

        Ok(DetectionResult::empty())
    }

    /// Internal method to detect project type from a specific path
    fn detect_from_path(&self, path: &Path) -> Result<DetectionResult> {
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

    /// Scan the project directory for relevant files (optimized with early termination)
    fn scan_project_files(&self, path: &Path) -> Result<Vec<String>> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        // Use pre-computed patterns (no allocation during scanning)
        let patterns = &self.unique_patterns;

        // Early termination flag - stop when we have enough evidence
        let should_stop = Arc::new(AtomicBool::new(false));
        let matched_files = Arc::new(std::sync::Mutex::new(HashSet::new()));

        // Optimized directory traversal with early termination
        WalkDir::new(path)
            .max_depth(3)
            .into_iter()
            .par_bridge() // Convert to parallel iterator
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .take_any_while(|_| !should_stop.load(Ordering::Relaxed))
            .for_each(|entry| {
                // Skip if we already found enough matches
                if should_stop.load(Ordering::Relaxed) {
                    return;
                }

                let file_path = entry.path();
                let file_name = file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");

                let mut found_match = None;

                // Check if this file matches any pattern we're interested in
                for pattern in patterns {
                    if self.matches_pattern(file_name, pattern) {
                        found_match = Some(file_name.to_string());
                        break;
                    }
                }

                // Also check relative path for patterns like "src/*.rs"
                if found_match.is_none() {
                    if let Ok(relative_path) = file_path.strip_prefix(path) {
                        if let Some(relative_str) = relative_path.to_str() {
                            for pattern in patterns {
                                if self.matches_pattern(relative_str, pattern) {
                                    found_match = Some(relative_str.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }

                // If we found a match, add it and check if we should stop
                if let Some(matched_file) = found_match {
                    if let Ok(mut files) = matched_files.lock() {
                        files.insert(matched_file);

                        // Early termination: stop if we have matches for most patterns
                        // This significantly speeds up scanning in large projects
                        if files.len() >= patterns.len().min(10) {
                            should_stop.store(true, Ordering::Relaxed);
                        }
                    }
                }
            });

        let final_files = matched_files
            .lock()
            .map_err(|_| anyhow::anyhow!("File scanning mutex poisoned"))?;
        Ok(final_files.iter().cloned().collect())
    }

    /// Check if a file name matches a pattern (optimized with compiled regex)
    fn matches_pattern(&self, file_name: &str, pattern: &str) -> bool {
        // Use compiled regex if available (for wildcard patterns)
        if let Some(compiled_pattern) = self.pattern_cache.get(pattern) {
            compiled_pattern.is_match(file_name)
        } else if pattern.contains('*') {
            // Fallback for wildcard patterns not in cache
            simple_wildcard_match(file_name, pattern)
        } else {
            // Fast path for exact matches (no wildcards)
            file_name == pattern
        }
    }

    /// Detect the most likely language based on found files
    fn detect_language(&self, project_files: &[String]) -> Option<&Arc<ProjectIndicator>> {
        let mut candidates: Vec<(&Arc<ProjectIndicator>, usize)> = Vec::new();

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

    /// Count how many files match a language's patterns (optimized to avoid nested loops)
    fn count_matching_files(
        &self,
        language: &Arc<ProjectIndicator>,
        project_files: &[String],
    ) -> usize {
        project_files
            .iter()
            .filter(|file| {
                // Check if this file matches any of the language's patterns
                language
                    .files
                    .iter()
                    .any(|pattern| self.matches_pattern(file, pattern))
            })
            .count()
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
    pub fn languages_by_priority(&self) -> Vec<&Arc<ProjectIndicator>> {
        let mut languages: Vec<&Arc<ProjectIndicator>> = self.languages.iter().collect();
        languages.sort_by_key(|lang| lang.priority);
        languages
    }

    /// Find a language by name
    pub fn find_language(&self, name: &str) -> Option<&Arc<ProjectIndicator>> {
        self.languages
            .iter()
            .find(|lang| lang.name.eq_ignore_ascii_case(name))
    }

    /// Detect frameworks for a given language (optimized to group by detection type)
    fn detect_frameworks(
        &self,
        path: &Path,
        language: &Arc<ProjectIndicator>,
    ) -> Result<Vec<FrameworkMatch>> {
        if language.frameworks.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_matches = Vec::new();

        // Group frameworks by detection type to avoid repeated file operations
        let mut package_json_frameworks = Vec::new();
        let mut cargo_toml_frameworks = Vec::new();
        let mut go_mod_frameworks = Vec::new();
        let mut pyproject_toml_frameworks = Vec::new();
        let mut gemspec_frameworks = Vec::new();
        let mut composer_json_frameworks = Vec::new();
        let mut file_exists_frameworks = Vec::new();
        let mut config_file_frameworks = Vec::new();

        // Group frameworks by their detection type
        for framework in &language.frameworks {
            match &framework.detection {
                DetectionType::PackageJson { .. } => package_json_frameworks.push(framework),
                DetectionType::CargoToml { .. } => cargo_toml_frameworks.push(framework),
                DetectionType::GoMod { .. } => go_mod_frameworks.push(framework),
                DetectionType::PyProjectToml { .. } => pyproject_toml_frameworks.push(framework),
                DetectionType::GemSpec { .. } => gemspec_frameworks.push(framework),
                DetectionType::ComposerJson { .. } => composer_json_frameworks.push(framework),
                DetectionType::FileExists { .. } => file_exists_frameworks.push(framework),
                DetectionType::ConfigFile { .. } => config_file_frameworks.push(framework),
            }
        }

        // Process each group once (much more efficient than per-framework)
        if !package_json_frameworks.is_empty() {
            let frameworks_slice: Vec<_> = package_json_frameworks
                .iter()
                .map(|f| (*f).clone())
                .collect();
            let mut matches = PackageJsonMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }
        if !cargo_toml_frameworks.is_empty() {
            let frameworks_slice: Vec<_> =
                cargo_toml_frameworks.iter().map(|f| (*f).clone()).collect();
            let mut matches = CargoTomlMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }
        if !go_mod_frameworks.is_empty() {
            let frameworks_slice: Vec<_> = go_mod_frameworks.iter().map(|f| (*f).clone()).collect();
            let mut matches = GoModMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }
        if !pyproject_toml_frameworks.is_empty() {
            let frameworks_slice: Vec<_> = pyproject_toml_frameworks
                .iter()
                .map(|f| (*f).clone())
                .collect();
            let mut matches = PyProjectTomlMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }
        if !gemspec_frameworks.is_empty() {
            let frameworks_slice: Vec<_> =
                gemspec_frameworks.iter().map(|f| (*f).clone()).collect();
            let mut matches = GemfileMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }
        if !composer_json_frameworks.is_empty() {
            let frameworks_slice: Vec<_> = composer_json_frameworks
                .iter()
                .map(|f| (*f).clone())
                .collect();
            let mut matches = ComposerJsonMatcher::detect_frameworks(path, &frameworks_slice)?;
            all_matches.append(&mut matches);
        }

        // Handle file exists frameworks
        for framework in file_exists_frameworks {
            if let DetectionType::FileExists { files } = &framework.detection {
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
        }

        // Handle config file frameworks
        for framework in config_file_frameworks {
            if let DetectionType::ConfigFile { file, keys } = &framework.detection {
                if let Some(confidence) = self.check_config_file(path, file, keys)? {
                    all_matches.push(FrameworkMatch::new(
                        framework.clone(),
                        confidence,
                        vec![file.clone()],
                    ));
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
        // Populate cache dynamic relevant set for this run
        cache.clear_dynamic_relevant();
        // Language file patterns (use base names when applicable)
        for lang in &self.languages {
            for pat in &lang.files {
                // Only add simple filenames (skip wildcards to avoid explosion)
                if !pat.contains('*') {
                    cache.add_dynamic_relevant(pat.clone());
                }
            }
            // Framework config files that are single-file based
            for fw in &lang.frameworks {
                match &fw.detection {
                    DetectionType::PackageJson { .. } => cache.add_dynamic_relevant("package.json"),
                    DetectionType::CargoToml { .. } => cache.add_dynamic_relevant("Cargo.toml"),
                    DetectionType::GoMod { .. } => cache.add_dynamic_relevant("go.mod"),
                    DetectionType::PyProjectToml { .. } => {
                        cache.add_dynamic_relevant("pyproject.toml")
                    }
                    DetectionType::ComposerJson { .. } => {
                        cache.add_dynamic_relevant("composer.json")
                    }
                    DetectionType::GemSpec { .. } => cache.add_dynamic_relevant("Gemfile"),
                    DetectionType::FileExists { files } => {
                        for f in files {
                            cache.add_dynamic_relevant(f.clone());
                        }
                    }
                    DetectionType::ConfigFile { file, .. } => {
                        cache.add_dynamic_relevant(file.clone())
                    }
                }
            }
        }
        // Root indicators
        for ind in self.root_discovery.root_indicators() {
            cache.add_dynamic_relevant(ind.pattern.clone());
        }

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
        let temp_project = create_test_project(&["some_random_file.txt", "another_file.log"]);

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

    #[test]
    fn test_project_root_discovery() {
        use crate::types::RootIndicator;

        // Create engine with explicit root indicators
        let detection_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![
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
        let engine = DetectionEngine::with_config(vec![], detection_config);

        // Create a nested project structure
        let temp_project = create_test_project(&["Cargo.toml", "src/main.rs"]);
        let src_dir = temp_project.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        // Test from src directory - should find project root
        let root = engine.root_discovery.find_project_root(&src_dir);
        assert!(root.is_some());
        // Use canonicalized paths for comparison to handle symlinks
        let canonical_root = root.unwrap().canonicalize().unwrap();
        let canonical_project = temp_project.path().canonicalize().unwrap();
        assert_eq!(canonical_root, canonical_project);

        // Test from project root - should find itself
        let root = engine.root_discovery.find_project_root(temp_project.path());
        assert!(root.is_some());
        let canonical_root = root.unwrap().canonicalize().unwrap();
        assert_eq!(canonical_root, canonical_project);

        // Test from non-project directory
        let temp_empty = TempDir::new().unwrap();
        let root = engine.root_discovery.find_project_root(temp_empty.path());
        assert!(root.is_none());
    }

    #[test]
    fn test_root_confidence_calculation() {
        use crate::types::RootIndicator;

        // Create engine with explicit root indicators
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
                    pattern: "README.md".to_string(),
                    weight: 0.2,
                },
            ],
        };
        let engine = DetectionEngine::with_config(vec![], detection_config);

        // Create project with multiple indicators
        let temp_project = create_test_project(&["Cargo.toml", ".git/config", "README.md"]);
        fs::create_dir_all(temp_project.path().join(".git")).unwrap();
        fs::write(temp_project.path().join(".git/config"), "").unwrap();
        fs::write(temp_project.path().join("README.md"), "# Test Project").unwrap();

        let confidence = engine
            .root_discovery
            .calculate_root_confidence(temp_project.path());
        assert!(confidence >= 1.0); // Should have high confidence due to multiple indicators (capped at 1.0)

        // Test with just .git (highest confidence)
        let temp_git = TempDir::new().unwrap();
        fs::create_dir_all(temp_git.path().join(".git")).unwrap();
        let git_confidence = engine
            .root_discovery
            .calculate_root_confidence(temp_git.path());
        assert_eq!(git_confidence, 1.0);

        // Test with no indicators
        let temp_empty = TempDir::new().unwrap();
        let empty_confidence = engine
            .root_discovery
            .calculate_root_confidence(temp_empty.path());
        assert_eq!(empty_confidence, 0.0);
    }

    #[test]
    fn test_detection_with_root_discovery() {
        use crate::types::RootIndicator;

        let rust_lang = create_test_language("Rust", vec!["Cargo.toml"], 1);
        let detection_config = DetectionConfig {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 1.0,
            }],
        };
        let engine = DetectionEngine::with_config(vec![rust_lang.clone()], detection_config);

        // Create a project with nested structure
        let temp_project = create_test_project(&["Cargo.toml"]);
        let deep_dir = temp_project.path().join("src/components");
        fs::create_dir_all(&deep_dir).unwrap();

        // Test detection from deep directory with root discovery enabled (indicators present)
        let result = engine.detect(&deep_dir).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result.language.as_ref().unwrap().name, "Rust");

        // Test detection from deep directory with root discovery effectively disabled (no indicators)
        let disabled_engine = DetectionEngine::with_config(
            vec![rust_lang],
            DetectionConfig {
                max_upward_traversal: 10,
                require_vcs_root: false,
                confidence_threshold: 0.3,
                root_indicators: vec![],
            },
        );
        let result_disabled = disabled_engine.detect(&deep_dir).unwrap();
        assert!(result_disabled.is_empty()); // Should not find anything without root discovery
    }
}
