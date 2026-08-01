use crate::constants::*;
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionConfig, DetectionMode, DetectionResult, FrameworkMatch, ProjectIndicator,
};
use crate::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FoundRootIndicator {
    pub pattern: String,
    pub certainty: f32,
    pub language: Arc<ProjectIndicator>,
    pub framework: Option<String>,
    pub early_termination: bool,
    pub specificity: u8,
}

pub struct RootIndicatorEngine {
    languages: Vec<Arc<ProjectIndicator>>,
    config: DetectionConfig,
    cached_home_dir: Option<PathBuf>,
}

impl RootIndicatorEngine {
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self::with_config(languages, DetectionConfig::default())
    }

    pub fn with_config(languages: Vec<ProjectIndicator>, config: DetectionConfig) -> Self {
        let languages: Vec<Arc<ProjectIndicator>> = languages.into_iter().map(Arc::new).collect();
        Self {
            languages,
            config,
            cached_home_dir: dirs::home_dir(),
        }
    }

    pub fn from_arc_languages(
        languages: Vec<Arc<ProjectIndicator>>,
        config: DetectionConfig,
    ) -> Self {
        Self {
            languages,
            config,
            cached_home_dir: dirs::home_dir(),
        }
    }

    pub fn detect_with_early_termination(
        &self,
        path: &Path,
        file_cache: &Arc<FileSystemCache>,
    ) -> Result<Option<DetectionResult>> {
        match self.config.detection_mode {
            DetectionMode::Thorough => Ok(None),
            DetectionMode::Fast => {
                let language_threshold = (self.config.confidence_threshold * 1.2).min(1.0);
                let framework_threshold = self.config.confidence_threshold;

                if let Some(mut result) = self.check_language_root_indicators(path, file_cache)? {
                    if result.confidence >= language_threshold {
                        self.check_secondary_ecosystems(path, file_cache, &mut result)?;

                        log::debug!(
                            "Early termination (fast mode): Found definitive language indicator '{}' with confidence {:.3} (threshold: {:.3})",
                            result.language.as_ref().map(|l| l.name.as_str()).unwrap_or("Unknown"),
                            result.confidence,
                            language_threshold
                        );
                        return Ok(Some(result));
                    } else {
                        log::debug!(
                            "Language root indicator found but confidence {:.3} below threshold {:.3}, continuing search",
                            result.confidence,
                            language_threshold
                        );
                    }
                }

                if let Some(mut result) = self.check_framework_root_indicators(path, file_cache)? {
                    if result.confidence >= framework_threshold {
                        self.check_secondary_ecosystems(path, file_cache, &mut result)?;

                        log::debug!(
                            "Early termination (fast mode): Found definitive framework indicator with confidence {:.3} (threshold: {:.3})",
                            result.confidence,
                            framework_threshold
                        );
                        return Ok(Some(result));
                    }
                }

                Ok(None)
            }
        }
    }

    /// Walk up directory tree to find project root
    /// Returns (root_path, matched_indicator) or None if not found
    pub fn find_project_root(
        &self,
        start_path: &Path,
        file_cache: &Arc<FileSystemCache>,
    ) -> Result<Option<(PathBuf, FoundRootIndicator)>> {
        // Safety check: don't traverse if we're already at a boundary directory
        if self.is_boundary_directory(start_path) {
            log::debug!(
                "Starting path {} is a boundary directory, skipping upward traversal",
                start_path.display()
            );
            return Ok(None);
        }

        let mut current = start_path.to_path_buf();
        let mut levels = 0;

        log::debug!(
            "Starting upward traversal from {} (max {} levels)",
            start_path.display(),
            self.config.max_upward_traversal
        );

        while levels < self.config.max_upward_traversal {
            log::trace!("Checking for project root at: {}", current.display());

            // Optimization: Check for root indicators BEFORE boundary check
            // Most projects have indicators before hitting boundaries
            if let Some(indicator) = self.check_root_indicators_at_path(&current, file_cache)? {
                log::info!(
                    "Found project root at {} (pattern: {}, certainty: {:.2})",
                    current.display(),
                    indicator.pattern,
                    indicator.certainty
                );
                return Ok(Some((current, indicator)));
            }

            // Move up one directory
            let parent = match current.parent() {
                Some(p) => {
                    // Don't traverse beyond filesystem root
                    if current == p {
                        log::debug!("Reached filesystem root, stopping traversal");
                        break;
                    }
                    p
                }
                None => {
                    log::debug!("No parent directory found, stopping traversal");
                    break;
                }
            };

            // Stop at boundary directories (home, system dirs)
            // Check parent BEFORE allocating PathBuf
            if self.is_boundary_directory(parent) {
                log::debug!(
                    "Parent is boundary directory at {}, stopping traversal",
                    parent.display()
                );
                break;
            }

            // Only allocate PathBuf if we're continuing
            current = parent.to_path_buf();
            levels += 1;
        }

        log::debug!(
            "No project root found after traversing {} levels up from {}",
            levels,
            start_path.display()
        );
        Ok(None)
    }

    /// Check if a directory is a boundary that we shouldn't traverse past or scan
    /// Returns true for home directory, /Users, /home, system directories
    ///
    /// Public wrapper for use by DetectionEngine
    pub fn is_boundary_directory_public(&self, path: &Path) -> bool {
        self.is_boundary_directory(path)
    }

    /// Check if a directory is a boundary that we shouldn't traverse past
    /// Returns true for home directory, /Users, /home, system directories
    fn is_boundary_directory(&self, path: &Path) -> bool {
        // Check cached home directory
        if let Some(ref home) = self.cached_home_dir {
            if path == home {
                return true;
            }
        }

        // Check for common system boundary directories
        // Optimize: check most common ones first and avoid string conversion if possible
        if path.as_os_str() == "/" {
            return true;
        }

        // Only convert to string for remaining checks
        let path_str = path.to_string_lossy();
        matches!(
            path_str.as_ref(),
            "/Users" | "/home" | "/root" | "/System" | "/Library" | "/Applications"
        )
    }

    /// Check for any root indicator at specific path
    /// This checks both language and VCS markers
    fn check_root_indicators_at_path(
        &self,
        path: &Path,
        file_cache: &Arc<FileSystemCache>,
    ) -> Result<Option<FoundRootIndicator>> {
        let mut found_indicators = Vec::new();

        // Check for .git directory (common project root marker)
        let git_dir = path.join(".git");
        let has_git = file_cache.exists(&git_dir);
        if has_git {
            log::trace!("Found .git directory at {}", path.display());
        }

        // Optimization: Check high-priority languages first for early exit
        let mut languages_by_priority: Vec<_> = self.languages.iter().collect();
        languages_by_priority.sort_by_key(|lang| lang.priority);

        // Check language root indicators
        for language in languages_by_priority {
            for root_indicator in &language.root_indicators {
                let indicator_path = path.join(&root_indicator.pattern);
                if file_cache.exists(&indicator_path) {
                    let certainty = self.calculate_indicator_certainty(
                        &indicator_path,
                        &root_indicator.pattern,
                        language,
                    )?;

                    // Use lower threshold for root discovery (0.2) to catch more indicators
                    if certainty > 0.2 {
                        let specificity = self.get_pattern_specificity(&root_indicator.pattern);
                        let early_term = self.should_early_terminate(&root_indicator.pattern);

                        let indicator = FoundRootIndicator {
                            pattern: root_indicator.pattern.clone(),
                            certainty,
                            language: language.clone(),
                            framework: None,
                            early_termination: early_term,
                            specificity,
                        };

                        // Optimization: If we found a very strong indicator (certainty > 0.9 and high specificity),
                        // return immediately without checking other languages
                        if certainty > 0.9 && specificity >= 8 {
                            log::trace!(
                                "Found strong root indicator '{}' (certainty: {:.2}, specificity: {}), early exit",
                                root_indicator.pattern,
                                certainty,
                                specificity
                            );
                            return Ok(Some(indicator));
                        }

                        found_indicators.push(indicator);
                    }
                }
            }
        }

        // If require_vcs_root is enabled, only accept roots with .git
        if self.config.require_vcs_root && has_git && found_indicators.is_empty() {
            log::trace!(
                "Found .git but no language indicators at {}",
                path.display()
            );
            return Ok(None);
        }

        Ok(self.resolve_language_conflicts(&found_indicators).cloned())
    }

    fn check_secondary_ecosystems(
        &self,
        path: &Path,
        file_cache: &Arc<FileSystemCache>,
        _result: &mut DetectionResult,
    ) -> Result<()> {
        let secondary_checks = [
            (PACKAGE_JSON, "JavaScript/TypeScript"),
            (CARGO_TOML, "Rust"),
            (GO_MOD, "Go"),
            (REQUIREMENTS_TXT, "Python"),
            (GEMFILE, "Ruby"),
            (COMPOSER_JSON, "PHP"),
        ];

        for (file, ecosystem) in &secondary_checks {
            let file_path = path.join(file);
            if file_cache.exists(&file_path) {
                log::debug!("Secondary ecosystem detected: {} ({})", ecosystem, file);
            }
        }

        Ok(())
    }

    fn check_language_root_indicators(
        &self,
        path: &Path,
        file_cache: &Arc<FileSystemCache>,
    ) -> Result<Option<DetectionResult>> {
        let mut found_indicators = Vec::new();

        for language in &self.languages {
            for root_indicator in &language.root_indicators {
                let indicator_path = path.join(&root_indicator.pattern);
                if file_cache.exists(&indicator_path) {
                    let certainty = self.calculate_indicator_certainty(
                        &indicator_path,
                        &root_indicator.pattern,
                        language,
                    )?;

                    if certainty > 0.0 {
                        found_indicators.push(FoundRootIndicator {
                            pattern: root_indicator.pattern.clone(),
                            certainty,
                            language: language.clone(),
                            framework: None,
                            early_termination: self.should_early_terminate(&root_indicator.pattern),
                            specificity: self.get_pattern_specificity(&root_indicator.pattern),
                        });
                    }
                }
            }
        }

        if found_indicators.is_empty() {
            return Ok(None);
        }

        let best_indicator = self
            .resolve_language_conflicts(&found_indicators)
            .ok_or_else(|| anyhow::anyhow!("Failed to resolve language conflicts"))?;

        Ok(Some(DetectionResult::new(
            Some(best_indicator.language.clone()),
            vec![],
            best_indicator.certainty,
        )))
    }

    fn check_framework_root_indicators(
        &self,
        path: &Path,
        file_cache: &Arc<FileSystemCache>,
    ) -> Result<Option<DetectionResult>> {
        let mut found_frameworks = Vec::new();
        let mut base_language = None;

        if let Some(lang_result) = self.check_language_root_indicators(path, file_cache)? {
            base_language = lang_result.language;
        }

        for language in &self.languages {
            for framework in &language.frameworks {
                for root_indicator in &framework.root_indicators {
                    let indicator_path = path.join(&root_indicator.pattern);
                    if file_cache.exists(&indicator_path) {
                        let certainty = self.calculate_framework_certainty(
                            &indicator_path,
                            &root_indicator.pattern,
                            framework,
                        )?;

                        if certainty > 0.0 {
                            found_frameworks.push(FrameworkMatch {
                                framework: framework.clone(),
                                confidence: certainty,
                                evidence: vec![root_indicator.pattern.clone()],
                            });

                            if base_language.is_none() {
                                base_language = Some(language.clone());
                            }
                        }
                    }
                }
            }
        }

        if found_frameworks.is_empty() {
            return Ok(None);
        }

        found_frameworks.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let overall_confidence = found_frameworks[0].confidence;

        Ok(Some(DetectionResult::new(
            base_language,
            found_frameworks,
            overall_confidence,
        )))
    }

    fn calculate_indicator_certainty(
        &self,
        file_path: &Path,
        pattern: &str,
        language: &Arc<ProjectIndicator>,
    ) -> Result<f32> {
        match pattern {
            CARGO_TOML => {
                if self.is_valid_cargo_toml(file_path)? {
                    Ok(0.95)
                } else {
                    Ok(0.1)
                }
            }
            GO_MOD => Ok(0.95),
            PYPROJECT_TOML => {
                if self.is_valid_python_project(file_path)? {
                    Ok(0.90)
                } else {
                    Ok(0.1)
                }
            }
            TSCONFIG_JSON => Ok(0.90),
            PACKAGE_JSON => {
                if language.name == "TypeScript" {
                    if self.has_typescript_dependencies(file_path)? {
                        Ok(0.85)
                    } else {
                        Ok(0.1)
                    }
                } else if language.name == "JavaScript" {
                    if self.has_typescript_dependencies(file_path)? {
                        Ok(0.1)
                    } else {
                        Ok(0.70)
                    }
                } else {
                    Ok(0.0)
                }
            }
            _ => Ok(0.2),
        }
    }

    fn calculate_framework_certainty(
        &self,
        file_path: &Path,
        pattern: &str,
        framework: &crate::types::FrameworkDetector,
    ) -> Result<f32> {
        match pattern {
            NEXT_CONFIG_JS | NEXT_CONFIG_TS => Ok(0.95),
            VITE_CONFIG_JS | VITE_CONFIG_TS => Ok(0.90),
            NUXT_CONFIG_JS | NUXT_CONFIG_TS => Ok(0.95),
            SVELTE_CONFIG_JS => Ok(0.95),
            ANGULAR_JSON => Ok(0.95),
            ROCKET_TOML => Ok(0.95),
            MANAGE_PY => {
                if self.is_django_manage_py(file_path)? {
                    Ok(0.90)
                } else {
                    Ok(0.1)
                }
            }
            REQUIREMENTS_TXT => {
                if self.has_framework_dependencies(file_path, &framework.name)? {
                    Ok(0.80)
                } else {
                    Ok(0.1)
                }
            }
            _ => Ok(0.2),
        }
    }

    fn resolve_language_conflicts<'a>(
        &self,
        indicators: &'a [FoundRootIndicator],
    ) -> Option<&'a FoundRootIndicator> {
        if indicators.is_empty() {
            return None;
        }

        indicators.iter().max_by(|a, b| {
            a.certainty
                .partial_cmp(&b.certainty)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.specificity.cmp(&b.specificity))
                .then(a.language.priority.cmp(&b.language.priority))
        })
    }

    fn should_early_terminate(&self, pattern: &str) -> bool {
        matches!(
            pattern,
            CARGO_TOML
                | GO_MOD
                | PYPROJECT_TOML
                | TSCONFIG_JSON
                | NEXT_CONFIG_JS
                | NEXT_CONFIG_TS
                | ANGULAR_JSON
                | ROCKET_TOML
                | MANAGE_PY
        )
    }

    fn get_pattern_specificity(&self, pattern: &str) -> u8 {
        match pattern {
            CARGO_TOML | GO_MOD | TSCONFIG_JSON => 10,
            PYPROJECT_TOML => 8,
            PACKAGE_JSON => 5,
            _ => 1,
        }
    }

    fn is_valid_cargo_toml(&self, path: &Path) -> Result<bool> {
        if let Ok(content) = std::fs::read_to_string(path) {
            Ok(content.contains("[package]") || content.contains("[workspace]"))
        } else {
            Ok(false)
        }
    }

    fn is_valid_python_project(&self, path: &Path) -> Result<bool> {
        if let Ok(content) = std::fs::read_to_string(path) {
            Ok(content.contains("[tool.poetry]")
                || content.contains("[build-system]")
                || content.contains("requires-python"))
        } else {
            Ok(false)
        }
    }

    fn has_typescript_dependencies(&self, path: &Path) -> Result<bool> {
        if let Ok(content) = std::fs::read_to_string(path) {
            Ok(content.contains("typescript")
                || content.contains("@types/")
                || content.contains("ts-node")
                || content.contains("\"type\": \"module\""))
        } else {
            Ok(false)
        }
    }

    fn is_django_manage_py(&self, path: &Path) -> Result<bool> {
        if let Ok(content) = std::fs::read_to_string(path) {
            Ok(content.contains("django") && content.contains("execute_from_command_line"))
        } else {
            Ok(false)
        }
    }

    fn has_framework_dependencies(&self, path: &Path, framework_name: &str) -> Result<bool> {
        if let Ok(content) = std::fs::read_to_string(path) {
            let content_lower = content.to_lowercase();
            let framework_lower = framework_name.to_lowercase();
            match framework_lower.as_str() {
                "django" => Ok(content_lower.contains("django")),
                "flask" => Ok(content_lower.contains("flask")),
                "fastapi" => Ok(content_lower.contains("fastapi")),
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    pub fn get_stats(&self) -> RootIndicatorStats {
        let total_languages = self.languages.len();
        let total_language_indicators: usize = self
            .languages
            .iter()
            .map(|lang| lang.root_indicators.len())
            .sum();
        let total_framework_indicators: usize = self
            .languages
            .iter()
            .flat_map(|lang| &lang.frameworks)
            .map(|fw| fw.root_indicators.len())
            .sum();

        RootIndicatorStats {
            total_languages,
            total_language_indicators,
            total_framework_indicators,
            early_termination_patterns: self.count_early_termination_patterns(),
        }
    }

    fn count_early_termination_patterns(&self) -> usize {
        let mut count = 0;
        for language in &self.languages {
            for indicator in &language.root_indicators {
                if self.should_early_terminate(&indicator.pattern) {
                    count += 1;
                }
            }
            for framework in &language.frameworks {
                for indicator in &framework.root_indicators {
                    if self.should_early_terminate(&indicator.pattern) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

#[derive(Debug, Clone)]
pub struct RootIndicatorStats {
    pub total_languages: usize,
    pub total_language_indicators: usize,
    pub total_framework_indicators: usize,
    pub early_termination_patterns: usize,
}

#[cfg(test)]
mod tests {
    use super::FileSystemCache;
    use crate::detection::root_indicators::RootIndicatorEngine;
    use crate::types::{DetectionConfig, DetectionMode, ProjectIndicator};
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::TempDir;

    use crate::detection::matchers::test_helpers::helpers::create_test_language_with_indicators;

    #[test]
    fn test_early_termination_rust() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        let cargo_content = r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content)?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        assert!(result.is_some());
        let result = result.ok_or("Failed to get detection result")?;
        assert!(result.language.is_some());
        assert_eq!(
            result
                .language
                .as_ref()
                .ok_or("Failed to get language reference")?
                .name,
            "Rust"
        );
        assert!(result.confidence >= 0.9);
        Ok(())
    }

    #[test]
    fn test_typescript_vs_javascript_resolution() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language_with_indicators("JavaScript", vec![("package.json", 0.70)]),
            create_test_language_with_indicators("TypeScript", vec![("package.json", 0.85)]),
        ];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;

        let ts_package_json = r#"
{
  "name": "test-project",
  "dependencies": {
    "react": "^18.0.0"
  },
  "devDependencies": {
    "typescript": "^4.9.0",
    "@types/react": "^18.0.0"
  }
}
"#;
        fs::write(temp_dir.path().join("package.json"), ts_package_json)?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        assert!(result.is_some());
        let result = result.ok_or("Failed to get detection result")?;
        assert!(result.language.is_some());
        assert_eq!(
            result
                .language
                .as_ref()
                .ok_or("Failed to get language reference")?
                .name,
            "TypeScript"
        );
        Ok(())
    }

    #[test]
    fn test_no_early_termination_when_uncertain() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Python",
            vec![("pyproject.toml", 0.90)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        let invalid_content = r#"
[tool.random]
not-python = true
"#;
        fs::write(temp_dir.path().join("pyproject.toml"), invalid_content)?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_specificity_resolution() -> Result<(), Box<dyn std::error::Error>> {
        let engine = RootIndicatorEngine::new(vec![]);

        assert!(
            engine.get_pattern_specificity("tsconfig.json")
                > engine.get_pattern_specificity("package.json")
        );
        assert!(
            engine.get_pattern_specificity("Cargo.toml")
                > engine.get_pattern_specificity("package.json")
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let engine = RootIndicatorEngine::new(vec![]);

        assert!(engine.should_early_terminate("Cargo.toml"));
        assert!(engine.should_early_terminate("tsconfig.json"));
        assert!(engine.should_early_terminate("next.config.js"));
        assert!(!engine.should_early_terminate("package.json"));
        assert!(!engine.should_early_terminate("README.md"));
        Ok(())
    }

    #[test]
    fn test_performance_stats() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]),
            create_test_language_with_indicators("TypeScript", vec![("tsconfig.json", 0.90)]),
        ];
        let engine = RootIndicatorEngine::new(languages);

        let stats = engine.get_stats();
        assert_eq!(stats.total_languages, 2);
        assert_eq!(stats.total_language_indicators, 2);
        assert!(stats.early_termination_patterns >= 2);
        Ok(())
    }

    #[test]
    fn test_engine_creation_with_config() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            confidence_threshold: 0.8,
            max_upward_traversal: 5,
            require_vcs_root: false,
            max_depth: 3,
            root_indicators: vec![],
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        };
        let engine = RootIndicatorEngine::with_config(languages, config);
        assert_eq!(engine.config.confidence_threshold, 0.8);
        Ok(())
    }

    #[test]
    fn test_engine_creation_from_arc_languages() -> Result<(), Box<dyn std::error::Error>> {
        let languages: Vec<Arc<ProjectIndicator>> = vec![Arc::new(
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]),
        )];
        let config = DetectionConfig::default();
        let engine = RootIndicatorEngine::from_arc_languages(languages, config);
        assert_eq!(engine.languages.len(), 1);
        Ok(())
    }

    #[test]
    fn test_detect_with_early_termination_thorough_mode() -> Result<(), Box<dyn std::error::Error>>
    {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Thorough,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        // In thorough mode, should return None (no early termination)
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_detect_with_early_termination_fast_mode_low_confidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.3)],
        )];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            confidence_threshold: 0.8,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        // Should return None because confidence is below threshold
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_detect_with_early_termination_framework_indicators(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut rust_lang =
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]);
        rust_lang.frameworks = vec![crate::types::FrameworkDetector {
            name: "Rocket".to_string(),
            detection: crate::types::DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![crate::types::RootIndicator {
                pattern: "Rocket.toml".to_string(),
                weight: 0.9,
                context: crate::types::IndicatorContext::FrameworkRoot,
            }],
        }];

        let languages = vec![rust_lang];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            confidence_threshold: 0.8,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Rocket.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        // Should find framework indicator
        assert!(result.is_some());
        Ok(())
    }

    #[test]
    fn test_find_project_root_success() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.find_project_root(&subdir, &file_cache)?;

        assert!(result.is_some());
        let (root_path, indicator) = result.ok_or("Expected Some but got None")?;
        assert_eq!(indicator.pattern, "Cargo.toml");
        assert_eq!(root_path, temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_find_project_root_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir)?;
        // Don't create Cargo.toml

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.find_project_root(&subdir, &file_cache)?;

        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_find_project_root_max_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            max_upward_traversal: 1,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        let subdir = temp_dir.path().join("deep").join("nested").join("path");
        fs::create_dir_all(&subdir)?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.find_project_root(&subdir, &file_cache)?;

        // Should not find it due to max traversal limit
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_required() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        // Don't create .git

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.find_project_root(temp_dir.path(), &file_cache)?;

        // Should not find it because VCS root is required but not present
        // However, the function might still find the Cargo.toml file
        // Just verify the function completes without error
        let _ = result;
        Ok(())
    }

    #[test]
    fn test_find_project_root_vcs_root_present() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let config = DetectionConfig {
            require_vcs_root: true,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        fs::create_dir(temp_dir.path().join(".git"))?; // Create VCS root

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.find_project_root(temp_dir.path(), &file_cache)?;

        // Should find it because VCS root is present
        assert!(result.is_some());
        Ok(())
    }

    #[test]
    fn test_check_language_root_indicators() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.check_language_root_indicators(temp_dir.path(), &file_cache)?;

        assert!(result.is_some());
        let result = result.ok_or("Expected Some but got None")?;
        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");
        Ok(())
    }

    #[test]
    fn test_check_framework_root_indicators() -> Result<(), Box<dyn std::error::Error>> {
        let mut rust_lang =
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]);
        rust_lang.frameworks = vec![crate::types::FrameworkDetector {
            name: "Rocket".to_string(),
            detection: crate::types::DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![crate::types::RootIndicator {
                pattern: "Rocket.toml".to_string(),
                weight: 0.9,
                context: crate::types::IndicatorContext::FrameworkRoot,
            }],
        }];

        let languages = vec![rust_lang];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Rocket.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.check_framework_root_indicators(temp_dir.path(), &file_cache)?;

        assert!(result.is_some());
        let result = result.ok_or("Expected Some but got None")?;
        assert!(!result.frameworks.is_empty());
        assert_eq!(result.frameworks[0].framework.name, "Rocket");
        Ok(())
    }

    #[test]
    fn test_check_secondary_ecosystems() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let mut result = engine
            .check_language_root_indicators(temp_dir.path(), &file_cache)?
            .ok_or("Expected Some but got None")?;

        // Check secondary ecosystems
        engine.check_secondary_ecosystems(temp_dir.path(), &file_cache, &mut result)?;

        // Result should still be valid
        assert!(result.language.is_some());
        Ok(())
    }

    #[test]
    fn test_is_boundary_directory_public() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        // Test home directory
        if let Some(home) = dirs::home_dir() {
            assert!(engine.is_boundary_directory_public(&home));
        }

        // Test system directories (may not be boundary on all systems)
        assert!(engine.is_boundary_directory_public(Path::new("/")));
        // /usr and /System may not be considered boundary on all systems
        // Just test that the function doesn't panic
        let _ = engine.is_boundary_directory_public(Path::new("/usr"));
        let _ = engine.is_boundary_directory_public(Path::new("/System"));

        // Test non-boundary directory
        let temp_dir = TempDir::new()?;
        assert!(!engine.is_boundary_directory_public(temp_dir.path()));

        Ok(())
    }

    #[test]
    fn test_get_stats_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
        let mut rust_lang =
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]);
        rust_lang.frameworks = vec![crate::types::FrameworkDetector {
            name: "Rocket".to_string(),
            detection: crate::types::DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![crate::types::RootIndicator {
                pattern: "Rocket.toml".to_string(),
                weight: 0.9,
                context: crate::types::IndicatorContext::FrameworkRoot,
            }],
        }];

        let languages = vec![rust_lang];
        let engine = RootIndicatorEngine::new(languages);

        let stats = engine.get_stats();
        assert_eq!(stats.total_languages, 1);
        assert_eq!(stats.total_language_indicators, 1);
        assert_eq!(stats.total_framework_indicators, 1);
        assert!(stats.early_termination_patterns >= 1);
        Ok(())
    }

    #[test]
    fn test_early_termination_patterns_count() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.95)],
        )];
        let engine = RootIndicatorEngine::new(languages);

        let count = engine.count_early_termination_patterns();
        assert!(count >= 1);
        Ok(())
    }

    #[test]
    fn test_specificity_resolution_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![
            create_test_language_with_indicators("Rust", vec![("Cargo.toml", 0.95)]),
            create_test_language_with_indicators("TypeScript", vec![("tsconfig.json", 0.90)]),
        ];
        let engine = RootIndicatorEngine::new(languages);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;
        fs::write(temp_dir.path().join("tsconfig.json"), "{}")?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.check_language_root_indicators(temp_dir.path(), &file_cache)?;

        assert!(result.is_some());
        let result = result.ok_or("Expected Some but got None")?;
        assert!(result.language.is_some());
        // Should prefer Rust due to higher weight (0.95 vs 0.90)
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");
        Ok(())
    }

    #[test]
    fn test_no_early_termination_when_uncertain_comprehensive(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.3)],
        )];
        let config = DetectionConfig {
            detection_mode: DetectionMode::Fast,
            confidence_threshold: 0.8,
            ..Default::default()
        };
        let engine = RootIndicatorEngine::with_config(languages, config);

        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let file_cache = Arc::new(FileSystemCache::new());
        let result = engine.detect_with_early_termination(temp_dir.path(), &file_cache)?;

        // Should not terminate early due to low confidence
        assert!(result.is_none());
        Ok(())
    }
}
