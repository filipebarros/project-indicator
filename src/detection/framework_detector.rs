use crate::detection::caches::ParsedFileCache;
use crate::detection::matchers::dependency_matcher;
use crate::detection::pattern_processor::join_components;
use crate::performance::FileSystemCache;
use crate::types::{
    DetectionEvidence, DetectionType, EvidenceItem, Framework, FrameworkMatch, Indicator,
};
use std::collections::HashSet;
use std::path::Path;

/// Confidence of a framework detected only by the presence of its files
const FILE_EXISTS_CONFIDENCE: f32 = 0.8;

pub fn detect_frameworks(
    path: &Path,
    indicator: &Indicator,
    catalog: &[Framework],
    evidence: &mut DetectionEvidence,
    file_cache: &FileSystemCache,
    parsed_cache: &ParsedFileCache,
) -> Vec<FrameworkMatch> {
    let (frameworks, watched) = detect(path, indicator, catalog, file_cache, parsed_cache);
    evidence.watched_dirs.extend(watched);
    evidence.watched_dirs.sort();
    evidence.watched_dirs.dedup();

    for framework in &frameworks {
        for evidence_file in &framework.evidence {
            evidence.add_framework_evidence(EvidenceItem::framework_dependency(
                evidence_file.clone(),
                &framework.framework.name,
                framework.confidence,
            ));
        }
    }

    frameworks
}

/// Frameworks whose ecosystems intersect the indicator's, detected by
/// dependency or by file presence, best first and unique by name; plus the
/// directories (relative to `path`) the file probes depended on: creating a
/// probed file changes its nearest existing parent's mtime, which is how the
/// persistent cache notices a framework appearing.
fn detect(
    path: &Path,
    indicator: &Indicator,
    catalog: &[Framework],
    file_cache: &FileSystemCache,
    parsed_cache: &ParsedFileCache,
) -> (Vec<FrameworkMatch>, Vec<String>) {
    let (dependency_frameworks, file_frameworks): (Vec<&Framework>, Vec<&Framework>) = catalog
        .iter()
        .filter(|f| {
            f.ecosystems
                .iter()
                .any(|e| indicator.ecosystems.contains(e))
        })
        .partition(|f| matches!(f.detection, DetectionType::Dependencies(..)));

    let mut matches = dependency_matcher::detect_frameworks(
        path,
        dependency_frameworks,
        &indicator.ecosystems,
        parsed_cache,
    );

    let mut watched = Vec::new();
    for framework in file_frameworks {
        if let DetectionType::FileExists(files) = &framework.detection {
            watched.extend(
                files
                    .iter()
                    .filter_map(|file| nearest_existing_parent(path, file, file_cache)),
            );
            let present: Vec<String> = files
                .iter()
                .filter(|file| file_cache.exists(&path.join(file)))
                .cloned()
                .collect();
            if !present.is_empty() {
                matches.push(FrameworkMatch::new(
                    framework.clone(),
                    FILE_EXISTS_CONFIDENCE,
                    present,
                ));
            }
        }
    }

    (resolve_framework_matches(matches), watched)
}

/// The deepest existing directory above `relative` (below `root`), `/`-joined;
/// `None` when that is `root` itself, which is always watched
fn nearest_existing_parent(
    root: &Path,
    relative: &str,
    file_cache: &FileSystemCache,
) -> Option<String> {
    let mut candidate = Path::new(relative);
    while let Some(parent) = candidate.parent() {
        if parent.as_os_str().is_empty() {
            return None;
        }
        if file_cache.exists(&root.join(parent)) {
            return join_components(parent);
        }
        candidate = parent;
    }
    None
}

/// Lowest priority number first, then highest confidence; one match per name
fn resolve_framework_matches(mut matches: Vec<FrameworkMatch>) -> Vec<FrameworkMatch> {
    matches.sort_by(|a, b| {
        (a.framework.priority.cmp(&b.framework.priority))
            .then(b.confidence.total_cmp(&a.confidence))
    });

    let mut seen_names = HashSet::new();
    matches.retain(|m| seen_names.insert(m.framework.name.clone()));
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Ecosystem;
    use crate::Result;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn framework(name: &str, priority: u8, detection: DetectionType) -> Framework {
        Framework {
            name: name.to_owned(),
            ecosystems: vec![Ecosystem::Npm],
            detection,
            icon: None,
            color: None,
            priority,
        }
    }

    fn npm_indicator(ecosystems: Vec<Ecosystem>) -> Arc<Indicator> {
        Arc::new(Indicator::new(
            "JS".to_owned(),
            vec!["package.json".to_owned()],
            "#000000".to_owned(),
            "J".to_owned(),
            ecosystems,
        ))
    }

    fn detect(dir: &TempDir, indicator: &Indicator, catalog: &[Framework]) -> Vec<FrameworkMatch> {
        super::detect(
            dir.path(),
            indicator,
            catalog,
            &FileSystemCache::default(),
            &ParsedFileCache::new(),
        )
        .0
    }

    #[test]
    fn test_file_exists_framework_reports_present_files_only() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("next.config.js"), "")?;
        let catalog = [framework(
            "Next.js",
            1,
            DetectionType::FileExists(vec!["next.config.js".into(), "next.config.mjs".into()]),
        )];

        let matches = detect(&dir, &npm_indicator(vec![Ecosystem::Npm]), &catalog);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].evidence, vec!["next.config.js"]);
        assert_eq!(matches[0].confidence, FILE_EXISTS_CONFIDENCE);
        Ok(())
    }

    #[test]
    fn test_framework_outside_the_indicators_ecosystems_is_ignored() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("next.config.js"), "")?;
        let catalog = [framework(
            "Next.js",
            1,
            DetectionType::FileExists(vec!["next.config.js".into()]),
        )];

        assert!(detect(&dir, &npm_indicator(vec![Ecosystem::Cargo]), &catalog).is_empty());
        Ok(())
    }

    #[test]
    fn test_nearest_existing_parent_walks_up_to_the_first_directory_that_exists() -> Result<()> {
        let dir = TempDir::new()?;
        fs::create_dir_all(dir.path().join("a/b"))?;
        let cache = FileSystemCache::default();
        let parent = |relative| nearest_existing_parent(dir.path(), relative, &cache);

        assert_eq!(parent("a/b/c/x.json"), Some("a/b".to_string()));
        assert_eq!(parent("a/x.json"), Some("a".to_string()));
        // directly in the root, or under a directory that does not exist yet:
        // the root is always watched
        assert_eq!(parent("x.json"), None);
        assert_eq!(parent("missing/x.json"), None);
        Ok(())
    }

    #[test]
    fn test_resolve_sorts_by_priority_then_confidence() {
        let by_files = || DetectionType::FileExists(vec![]);
        let resolved = resolve_framework_matches(vec![
            FrameworkMatch::new(framework("React", 2, by_files()), 0.8, vec![]),
            FrameworkMatch::new(framework("Vue", 1, by_files()), 0.7, vec![]),
        ]);

        let names: Vec<&str> = resolved.iter().map(|m| m.framework.name.as_str()).collect();
        assert_eq!(names, ["Vue", "React"]);
    }

    #[test]
    fn test_resolve_keeps_the_higher_confidence_duplicate() {
        let react = framework("React", 1, DetectionType::FileExists(vec![]));
        let resolved = resolve_framework_matches(vec![
            FrameworkMatch::new(react.clone(), 0.8, vec![]),
            FrameworkMatch::new(react, 0.9, vec![]),
        ]);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].confidence, 0.9);
    }
}
