use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{DetectionEvidence as OriginalEvidence, DetectionResult};

/// A snapshot of a single detection result with full context.
///
/// This captures everything needed to:
/// - Reproduce the result
/// - Understand why the detection happened
/// - Compare with future detections
/// - Debug changes over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionSnapshot {
    /// Unique identifier for this snapshot
    pub snapshot_id: String,

    /// When this detection occurred (Unix timestamp)
    pub timestamp: u64,

    /// Canonicalized path that was detected
    pub path: String,

    /// Hash of the path for fast lookups
    pub path_hash: u64,

    /// Detected language (if any)
    pub language: Option<LanguageResult>,

    /// Detected frameworks
    pub frameworks: Vec<FrameworkResult>,

    /// Overall confidence score
    pub confidence: f32,

    /// Evidence that led to this result
    pub evidence: EvidenceSnapshot,

    /// Cache usage information
    pub cache_info: CacheInfo,

    /// Performance metrics
    pub duration_micros: u64,
    pub files_scanned: usize,
}

/// Language detection result
///
/// Uses `Arc<str>` for name to reduce allocations when the same
/// language name appears in multiple snapshots (common in shell prompts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageResult {
    #[serde(with = "arc_str_serde")]
    pub name: Arc<str>,
    pub confidence: f32,
    /// Sample of files that led to this detection (limited to 10)
    pub sample_files: Vec<String>,
}

/// Framework detection result
///
/// Uses `Arc<str>` for name to reduce allocations when the same
/// framework name appears in multiple snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkResult {
    #[serde(with = "arc_str_serde")]
    pub name: Arc<str>,
    pub confidence: f32,
    /// Files that provided evidence for this framework
    pub detected_via: Vec<String>,
}

/// Custom serde module for `Arc<str>` serialization
mod arc_str_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(arc: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arc.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Arc::from(s.as_str()))
    }
}

/// Evidence collected during detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSnapshot {
    /// Sample of matched files (limited to avoid huge snapshots)
    pub sample_matched_files: Vec<FileEvidence>,

    /// Root indicators found
    pub root_indicators: Vec<String>,

    /// Early termination reason (if any)
    pub early_termination: Option<String>,

    /// Number of total files matched
    pub total_files_matched: usize,
}

/// Individual file evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvidence {
    pub path: String,
    pub pattern: String,
    pub depth: usize,
}

/// Information about cache usage during detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    /// Was this entire detection result from cache?
    pub detection_from_cache: bool,

    /// If from cache, when was it originally computed?
    pub cached_at: Option<u64>,

    /// Pattern cache statistics during this detection
    pub pattern_cache_hits: usize,
    pub pattern_cache_misses: usize,

    /// File system cache statistics
    pub fs_cache_hits: usize,
    pub fs_cache_misses: usize,
}

/// Metadata collected during detection
pub struct DetectionMetadata {
    pub duration_micros: u64,
    pub from_cache: bool,
    pub cached_at: Option<u64>,
    pub pattern_cache_hits: usize,
    pub pattern_cache_misses: usize,
    pub fs_cache_hits: usize,
    pub fs_cache_misses: usize,
}

impl DetectionSnapshot {
    /// Generate a new unique snapshot ID
    pub fn new_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Calculate path hash for fast lookups
    pub fn hash_path(path: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }

