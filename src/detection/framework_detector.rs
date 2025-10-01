use crate::detection::caches::ParsedFileCache;
use crate::detection::matchers::DependencyMatcher;
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionEvidence, DetectionType, EvidenceItem, FrameworkMatch, ProjectIndicator,
};
use crate::Result;
use anyhow::Context;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

pub struct FrameworkDetector;

impl FrameworkDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect_frameworks_with_evidence(
        &self,
        path: &Path,
        language: &Arc<ProjectIndicator>,
        evidence: &mut DetectionEvidence,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        let frameworks = self.detect_frameworks(path, language, file_cache, parsed_cache)?;

        for framework in &frameworks {
            for evidence_file in &framework.evidence {
                evidence.add_framework_evidence(EvidenceItem::framework_dependency(
                    evidence_file.clone(),
                    &framework.framework.name,
                    framework.confidence,
                ));
            }
        }

        Ok(frameworks)
    }

    pub fn detect_frameworks(
        &self,
        path: &Path,
        language: &Arc<ProjectIndicator>,
        file_cache: &FileSystemCache,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        if language.frameworks.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_matches = Vec::new();

        let mut file_exists_frameworks = Vec::new();
        let mut config_file_frameworks = Vec::new();
        let mut dependency_frameworks = Vec::new();

        for framework in &language.frameworks {
            match &framework.detection {
                DetectionType::NodeEcosystem { .. }
                | DetectionType::RustEcosystem { .. }
                | DetectionType::PythonEcosystem { .. }
                | DetectionType::PHPEcosystem { .. }
                | DetectionType::RubyEcosystem { .. }
                | DetectionType::GoEcosystem { .. }
                | DetectionType::JavaEcosystem { .. }
                | DetectionType::DotNetEcosystem { .. }
                | DetectionType::ScalaEcosystem { .. }
                | DetectionType::DartEcosystem { .. }
                | DetectionType::LuaEcosystem { .. } => {
                    dependency_frameworks.push(framework.clone());
                }
                DetectionType::FileExists { .. } => file_exists_frameworks.push(framework),
                DetectionType::ConfigFile { .. } => config_file_frameworks.push(framework),
            }
        }

        if !dependency_frameworks.is_empty() {
            let mut dependency_matches =
                DependencyMatcher::detect_frameworks(path, &dependency_frameworks, parsed_cache)?;
            all_matches.append(&mut dependency_matches);
        }

        for framework in file_exists_frameworks {
            if let DetectionType::FileExists { files } = &framework.detection {
                if self.check_file_exists(path, files, file_cache) {
                    let evidence: Vec<String> = files
                        .iter()
                        .filter(|file| file_cache.exists(&path.join(file)))
                        .cloned()
                        .collect();

                    if !evidence.is_empty() {
                        all_matches.push(FrameworkMatch::new(framework.clone(), 0.8, evidence));
                    }
                }
            }
        }

        for framework in config_file_frameworks {
            if let DetectionType::ConfigFile { file, keys } = &framework.detection {
                if let Some(confidence) = self.check_config_file(path, file, keys, file_cache)? {
                    all_matches.push(FrameworkMatch::new(
                        framework.clone(),
                        confidence,
                        vec![file.clone()],
                    ));
                }
            }
        }

        self.resolve_framework_matches(all_matches)
    }

    fn check_file_exists(
        &self,
        base_path: &Path,
        files: &[String],
        file_cache: &FileSystemCache,
    ) -> bool {
        files.iter().any(|file| {
            let file_path = base_path.join(file);
            file_cache.exists(&file_path)
        })
    }

    fn check_config_file(
        &self,
        base_path: &Path,
        file: &str,
        keys: &[String],
        file_cache: &FileSystemCache,
    ) -> Result<Option<f32>> {
        let config_path = base_path.join(file);

        if !file_cache.exists(&config_path) {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

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
                    return Ok(self.check_text_keys(&content, keys));
                }
            }
        } else {
            return Ok(self.check_text_keys(&content, keys));
        }

        Ok(None)
    }

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

    fn check_text_keys(&self, content: &str, keys: &[String]) -> Option<f32> {
        let mut found_keys = 0;
        for key in keys {
            if content.contains(key) {
                found_keys += 1;
            }
        }

        if found_keys > 0 {
            let confidence = (found_keys as f32 / keys.len() as f32) * 0.7;
            Some(confidence)
        } else {
            None
        }
    }

    fn resolve_framework_matches(
        &self,
        mut matches: Vec<FrameworkMatch>,
    ) -> Result<Vec<FrameworkMatch>> {
        if matches.is_empty() {
            return Ok(matches);
        }

        matches.sort_by(|a, b| {
            a.framework.priority.cmp(&b.framework.priority).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        let mut seen_names = HashSet::new();
        matches.retain(|m| seen_names.insert(m.framework.name.clone()));

        Ok(matches)
    }
}

impl Default for FrameworkDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::FileSystemCache;
    use crate::types::{DetectionType, FrameworkDetector as FwDetector};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_language_with_frameworks(
        name: &str,
        frameworks: Vec<FwDetector>,
    ) -> Arc<ProjectIndicator> {
        Arc::new(ProjectIndicator::new(
            name.to_owned(),
            vec!["*.rs".to_owned()],
            "#FF0000".to_owned(),
            "🔥".to_owned(),
            1,
            frameworks,
        ))
    }

    fn create_test_directory_with_files(
        files: &[(&str, &str)],
    ) -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        for (filename, content) in files {
            if filename.contains('/') {
                if let Some(parent) = std::path::Path::new(filename).parent() {
                    fs::create_dir_all(root.join(parent))?;
                }
            }
            fs::write(root.join(filename), content)?;
        }

        Ok(temp_dir)
    }

    #[test]
    fn test_framework_detector_creation() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();

        let empty_language = Arc::new(ProjectIndicator::new(
            "Empty".to_owned(),
            vec![],
            "#000000".to_owned(),
            "🔥".to_owned(),
            1,
            vec![],
        ));

        let temp_dir = TempDir::new()?;
        let file_cache = FileSystemCache::new(300, 1000);
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let result = detector.detect_frameworks(
            temp_dir.path(),
            &empty_language,
            &file_cache,
            &parsed_cache,
        );

        assert!(result.is_ok());
        assert!(result?.is_empty());
        Ok(())
    }

    #[test]
    fn test_detect_frameworks_empty_language() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let language = Arc::new(ProjectIndicator::new(
            "Test".to_owned(),
            vec!["*.test".to_owned()],
            "#000000".to_owned(),
            "🔥".to_owned(),
            1,
            vec![],
        ));

        let temp_dir = TempDir::new()?;
        let file_cache = FileSystemCache::new(300, 1000);
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let frameworks =
            detector.detect_frameworks(temp_dir.path(), &language, &file_cache, &parsed_cache)?;

        assert!(
            frameworks.is_empty(),
            "Should return empty for language with no frameworks"
        );
        Ok(())
    }

    #[test]
    fn test_check_file_exists() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let temp_dir = create_test_directory_with_files(&[
            ("package.json", "{}"),
            ("src/main.js", "console.log('hello');"),
        ])?;

        let file_cache = FileSystemCache::new(300, 1000);
        assert!(detector.check_file_exists(
            temp_dir.path(),
            &["package.json".to_string()],
            &file_cache
        ));
        assert!(!detector.check_file_exists(
            temp_dir.path(),
            &["nonexistent.json".to_string()],
            &file_cache
        ));
        assert!(detector.check_file_exists(
            temp_dir.path(),
            &["package.json".to_string(), "nonexistent.json".to_string()],
            &file_cache
        ));
        Ok(())
    }

    #[test]
    fn test_check_toml_key() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let toml_content = r#"
