//! Detection evidence and result types for language/framework identification.
//!
//! This module provides types for tracking detection evidence, calculating confidence scores,
//! and representing final detection results with full provenance information.
//!
//! # Overview
//!
//! The detection system builds confidence through evidence collection:
//!
//! 1. **Evidence Collection** - Gather [`EvidenceItem`]s from file scanning
//! 2. **Confidence Calculation** - Combine evidence into [`ConfidenceFactor`]s
//! 3. **Result Assembly** - Create final [`DetectionResult`] with language + frameworks
//!
//! # Core Types
//!
//! - [`EvidenceType`] - Categories of evidence (files, manifests, dependencies, etc.)
//! - [`EvidenceItem`] - Individual piece of evidence with weight and description
//! - [`ConfidenceFactor`] - Calculated confidence component
//! - [`DetectionEvidence`] - Complete collection of all evidence
//! - [`DetectionResult`] - Final result with language, frameworks, and confidence
//!
//! # Evidence Types
//!
//! Different types of evidence contribute to confidence differently:
//!
//! - **IndicatorFile** - Source files matching patterns (*.rs, *.py)
//! - **ManifestFile** - Package manifests (Cargo.toml, package.json) - Higher weight
//! - **ConfigFile** - Configuration files (tsconfig.json, .eslintrc)
//! - **FrameworkDependency** - Framework detected in dependencies
//! - **RootIndicator** - Project root markers - Highest weight
//! - **DirectoryStructure** - Conventional directory names (src/, tests/)
//!
//! # Example: Building Evidence
//!
//! ```rust
//! use project_indicator::types::{DetectionEvidence, EvidenceItem};
//!
//! let mut evidence = DetectionEvidence::new();
//!
//! // Add language file evidence
//! evidence.add_indicator_evidence(EvidenceItem::indicator_file(
//!     "src/main.rs".to_string(),
//!     "*.rs".to_string(),
//!     0.8,
//! ));
//!
//! // Add manifest evidence (higher weight)
//! evidence.add_indicator_evidence(EvidenceItem::manifest_file(
//!     "Cargo.toml".to_string(),
//!     "Cargo.toml".to_string(),
//!     0.95,
//! ));
//!
//! // Add framework evidence
//! evidence.add_framework_evidence(EvidenceItem::framework_dependency(
//!     "Cargo.toml".to_string(),
//!     "Rocket",
//!     0.9,
//! ));
//!
//! // Set scan metrics
//! evidence.set_scan_metrics(42, 150);
//!
//! assert_eq!(evidence.indicator_evidence.len(), 2);
//! assert_eq!(evidence.framework_evidence.len(), 1);
//! assert_eq!(evidence.files_scanned, 42);
//! ```
//!
//! # Detection Results
//!
//! A [`DetectionResult`] contains the complete detection outcome:
//!
//! ```rust
//! use project_indicator::types::{
//!     DetectionResult, Indicator, FrameworkMatch,
//!     Framework, DetectionType
//! };
//! use std::sync::Arc;
//!
//! // Create language
//! let rust = Arc::new(Indicator::new(
//!     "Rust".to_string(),
//!     vec!["*.rs".to_string()],
//!     "#DEA584".to_string(),
//!     "".to_string(),
//!     1,
//!     vec![],
//! ));
//!
//! // Create framework match
//! let rocket = Framework {
//!     name: "Rocket".to_string(),
//!     ecosystems: vec![],
//!     detection: DetectionType::Dependencies {
//!         dependencies: vec!["rocket".to_string()],
//!     },
//!     icon: Some("🚀".to_string()),
//!     color: Some("#D33847".to_string()),
//!     priority: 1,
//!     files: vec![],
//!     root_indicators: vec![],
//! };
//!
//! let framework_match = FrameworkMatch::new(
//!     rocket,
//!     0.9,
//!     vec!["Cargo.toml".to_string()],
//! );
//!
//! // Assemble final result
//! let result = DetectionResult::new(
//!     Some(rust),
//!     vec![framework_match],
//!     0.95,
//! );
//!
//! assert_eq!(result.confidence, 0.95);
//! assert_eq!(result.display_icon(), Some("🚀"));
//! assert_eq!(result.display_color(), Some("#D33847"));
//! ```
//!
//! # Display Priority
//!
//! When displaying results, the system prioritizes in this order:
//!
//! 1. **Best framework** - Highest priority framework (lowest priority number)
//! 2. **Language fallback** - If no frameworks, use language icon/color
//!
//! ```rust
//! use project_indicator::types::{DetectionResult, Indicator};
//! use std::sync::Arc;
//!
//! let python = Arc::new(Indicator::new(
//!     "Python".to_string(),
//!     vec!["*.py".to_string()],
//!     "#3776AB".to_string(),
//!     "".to_string(),
//!     1,
//!     vec![],
//! ));
//!
//! // No frameworks - uses language icon/color
//! let result = DetectionResult::new(Some(python), vec![], 0.8);
//! assert_eq!(result.display_icon(), Some(""));
//! assert_eq!(result.display_color(), Some("#3776AB"));
//! ```
//!
//! # Confidence Factors
//!
//! Confidence is built from weighted factors:
//!
//! - File quantity and diversity
//! - Presence of root indicators
//! - Framework detection strength
//! - Directory structure matching
//!
//! Each factor contributes a weighted score to the final confidence value.

