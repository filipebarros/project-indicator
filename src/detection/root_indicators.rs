use crate::performance::FileSystemCache;
use crate::types::{
    DetectionConfig, DetectionMode, DetectionResult, FrameworkMatch, ProjectIndicator,
};
use crate::Result;
use std::path::Path;
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
}

impl RootIndicatorEngine {
    pub fn new(languages: Vec<ProjectIndicator>) -> Self {
        Self::with_config(languages, DetectionConfig::default())
    }

    pub fn with_config(languages: Vec<ProjectIndicator>, config: DetectionConfig) -> Self {
        let languages: Vec<Arc<ProjectIndicator>> = languages.into_iter().map(Arc::new).collect();
        Self { languages, config }
    }

    pub fn from_arc_languages(
        languages: Vec<Arc<ProjectIndicator>>,
        config: DetectionConfig,
    ) -> Self {
        Self { languages, config }
    }

    pub fn detect_with_early_termination(
        &self,
        path: &Path,
        file_cache: &FileSystemCache,
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

    fn check_secondary_ecosystems(
        &self,
        path: &Path,
        file_cache: &FileSystemCache,
        _result: &mut DetectionResult,
    ) -> Result<()> {
        let secondary_checks = [
            ("package.json", "JavaScript/TypeScript"),
            ("Cargo.toml", "Rust"),
            ("go.mod", "Go"),
            ("requirements.txt", "Python"),
            ("Gemfile", "Ruby"),
            ("composer.json", "PHP"),
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
        file_cache: &FileSystemCache,
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
        file_cache: &FileSystemCache,
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
            "Cargo.toml" => {
                if self.is_valid_cargo_toml(file_path)? {
                    Ok(0.95)
                } else {
                    Ok(0.1)
                }
            }
            "go.mod" => Ok(0.95),
            "pyproject.toml" => {
                if self.is_valid_python_project(file_path)? {
                    Ok(0.90)
                } else {
                    Ok(0.1)
                }
            }
            "tsconfig.json" => Ok(0.90),
            "package.json" => {
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
            "next.config.js" | "next.config.ts" => Ok(0.95),
            "vite.config.js" | "vite.config.ts" => Ok(0.90),
            "nuxt.config.js" | "nuxt.config.ts" => Ok(0.95),
            "svelte.config.js" => Ok(0.95),
            "angular.json" => Ok(0.95),
            "Rocket.toml" => Ok(0.95),
            "manage.py" => {
                if self.is_django_manage_py(file_path)? {
                    Ok(0.90)
                } else {
                    Ok(0.1)
                }
            }
            "requirements.txt" => {
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
        debug_assert!(!indicators.is_empty(), "indicators should not be empty");

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
            "Cargo.toml"
                | "go.mod"
                | "pyproject.toml"
                | "tsconfig.json"
                | "next.config.js"
                | "next.config.ts"
                | "angular.json"
                | "Rocket.toml"
                | "manage.py"
        )
    }

    fn get_pattern_specificity(&self, pattern: &str) -> u8 {
        match pattern {
            "Cargo.toml" | "go.mod" | "tsconfig.json" => 10,
            "pyproject.toml" => 8,
            "package.json" => 5,
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
            let framework_lower = framework_name.to_lowercase();
            match framework_lower.as_str() {
                "django" => Ok(content.to_lowercase().contains("django")),
                "flask" => Ok(content.to_lowercase().contains("flask")),
                "fastapi" => Ok(content.to_lowercase().contains("fastapi")),
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
    use crate::types::{DetectionConfig, DetectionMode};
    use std::fs;
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

        let file_cache = FileSystemCache::new(300, 1000);
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

        let file_cache = FileSystemCache::new(300, 1000);
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

        let file_cache = FileSystemCache::new(300, 1000);
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
}
