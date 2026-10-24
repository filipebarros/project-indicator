use crate::detection::caches::ParsedFileCache;
use crate::detection::matchers::DependencyMatcher;
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionEvidence, DetectionType, EvidenceItem, Framework, FrameworkMatch, Indicator,
};
use crate::Result;
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
        indicator: &Arc<Indicator>,
        catalog: &[Framework],
        evidence: &mut DetectionEvidence,
        file_cache: &Arc<FileSystemCache>,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        let frameworks =
            self.detect_frameworks(path, indicator, catalog, file_cache, parsed_cache)?;

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
        indicator: &Arc<Indicator>,
        catalog: &[Framework],
        file_cache: &Arc<FileSystemCache>,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        // Candidates: frameworks whose ecosystems intersect the indicator's
        let candidates: Vec<&Framework> = catalog
            .iter()
            .filter(|f| {
                f.ecosystems
                    .iter()
                    .any(|e| indicator.ecosystems.contains(e))
            })
            .collect();

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_matches = Vec::new();

        let mut file_exists_frameworks = Vec::with_capacity(candidates.len());
        let mut dependency_frameworks = Vec::with_capacity(candidates.len());

        for framework in candidates {
            match &framework.detection {
                DetectionType::Dependencies { .. } => {
                    dependency_frameworks.push(framework.clone());
                }
                DetectionType::FileExists { .. } => file_exists_frameworks.push(framework),
            }
        }

        let path_buf = path.to_path_buf();

        if !dependency_frameworks.is_empty() {
            let mut dependency_matches = DependencyMatcher::detect_frameworks(
                path,
                &dependency_frameworks,
                &indicator.ecosystems,
                parsed_cache,
            )?;
            all_matches.append(&mut dependency_matches);
        }

        if !file_exists_frameworks.is_empty() {
            let file_exists_matches: Vec<FrameworkMatch> = file_exists_frameworks
                .iter()
                .filter_map(|framework| {
                    if let DetectionType::FileExists { files } = &framework.detection {
                        if self.check_file_exists(&path_buf, files, file_cache) {
                            let evidence: Vec<String> = files
                                .iter()
                                .filter(|file| file_cache.exists(&path_buf.join(file)))
                                .cloned()
                                .collect();

                            if !evidence.is_empty() {
                                return Some(FrameworkMatch::new(
                                    (*framework).clone(),
                                    0.8,
                                    evidence,
                                ));
                            }
                        }
                    }
                    None
                })
                .collect();
            all_matches.extend(file_exists_matches);
        }

        self.resolve_framework_matches(all_matches)
    }

    fn check_file_exists(
        &self,
        base_path: &Path,
        files: &[String],
        file_cache: &Arc<FileSystemCache>,
    ) -> bool {
        files.iter().any(|file| {
            let file_path = base_path.join(file);
            file_cache.exists(&file_path)
        })
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
    use crate::types::{DetectionType, Framework as FwDetector};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_language_with_frameworks(
        name: &str,
        mut frameworks: Vec<FwDetector>,
    ) -> (Arc<Indicator>, Vec<FwDetector>) {
        for framework in &mut frameworks {
            if framework.ecosystems.is_empty() {
                framework.ecosystems = vec![crate::types::Ecosystem::Npm];
            }
        }
        let indicator = Arc::new(Indicator::new(
            name.to_owned(),
            vec!["*.rs".to_owned()],
            "#FF0000".to_owned(),
            "🔥".to_owned(),
            1,
            vec![crate::types::Ecosystem::Npm],
        ));
        (indicator, frameworks)
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

        let empty_language = Arc::new(Indicator::new(
            "Empty".to_owned(),
            vec![],
            "#000000".to_owned(),
            "🔥".to_owned(),
            1,
            vec![],
        ));

        let temp_dir = TempDir::new()?;
        let file_cache = Arc::new(FileSystemCache::new());
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let result = detector.detect_frameworks(
            temp_dir.path(),
            &empty_language,
            &[],
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
        let language = Arc::new(Indicator::new(
            "Test".to_owned(),
            vec!["*.test".to_owned()],
            "#000000".to_owned(),
            "🔥".to_owned(),
            1,
            vec![],
        ));

        let temp_dir = TempDir::new()?;
        let file_cache = Arc::new(FileSystemCache::new());
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let frameworks = detector.detect_frameworks(
            temp_dir.path(),
            &language,
            &[],
            &file_cache,
            &parsed_cache,
        )?;

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

        let file_cache = Arc::new(FileSystemCache::new());
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
    fn test_resolve_framework_matches() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FrameworkDetector::new();

        let framework1 = FwDetector {
            name: "React".to_owned(),
            ecosystems: vec![],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 2,
            root_indicators: vec![],
        };

        let framework2 = FwDetector {
            name: "Vue".to_owned(),
            ecosystems: vec![],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
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
            ecosystems: vec![],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
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
        let (language, catalog) = create_test_language_with_frameworks("Test", vec![]);

        let temp_dir = TempDir::new()?;
        let mut evidence = DetectionEvidence::new();

        let file_cache = Arc::new(FileSystemCache::new());
        let parsed_cache = crate::detection::caches::ParsedFileCache::new();
        let frameworks = detector.detect_frameworks_with_evidence(
            temp_dir.path(),
            &language,
            &catalog,
            &mut evidence,
            &file_cache,
            &parsed_cache,
        )?;

        assert!(frameworks.is_empty());
        Ok(())
    }
}