    /// Get the age of this snapshot in seconds
    pub fn age_seconds(&self) -> Result<u64, std::time::SystemTimeError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        Ok(now.saturating_sub(self.timestamp))
    }

    /// Calculate cache hit rate percentage
    pub fn cache_hit_rate(&self) -> f64 {
        let total =
            (self.cache_info.pattern_cache_hits + self.cache_info.pattern_cache_misses) as f64;

        if total == 0.0 {
            return 0.0;
        }

        (self.cache_info.pattern_cache_hits as f64 / total) * 100.0
    }

    /// Create a snapshot from a DetectionResult
    pub fn from_detection_result(
        result: &DetectionResult,
        path: &Path,
        metadata: DetectionMetadata,
        path_cache: Option<&super::PathCache>,
    ) -> anyhow::Result<Self> {
        // Use path cache if available, otherwise canonicalize directly
        let path_str = if let Some(cache) = path_cache {
            cache.get_canonical(path)
        } else {
            path.canonicalize()
                .unwrap_or_else(|_| path.to_path_buf())
                .to_string_lossy()
                .to_string()
        };

        let path_hash = Self::hash_path(&path_str);

        Ok(Self {
            snapshot_id: Self::new_id(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            path: path_str,
            path_hash,
            language: result.language.as_ref().map(|lang| LanguageResult {
                name: Arc::from(lang.name.as_str()),
                confidence: result.confidence,
                sample_files: Self::extract_language_files(&result.evidence, &lang.name),
            }),
            frameworks: result
                .frameworks
                .iter()
                .map(|fm| FrameworkResult {
                    name: Arc::from(fm.framework.name.as_str()),
                    confidence: fm.confidence,
                    detected_via: fm.evidence.clone(),
                })
                .collect(),
            confidence: result.confidence,
            evidence: EvidenceSnapshot::from_evidence(&result.evidence),
            cache_info: CacheInfo {
                detection_from_cache: metadata.from_cache,
                cached_at: metadata.cached_at,
                pattern_cache_hits: metadata.pattern_cache_hits,
                pattern_cache_misses: metadata.pattern_cache_misses,
                fs_cache_hits: metadata.fs_cache_hits,
                fs_cache_misses: metadata.fs_cache_misses,
            },
            duration_micros: metadata.duration_micros,
            files_scanned: result.evidence.files_scanned,
        })
    }

    /// Extract sample files that matched for a specific language
    fn extract_language_files(evidence: &OriginalEvidence, language_name: &str) -> Vec<String> {
        evidence
            .language_evidence
            .iter()
            .filter_map(|item| {
                // Extract file path from evidence description
                // Evidence items have format like "File: src/main.rs (pattern: *.rs)"
                if item.description.contains(language_name) || item.description.contains("File:") {
                    Some(item.description.clone())
                } else {
                    None
                }
            })
            .take(10) // Limit to 10 samples
            .collect()
    }

    /// Pre-serialize the snapshot to a byte buffer for faster writing.
    ///
    /// This optimization allows serialization to happen off the critical path
    /// and enables potential buffering/batching strategies.
    ///
    /// **Performance Impact:**
    /// - Pre-allocates buffer with typical snapshot size (~2KB)
    /// - Reduces memory allocations during serialization
    /// - Can be serialized in background before writing
    ///
    /// Returns a `Vec<u8>` containing the JSON representation with trailing newline.
    pub fn serialize_to_buffer(&self) -> anyhow::Result<Vec<u8>> {
        // Pre-allocate buffer with typical snapshot size to reduce reallocations
        // Average snapshot is ~1-2KB, using 2KB as capacity
        let mut buffer = Vec::with_capacity(2048);

        serde_json::to_writer(&mut buffer, self)?;
        buffer.push(b'\n');

        Ok(buffer)
    }

    /// Check if two snapshots have the same detection result
    pub fn has_same_result(&self, other: &Self) -> bool {
        // Compare language names (Arc<str> implements PartialEq)
        let lang_match = match (&self.language, &other.language) {
            (Some(a), Some(b)) => a.name.as_ref() == b.name.as_ref(),
            (None, None) => true,
            _ => false,
        };

        // Compare framework names (order-independent)
        let fw_names_self: std::collections::HashSet<_> =
            self.frameworks.iter().map(|f| f.name.as_ref()).collect();
        let fw_names_other: std::collections::HashSet<_> =
            other.frameworks.iter().map(|f| f.name.as_ref()).collect();

        lang_match && fw_names_self == fw_names_other
    }
}

impl EvidenceSnapshot {
    /// Convert from original DetectionEvidence
    fn from_evidence(evidence: &OriginalEvidence) -> Self {
        // Check if early termination occurred by looking for root indicators
        // with early_termination flag (this info is embedded in the evidence)
        let early_termination = Self::extract_early_termination(evidence);

        Self {
            sample_matched_files: evidence
                .language_evidence
                .iter()
                .take(20) // Limit sample size
                .map(|item| FileEvidence {
                    path: item.file_path.clone(),
                    pattern: item.pattern_matched.clone(),
                    depth: Self::calculate_path_depth(&item.file_path),
                })
                .collect(),
            root_indicators: evidence
                .root_discovery
                .iter()
                .map(|item| item.description.clone())
                .collect(),
            early_termination,
            total_files_matched: evidence.files_scanned,
        }
    }

    /// Calculate the depth of a path by counting separators
    fn calculate_path_depth(path: &str) -> usize {
        path.matches(std::path::MAIN_SEPARATOR).count()
    }