[package]
name = "test"
version = "1.0.0"

[dependencies]
serde = "1.0"
"#;

        let value: toml::Value = toml::from_str(toml_content)?;

        assert!(detector.check_toml_key(&value, "package"));
        assert!(detector.check_toml_key(&value, "package.name"));
        assert!(detector.check_toml_key(&value, "dependencies"));
        assert!(detector.check_toml_key(&value, "dependencies.serde"));
        assert!(!detector.check_toml_key(&value, "nonexistent"));
        assert!(!detector.check_toml_key(&value, "package.nonexistent"));
        Ok(())
    }

    #[test]
    fn test_check_text_keys() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let content = "This is a test file with react and vue frameworks";

        let keys = vec!["react".to_string(), "vue".to_string()];
        let confidence = detector.check_text_keys(content, &keys);
        assert!(confidence.is_some());
        assert_eq!(confidence.ok_or("Failed to get confidence")?, 0.7);

        let partial_keys = vec!["react".to_string(), "angular".to_string()];
        let partial_confidence = detector.check_text_keys(content, &partial_keys);
        assert!(partial_confidence.is_some());
        assert_eq!(
            partial_confidence.ok_or("Failed to get partial confidence")?,
            0.35
        );

        let no_keys = vec!["angular".to_string(), "svelte".to_string()];
        let no_confidence = detector.check_text_keys(content, &no_keys);
        assert!(no_confidence.is_none());
        Ok(())
    }

    #[test]
    fn test_check_config_file_toml() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let temp_dir = create_test_directory_with_files(&[(
            "Cargo.toml",
            r#"
[package]
name = "test"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#,
        )])?;

        let keys = vec!["dependencies.serde".to_string()];
        let file_cache = FileSystemCache::new(300, 1000);
        let confidence =
            detector.check_config_file(temp_dir.path(), "Cargo.toml", &keys, &file_cache)?;
        assert!(confidence.is_some());
        assert!(confidence.ok_or("Failed to get confidence")? > 0.0);

        let bad_keys = vec!["nonexistent.key".to_string()];
        let no_confidence =
            detector.check_config_file(temp_dir.path(), "Cargo.toml", &bad_keys, &file_cache)?;
        assert!(no_confidence.is_none());
        Ok(())
    }

    #[test]
    fn test_check_config_file_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let temp_dir = TempDir::new()?;

        let keys = vec!["any.key".to_string()];
        let file_cache = FileSystemCache::new(300, 1000);
        let confidence =
            detector.check_config_file(temp_dir.path(), "nonexistent.toml", &keys, &file_cache)?;
        assert!(confidence.is_none());
        Ok(())
    }

    #[test]
    fn test_resolve_framework_matches() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();

        let framework1 = FwDetector {
            name: "React".to_owned(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 2,
            files: vec![],
            root_indicators: vec![],
        };

        let framework2 = FwDetector {
            name: "Vue".to_owned(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let matches = vec![
            FrameworkMatch::new(framework1, 0.8, vec!["package.json".to_owned()]),
            FrameworkMatch::new(framework2, 0.7, vec!["package.json".to_owned()]),
        ];

        let resolved = detector.resolve_framework_matches(matches)?;

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].framework.name, "Vue");
        assert_eq!(resolved[1].framework.name, "React");
        Ok(())
    }

    #[test]
    fn test_resolve_framework_matches_deduplication() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();

        let framework = FwDetector {
            name: "React".to_owned(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let matches = vec![
            FrameworkMatch::new(framework.clone(), 0.8, vec!["package.json".to_owned()]),
            FrameworkMatch::new(framework, 0.9, vec!["package.json".to_owned()]),
        ];

        let resolved = detector.resolve_framework_matches(matches)?;

        assert_eq!(resolved.len(), 1, "Should deduplicate by framework name");
        assert_eq!(
            resolved[0].confidence, 0.9,
            "Should keep higher confidence match"
        );
        Ok(())
    }

    #[test]
    fn test_detect_frameworks_with_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();
        let language = create_test_language_with_frameworks("Test", vec![]);

        let temp_dir = TempDir::new()?;
        let mut evidence = DetectionEvidence::new();

        let file_cache = FileSystemCache::new(300, 1000);
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let frameworks = detector.detect_frameworks_with_evidence(
            temp_dir.path(),
            &language,
            &mut evidence,
            &file_cache,
            &parsed_cache,
        )?;

        assert!(frameworks.is_empty());
        Ok(())
    }
}
