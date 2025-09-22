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
}