    /// Extract early termination info from evidence
    /// Checks root discovery for early termination indicators
    fn extract_early_termination(evidence: &OriginalEvidence) -> Option<String> {
        // Look through root discovery evidence for early termination mentions
        for item in &evidence.root_discovery {
            if item.description.contains("early termination")
                || item.description.contains("Early termination")
            {
                return Some(item.description.clone());
            }
        }

        // Check confidence factors for early termination
        for factor in &evidence.confidence_factors {
            if factor.description.contains("early termination")
                || factor.description.contains("Early termination")
            {
                return Some(factor.description.clone());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_id_generation() {
        let id1 = DetectionSnapshot::new_id();
        let id2 = DetectionSnapshot::new_id();

        // Each ID should be unique
        assert_ne!(id1, id2);

        // Should be valid UUID format
        assert_eq!(id1.len(), 36); // UUID v4 string length
    }

    #[test]
    fn test_path_hashing() {
        let path1 = "/home/user/project";
        let path2 = "/home/user/project";
        let path3 = "/home/user/different";

        let hash1 = DetectionSnapshot::hash_path(path1);
        let hash2 = DetectionSnapshot::hash_path(path2);
        let hash3 = DetectionSnapshot::hash_path(path3);

        // Same path should hash to same value
        assert_eq!(hash1, hash2);

        // Different path should (probably) hash differently
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_hit_rate_calculation() {
        let snapshot = DetectionSnapshot {
            snapshot_id: "test".to_string(),
            timestamp: 0,
            path: "/test".to_string(),
            path_hash: 0,
            language: None,
            frameworks: vec![],
            confidence: 0.0,
            evidence: EvidenceSnapshot {
                sample_matched_files: vec![],
                root_indicators: vec![],
                early_termination: None,
                total_files_matched: 0,
            },
            cache_info: CacheInfo {
                detection_from_cache: false,
                cached_at: None,
                pattern_cache_hits: 90,
                pattern_cache_misses: 10,
                fs_cache_hits: 0,
                fs_cache_misses: 0,
            },
            duration_micros: 0,
            files_scanned: 0,
        };

        assert_eq!(snapshot.cache_hit_rate(), 90.0);
    }

    #[test]
    fn test_serialization_roundtrip() -> anyhow::Result<()> {
        let snapshot = DetectionSnapshot {
            snapshot_id: DetectionSnapshot::new_id(),
            timestamp: 1234567890,
            path: "/test/path".to_string(),
            path_hash: DetectionSnapshot::hash_path("/test/path"),
            language: Some(LanguageResult {
                name: Arc::from("Rust"),
                confidence: 0.95,
                sample_files: vec!["src/main.rs".to_string()],
            }),
            frameworks: vec![],
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
        };

        // Serialize to JSON
        let json = serde_json::to_string(&snapshot)?;

        // Deserialize back
        let deserialized: DetectionSnapshot = serde_json::from_str(&json)?;

        // Should match
        assert_eq!(snapshot.snapshot_id, deserialized.snapshot_id);
        assert_eq!(snapshot.path, deserialized.path);
        assert_eq!(
            snapshot.language.as_ref().map(|l| l.name.as_ref()),
            deserialized.language.as_ref().map(|l| l.name.as_ref())
        );

        Ok(())
    }

    #[test]
    fn test_serialize_to_buffer() -> anyhow::Result<()> {
        let snapshot = DetectionSnapshot {
            snapshot_id: DetectionSnapshot::new_id(),
            timestamp: 1234567890,
            path: "/test/path".to_string(),
            path_hash: DetectionSnapshot::hash_path("/test/path"),
            language: Some(LanguageResult {
                name: Arc::from("Rust"),
                confidence: 0.95,
                sample_files: vec!["src/main.rs".to_string()],
            }),
            frameworks: vec![],
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
        };

        // Serialize to buffer
        let buffer = snapshot.serialize_to_buffer()?;

        // Should end with newline
        assert_eq!(buffer.last(), Some(&b'\n'));

        // Should be valid JSON (excluding the trailing newline)
        let json_str = std::str::from_utf8(&buffer[..buffer.len() - 1])?;
        let deserialized: DetectionSnapshot = serde_json::from_str(json_str)?;

        // Should match original
        assert_eq!(snapshot.snapshot_id, deserialized.snapshot_id);
        assert_eq!(snapshot.path, deserialized.path);

        Ok(())
    }
}
