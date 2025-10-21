use super::types::DetectionSnapshot;
use serde::{Deserialize, Serialize};

/// The result of comparing two detection snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub from_snapshot: String, // snapshot_id
    pub to_snapshot: String,
    pub from_timestamp: u64,
    pub to_timestamp: u64,
    pub path: String,
    pub changes: Vec<ChangeDetected>,
}

/// Types of changes that can be detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeDetected {
    LanguageChanged {
        from: Option<String>,
        to: Option<String>,
    },
    FrameworkAdded {
        name: String,
        confidence: f32,
    },
    FrameworkRemoved {
        name: String,
    },
    ConfidenceChanged {
        from: f32,
        to: f32,
        delta: f32,
    },
    CacheStatusChanged {
        from_cached: bool,
        to_cached: bool,
    },
    PerformanceChanged {
        from_micros: u64,
        to_micros: u64,
        delta_micros: i64,
    },
}

impl SnapshotDiff {
    /// Compare two snapshots and generate a diff
    pub fn compare(from: &DetectionSnapshot, to: &DetectionSnapshot) -> Self {
        let mut changes = Vec::new();

        // Check language change
        let from_lang = from.language.as_ref().map(|l| l.name.to_string());
        let to_lang = to.language.as_ref().map(|l| l.name.to_string());

        if from_lang != to_lang {
            changes.push(ChangeDetected::LanguageChanged {
                from: from_lang,
                to: to_lang,
            });
        }

        // Check framework changes
        let from_frameworks: std::collections::HashSet<_> =
            from.frameworks.iter().map(|f| f.name.as_ref()).collect();
        let to_frameworks: std::collections::HashSet<_> =
            to.frameworks.iter().map(|f| f.name.as_ref()).collect();

        // Frameworks added
        for to_fw in &to.frameworks {
            if !from_frameworks.contains(to_fw.name.as_ref()) {
                changes.push(ChangeDetected::FrameworkAdded {
                    name: to_fw.name.to_string(),
                    confidence: to_fw.confidence,
                });
            }
        }

        // Frameworks removed
        for from_fw in &from.frameworks {
            if !to_frameworks.contains(from_fw.name.as_ref()) {
                changes.push(ChangeDetected::FrameworkRemoved {
                    name: from_fw.name.to_string(),
                });
            }
        }

        // Check confidence change
        if (from.confidence - to.confidence).abs() > 0.01 {
            changes.push(ChangeDetected::ConfidenceChanged {
                from: from.confidence,
                to: to.confidence,
                delta: to.confidence - from.confidence,
            });
        }

        // Check cache status change
        if from.cache_info.detection_from_cache != to.cache_info.detection_from_cache {
            changes.push(ChangeDetected::CacheStatusChanged {
                from_cached: from.cache_info.detection_from_cache,
                to_cached: to.cache_info.detection_from_cache,
            });
        }

        // Check performance change (if significant)
        let delta = to.duration_micros as i64 - from.duration_micros as i64;
        if delta.abs() > 1000 {
            // More than 1ms difference
            changes.push(ChangeDetected::PerformanceChanged {
                from_micros: from.duration_micros,
                to_micros: to.duration_micros,
                delta_micros: delta,
            });
        }

        Self {
            from_snapshot: from.snapshot_id.clone(),
            to_snapshot: to.snapshot_id.clone(),
            from_timestamp: from.timestamp,
            to_timestamp: to.timestamp,
            path: from.path.clone(),
            changes,
        }
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Get only breaking changes (language/framework changes)
    pub fn breaking_changes(&self) -> Vec<&ChangeDetected> {
        self.changes
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    ChangeDetected::LanguageChanged { .. }
                        | ChangeDetected::FrameworkAdded { .. }
                        | ChangeDetected::FrameworkRemoved { .. }
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::types::*;

    fn create_snapshot(lang: &str, frameworks: Vec<&str>) -> DetectionSnapshot {
        use std::sync::Arc;

        DetectionSnapshot {
            snapshot_id: DetectionSnapshot::new_id(),
            timestamp: 1234567890,
            path: "/test".to_string(),
            path_hash: 0,
            language: Some(LanguageResult {
                name: Arc::from(lang),
                confidence: 0.95,
                sample_files: vec![],
            }),
            frameworks: frameworks
                .iter()
                .map(|name| FrameworkResult {
                    name: Arc::from(*name),
                    confidence: 0.9,
                    detected_via: vec![],
                })
                .collect(),
            confidence: 0.95,
            evidence: EvidenceSnapshot {
                sample_matched_files: vec![],
                root_indicators: vec![],
                early_termination: None,
                total_files_matched: 0,
            },
            cache_info: CacheInfo {
                detection_from_cache: false,
                cached_at: None,
                pattern_cache_hits: 0,
                pattern_cache_misses: 0,
                fs_cache_hits: 0,
                fs_cache_misses: 0,
            },
            duration_micros: 3200,
            files_scanned: 42,
        }
    }

    #[test]
    fn test_no_changes() {
        let s1 = create_snapshot("Rust", vec!["Rocket"]);
        let s2 = create_snapshot("Rust", vec!["Rocket"]);

        let diff = SnapshotDiff::compare(&s1, &s2);

        // Should have no changes (except maybe performance)
        assert!(diff.breaking_changes().is_empty());
    }

    #[test]
    fn test_language_changed() {
        let s1 = create_snapshot("JavaScript", vec!["React"]);
        let s2 = create_snapshot("TypeScript", vec!["React"]);

        let diff = SnapshotDiff::compare(&s1, &s2);

        assert!(diff.has_changes());
        assert_eq!(diff.breaking_changes().len(), 1);

        match &diff.changes[0] {
            ChangeDetected::LanguageChanged { from, to } => {
                assert_eq!(from.as_deref(), Some("JavaScript"));
                assert_eq!(to.as_deref(), Some("TypeScript"));
            }
            _ => panic!("Expected LanguageChanged"),
        }
    }

    #[test]
    fn test_framework_added() {
        let s1 = create_snapshot("TypeScript", vec!["React"]);
        let s2 = create_snapshot("TypeScript", vec!["React", "Next.js"]);

        let diff = SnapshotDiff::compare(&s1, &s2);

        assert!(diff.has_changes());

        let has_framework_add = diff
            .changes
            .iter()
            .any(|c| matches!(c, ChangeDetected::FrameworkAdded { name, .. } if name == "Next.js"));

        assert!(has_framework_add);
    }

    #[test]
    fn test_framework_removed() {
        let s1 = create_snapshot("TypeScript", vec!["React", "Next.js"]);
        let s2 = create_snapshot("TypeScript", vec!["React"]);

        let diff = SnapshotDiff::compare(&s1, &s2);

        let has_framework_remove = diff
            .changes
            .iter()
            .any(|c| matches!(c, ChangeDetected::FrameworkRemoved { name } if name == "Next.js"));

        assert!(has_framework_remove);
    }
}
