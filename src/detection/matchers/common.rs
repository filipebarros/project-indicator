//! Common utilities for framework matchers

use crate::types::FrameworkMatch;

/// Standard confidence calculation for dependency-based detection
///
/// This provides a consistent algorithm across all matchers:
/// - Base confidence: (found/required) * 0.9
/// - Completeness bonus: +0.1 if all required dependencies found
/// - Result clamped to [0.0, 1.0]
pub fn calculate_dependency_confidence(required: &[String], found: &[String]) -> f32 {
    if required.is_empty() {
        return 0.0;
    }

    let match_ratio = found.len() as f32 / required.len() as f32;

    // Base confidence from match ratio
    let base_confidence = match_ratio * 0.9;

    // Bonus for having all dependencies
    let completeness_bonus = if found.len() == required.len() {
        0.1
    } else {
        0.0
    };

    (base_confidence + completeness_bonus).min(1.0)
}

/// Standard sorting for framework matches
///
/// Sorts by:
/// 1. Confidence (highest first)
/// 2. Framework priority (lowest first - higher priority)
pub fn sort_framework_matches(matches: &mut [FrameworkMatch]) {
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.framework.priority.cmp(&b.framework.priority))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dependency_confidence() {
        // Perfect match
        assert_eq!(
            calculate_dependency_confidence(&["react".to_string()], &["react".to_string()]),
            1.0
        );

        // Partial match
        let confidence = calculate_dependency_confidence(
            &["react".to_string(), "react-dom".to_string()],
            &["react".to_string()],
        );
        assert!(confidence > 0.0 && confidence < 1.0);
        assert_eq!(confidence, 0.45); // (1/2) * 0.9 = 0.45

        // No match
        assert_eq!(
            calculate_dependency_confidence(&["react".to_string()], &[]),
            0.0
        );

        // Empty required
        assert_eq!(
            calculate_dependency_confidence(&[], &["react".to_string()]),
            0.0
        );
    }

    #[test]
    fn test_confidence_edge_cases() {
        // Multiple required, all found
        let confidence = calculate_dependency_confidence(
            &[
                "react".to_string(),
                "react-dom".to_string(),
                "typescript".to_string(),
            ],
            &[
                "react".to_string(),
                "react-dom".to_string(),
                "typescript".to_string(),
            ],
        );
        assert_eq!(confidence, 1.0); // 0.9 + 0.1 completeness bonus

        // More found than required (unusual but possible)
        let confidence = calculate_dependency_confidence(
            &["react".to_string()],
            &[
                "react".to_string(),
                "react-dom".to_string(),
                "extra".to_string(),
            ],
        );
        assert_eq!(confidence, 1.0); // Should cap at 1.0, gets completeness bonus

        // Large number of dependencies
        let required: Vec<String> = (0..10).map(|i| format!("dep{}", i)).collect();
        let found: Vec<String> = (0..7).map(|i| format!("dep{}", i)).collect();
        let confidence = calculate_dependency_confidence(&required, &found);
        assert_eq!(confidence, 0.63); // (7/10) * 0.9 = 0.63

        // Single dependency not found
        let confidence = calculate_dependency_confidence(&["missing-dep".to_string()], &[]);
        assert_eq!(confidence, 0.0);
    }

    #[test]
    fn test_confidence_precision() {
        // Test specific ratios for precision
        let confidence = calculate_dependency_confidence(
            &["a".to_string(), "b".to_string(), "c".to_string()],
            &["a".to_string(), "b".to_string()],
        );
        assert_eq!(confidence, 0.6); // (2/3) * 0.9 = 0.6

        let confidence = calculate_dependency_confidence(
            &[
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ],
            &["a".to_string()],
        );
        assert_eq!(confidence, 0.225); // (1/4) * 0.9 = 0.225
    }

    #[test]
    fn test_sort_framework_matches() {
        use crate::types::{DetectionType, FrameworkDetector};

        // Create test framework matches with different confidence and priority
        let framework1 = FrameworkDetector {
            name: "High Confidence, Low Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 5, // Lower priority (higher number)
            files: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "High Confidence, High Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1, // Higher priority (lower number)
            files: vec![],
        };

        let framework3 = FrameworkDetector {
            name: "Low Confidence, High Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut matches = vec![
            FrameworkMatch::new(framework1, 0.9, vec!["file1".to_string()]),
            FrameworkMatch::new(framework2, 0.9, vec!["file2".to_string()]),
            FrameworkMatch::new(framework3, 0.5, vec!["file3".to_string()]),
        ];

        sort_framework_matches(&mut matches);

        // Should be sorted by confidence first, then priority
        assert_eq!(matches[0].framework.name, "High Confidence, High Priority"); // 0.9 confidence, priority 1
        assert_eq!(matches[1].framework.name, "High Confidence, Low Priority"); // 0.9 confidence, priority 5
        assert_eq!(matches[2].framework.name, "Low Confidence, High Priority"); // 0.5 confidence, priority 1
    }

    #[test]
    fn test_sort_framework_matches_edge_cases() {
        use crate::types::{DetectionType, FrameworkDetector};

        // Test with identical confidence and priority
        let framework1 = FrameworkDetector {
            name: "First".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "Second".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        };

        let mut matches = vec![
            FrameworkMatch::new(framework1, 0.8, vec![]),
            FrameworkMatch::new(framework2, 0.8, vec![]),
        ];

        sort_framework_matches(&mut matches);

        // Order should be stable for identical values
        assert_eq!(matches.len(), 2);
        assert!(matches[0].confidence == 0.8);
        assert!(matches[1].confidence == 0.8);
    }

    #[test]
    fn test_sort_empty_matches() {
        let mut matches: Vec<FrameworkMatch> = vec![];
        sort_framework_matches(&mut matches);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_confidence_clamping() {
        // Test that confidence is properly clamped to 1.0
        let very_large_found: Vec<String> = (0..100).map(|i| format!("dep{}", i)).collect();
        let small_required: Vec<String> = (0..1).map(|i| format!("dep{}", i)).collect();

        let confidence = calculate_dependency_confidence(&small_required, &very_large_found);
        assert_eq!(confidence, 1.0); // Should be clamped

        // Test with exact boundary
        let confidence = calculate_dependency_confidence(&["a".to_string()], &["a".to_string()]);
        assert_eq!(confidence, 1.0); // 0.9 + 0.1 = 1.0 exactly
    }
}