use crate::types::{FrameworkMatch, Indicator};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvidenceType {
    IndicatorFile,
    ManifestFile,
    ConfigFile,
    FrameworkDependency,
    RootIndicator,
    DirectoryStructure,
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

    pub fn config_file(file_path: String, pattern: String, weight: f32) -> Self {
        let description = format!("Found config file: {}", file_path);
        Self {
            evidence_type: EvidenceType::ConfigFile,
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
            pattern_matched: crate::constants::FRAMEWORK_DETECTION_PATTERN.to_owned(),
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
    pub weight: f32,
    pub description: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionEvidence {
    pub indicator_evidence: Vec<EvidenceItem>,
    pub framework_evidence: Vec<EvidenceItem>,
    pub root_discovery: Vec<EvidenceItem>,
    pub confidence_factors: Vec<ConfidenceFactor>,
    pub files_scanned: usize,
    pub total_scan_time_ms: u64,
}

impl DetectionEvidence {
    pub fn new() -> Self {
        Self {
            indicator_evidence: Vec::new(),
            framework_evidence: Vec::new(),
            root_discovery: Vec::new(),
            confidence_factors: Vec::new(),
            files_scanned: 0,
            total_scan_time_ms: 0,
        }
    }

    pub fn add_indicator_evidence(&mut self, evidence: EvidenceItem) {
        self.indicator_evidence.push(evidence);
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

impl Default for DetectionEvidence {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionResult {
    pub indicator: Option<Arc<Indicator>>,
    pub frameworks: Vec<FrameworkMatch>,
    pub confidence: f32,
    pub evidence: DetectionEvidence,
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
            evidence: DetectionEvidence::new(),
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
        }
    }

    pub fn empty() -> Self {
        Self {
            indicator: None,
            frameworks: Vec::new(),
            confidence: 0.0,
            evidence: DetectionEvidence::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.indicator.is_none() && self.frameworks.is_empty()
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
    /// Icon and color are resolved as a PAIR from the same source: if the
    /// best framework supplies the icon, its color is used; when the icon
    /// falls back to the indicator, the color falls back with it. This keeps
    /// an indicator's glyph from rendering in a framework's brand color
    /// (e.g. the JavaScript logo in Svelte orange).
    fn display_source(&self) -> (Option<&str>, Option<&str>) {
        if let Some(best) = self.best_framework() {
            if let Some(icon) = best.framework.icon.as_deref() {
                let color = best
                    .framework
                    .color
                    .as_deref()
                    .or_else(|| self.indicator.as_ref().map(|l| l.color.as_str()));
                return (Some(icon), color);
            }
        }
        (
            self.indicator.as_ref().map(|l| l.icon.as_str()),
            self.indicator.as_ref().map(|l| l.color.as_str()),
        )
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
    fn test_icon_and_color_come_from_the_same_source() -> Result<(), Box<dyn std::error::Error>> {
        use crate::types::{DetectionType, Framework, FrameworkMatch};

        let indicator = crate::types::Indicator::new(
            "JavaScript".to_string(),
            vec!["*.js".to_string()],
            "#f7df1e".to_string(),
            "JS".to_string(),
            6,
            vec![],
        );

        // Framework with a color but NO icon (like Svelte/Vite/SolidJS)
        let framework = Framework {
            name: "Svelte".to_string(),
            ecosystems: vec![],
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: Some("#ff3e00".to_string()),
            priority: 1,
            files: vec![],
            root_indicators: vec![],
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
        Ok(())
    }

    #[test]
    fn test_detection_result_serde_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let lang = crate::types::Indicator::new(
            "Rust".to_string(),
            vec!["Cargo.toml".to_string()],
            "#dea584".to_string(),
            "R".to_string(),
            1,
            vec![],
        );
        let result = DetectionResult::new(Some(std::sync::Arc::new(lang)), vec![], 0.9);
        let json = serde_json::to_string(&result)?;
        let back: DetectionResult = serde_json::from_str(&json)?;
        assert_eq!(result, back);
        Ok(())
    }
}
