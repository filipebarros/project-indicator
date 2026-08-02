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
//! - **LanguageFile** - Source files matching patterns (*.rs, *.py)
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
//! evidence.add_language_evidence(EvidenceItem::language_file(
//!     "src/main.rs".to_string(),
//!     "*.rs".to_string(),
//!     0.8,
//! ));
//!
//! // Add manifest evidence (higher weight)
//! evidence.add_language_evidence(EvidenceItem::manifest_file(
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
//! assert_eq!(evidence.language_evidence.len(), 2);
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
//!     DetectionResult, ProjectIndicator, FrameworkMatch,
//!     FrameworkDetector, DetectionType
//! };
//! use std::sync::Arc;
//!
//! // Create language
//! let rust = Arc::new(ProjectIndicator::new(
//!     "Rust".to_string(),
//!     vec!["*.rs".to_string()],
//!     "#DEA584".to_string(),
//!     "".to_string(),
//!     1,
//!     vec![],
//! ));
//!
//! // Create framework match
//! let rocket = FrameworkDetector {
//!     name: "Rocket".to_string(),
//!     detection: DetectionType::RustEcosystem {
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
//! use project_indicator::types::{DetectionResult, ProjectIndicator};
//! use std::sync::Arc;
//!
//! let python = Arc::new(ProjectIndicator::new(
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

use crate::types::{FrameworkMatch, ProjectIndicator};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvidenceType {
    LanguageFile,
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

    pub fn language_file(file_path: String, pattern: String, weight: f32) -> Self {
        let description = format!("Matched {} against pattern {}", file_path, pattern);
        Self {
            evidence_type: EvidenceType::LanguageFile,
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
    pub language_evidence: Vec<EvidenceItem>,
    pub framework_evidence: Vec<EvidenceItem>,
    pub root_discovery: Vec<EvidenceItem>,
    pub confidence_factors: Vec<ConfidenceFactor>,
    pub files_scanned: usize,
    pub total_scan_time_ms: u64,
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

impl Default for DetectionEvidence {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionResult {
    pub language: Option<Arc<ProjectIndicator>>,
    pub frameworks: Vec<FrameworkMatch>,
    pub confidence: f32,
    pub evidence: DetectionEvidence,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_result_serde_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let lang = crate::types::ProjectIndicator::new(
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
