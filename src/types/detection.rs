//! A detection result and the evidence behind it. Serialised into the
//! persistent cache, so every field must round-trip through serde.

use crate::types::{FrameworkMatch, Indicator};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvidenceType {
    IndicatorFile,
    ManifestFile,
    FrameworkDependency,
    RootIndicator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub evidence_type: EvidenceType,
    pub file_path: String,
    pub pattern_matched: String,
    pub weight: f32,
    pub description: String,
}

impl EvidenceItem {
    pub fn indicator_file(file_path: String, pattern: String, weight: f32) -> Self {
        let description = format!("Matched {} against pattern {}", file_path, pattern);
        Self {
            evidence_type: EvidenceType::IndicatorFile,
            file_path,
            pattern_matched: pattern,
            weight,
            description,
        }
    }

    pub fn manifest_file(file_path: String, pattern: String, weight: f32) -> Self {
        let description = format!("Found manifest file: {}", file_path);
        Self {
            evidence_type: EvidenceType::ManifestFile,
            file_path,
            pattern_matched: pattern,
            weight,
            description,
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
        let description = format!("Root indicator found: {}", file_path);
        Self {
            evidence_type: EvidenceType::RootIndicator,
            file_path,
            pattern_matched: pattern,
            weight,
            description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFactor {
    pub factor_type: String,
    pub value: f32,
    pub description: String,
}

impl ConfidenceFactor {
    pub fn new(factor_type: String, value: f32, description: String) -> Self {
        Self {
            factor_type,
            value,
            description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DetectionEvidence {
    pub indicator_evidence: Vec<EvidenceItem>,
    pub framework_evidence: Vec<EvidenceItem>,
    pub root_discovery: Vec<EvidenceItem>,
    /// The directory root discovery settled on (absolute) and the scan ran
    /// in, when it found one; scan and framework evidence is relative to it
    pub scan_root: Option<std::path::PathBuf>,
    /// Whether this result was aggregated at a VCS repository root
    #[serde(default)]
    pub repository: bool,
    pub confidence_factors: Vec<ConfidenceFactor>,
    pub files_matched: usize,
    pub total_scan_time_ms: u64,
    /// The scan timed out and the result is built from a partial sample;
    /// such results must not be persisted
    pub scan_truncated: bool,
    /// A manifest or directory that exists could not be read, so the result
    /// was computed without it; fixing the permissions changes no mtime, so
    /// such results must not be persisted either
    pub read_failed: bool,
    /// Directories (relative to the scan root) whose contents shaped the
    /// result: the nearest existing ancestor of each probed source
    /// directory, and — for a container folder — every directory its
    /// nested-project scan examined (absolute). The persistent cache
    /// watches them; the root itself is always watched.
    pub watched_dirs: Vec<String>,
    /// Files, relative to the scan root, that matchers read to decide the
    /// frameworks (lockfiles no indicator pattern lists, ...). The persistent
    /// cache watches them too.
    pub read_files: Vec<String>,
}

impl DetectionEvidence {
    /// A scan that listed more directories than this is not persisted: the
    /// cache would have to stat them all on every hit.
    pub const MAX_WATCHED_DIRS: usize = 256;

    /// A result that read more files than this is not persisted either:
    /// every one is stat-ed on every hit
    pub const MAX_READ_FILES: usize = 256;

    /// Whether the persistent cache may keep this result
    pub fn is_cacheable(&self) -> bool {
        !self.scan_truncated && !self.read_failed
    }

    /// Two indicators matching the same file by the same pattern (JavaScript
    /// and TypeScript both claim package.json) produce one row, not two
    pub fn add_indicator_evidence(&mut self, evidence: EvidenceItem) {
        if !self.indicator_evidence.contains(&evidence) {
            self.indicator_evidence.push(evidence);
        }
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

    pub fn set_scan_metrics(&mut self, files_matched: usize, scan_time_ms: u64) {
        self.files_matched = files_matched;
        self.total_scan_time_ms = scan_time_ms;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionResult {
    pub indicator: Option<Arc<Indicator>>,
    pub frameworks: Vec<FrameworkMatch>,
    pub confidence: f32,
    pub evidence: DetectionEvidence,
    /// Projects discovered within a repository or a bounded non-VCS container
    pub nested: Vec<NestedProject>,
}

/// The framework that stands for a project: lowest priority number, then
/// highest confidence
fn best_framework_of(frameworks: &[FrameworkMatch]) -> Option<&FrameworkMatch> {
    frameworks.iter().min_by(|a, b| {
        a.framework
            .priority
            .cmp(&b.framework.priority)
            .then(b.confidence.total_cmp(&a.confidence))
    })
}

/// Icon and color are resolved as a PAIR from the same source: the best
/// framework's icon with its color (falling back to the language's color),
/// otherwise the language's own
fn display_source_of<'a>(
    indicator: Option<&'a Indicator>,
    frameworks: &'a [FrameworkMatch],
) -> (Option<&'a str>, Option<&'a str>) {
    if let Some(best) = best_framework_of(frameworks) {
        if let Some(icon) = best.framework.icon.as_deref() {
            let color = best
                .framework
                .color
                .as_deref()
                .or_else(|| indicator.map(|l| l.color.as_str()));
            return (Some(icon), color);
        }
    }
    (
        indicator.map(|l| l.icon.as_str()),
        indicator.map(|l| l.color.as_str()),
    )
}

/// A project below a folder that is not itself one
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedProject {
    /// The path relative to the containing folder, using `/` separators
    pub name: String,
    pub indicator: Arc<Indicator>,
    pub frameworks: Vec<FrameworkMatch>,
}

impl NestedProject {
    pub fn display_icon(&self) -> Option<&str> {
        display_source_of(Some(&self.indicator), &self.frameworks).0
    }

    pub fn display_color(&self) -> Option<&str> {
        display_source_of(Some(&self.indicator), &self.frameworks).1
    }
}

impl DetectionResult {
    pub fn new(
        indicator: Option<Arc<Indicator>>,
        frameworks: Vec<FrameworkMatch>,
        confidence: f32,
    ) -> Self {
        Self {
            indicator,
            frameworks,
            confidence,
            evidence: DetectionEvidence::default(),
            nested: Vec::new(),
        }
    }

    pub fn new_with_evidence(
        indicator: Option<Arc<Indicator>>,
        frameworks: Vec<FrameworkMatch>,
        confidence: f32,
        evidence: DetectionEvidence,
    ) -> Self {
        Self {
            indicator,
            frameworks,
            confidence,
            evidence,
            nested: Vec::new(),
        }
    }

    /// Icon and color are resolved as a PAIR from the same source: if the
    /// best framework supplies the icon, its color is used; when the icon
    /// falls back to the indicator, the color falls back with it. This keeps
    /// an indicator's glyph from rendering in a framework's brand color
    /// (e.g. the JavaScript logo in Svelte orange).
    fn display_source(&self) -> (Option<&str>, Option<&str>) {
        display_source_of(self.indicator.as_deref(), &self.frameworks)
    }
    pub fn display_icon(&self) -> Option<&str> {
        self.display_source().0
    }
    pub fn display_color(&self) -> Option<&str> {
        self.display_source().1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_evidence_is_recorded_once() {
        let mut evidence = DetectionEvidence::default();
        let item =
            || EvidenceItem::manifest_file("package.json".into(), "package.json".into(), 0.8);
        evidence.add_indicator_evidence(item());
        evidence.add_indicator_evidence(item());
        assert_eq!(evidence.indicator_evidence.len(), 1);
    }

    #[test]
    fn test_evidence_for_a_different_pattern_is_kept() {
        let mut evidence = DetectionEvidence::default();
        for pattern in ["*.js", "a.*"] {
            evidence.add_indicator_evidence(EvidenceItem::indicator_file(
                "a.js".into(),
                pattern.into(),
                0.8,
            ));
        }
        assert_eq!(evidence.indicator_evidence.len(), 2);
    }

    #[test]
    fn test_icon_and_color_come_from_the_same_source() {
        use crate::types::{DetectionType, Framework, FrameworkMatch};

        let indicator = crate::types::Indicator::new(
            "JavaScript".to_string(),
            vec!["*.js".to_string()],
            "#f7df1e".to_string(),
            "JS".to_string(),
            vec![],
        );

        // Framework with a color but NO icon (like Svelte/Vite/SolidJS)
        let framework = Framework {
            name: "Svelte".to_string(),
            ecosystems: vec![],
            detection: DetectionType::FileExists(vec![]),
            icon: None,
            color: Some("#ff3e00".to_string()),
            priority: 1,
        };
        let result = DetectionResult::new(
            Some(std::sync::Arc::new(indicator)),
            vec![FrameworkMatch::new(framework, 0.9, vec![])],
            0.9,
        );

        // The icon falls back to the indicator's glyph, so the color must
        // fall back with it — a JS logo must not render in Svelte orange
        assert_eq!(result.display_icon(), Some("JS"));
        assert_eq!(result.display_color(), Some("#f7df1e"));
    }
}
