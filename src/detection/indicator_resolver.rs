use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::patterns::simple_wildcard_match;
use crate::types::{ConfidenceFactor, DetectionEvidence, EvidenceItem, Indicator, MatchedFile};
use std::sync::Arc;

pub struct IndicatorResolver {
    confidence_scorer: ConfidenceScorer,
}

impl IndicatorResolver {
    pub fn new() -> Self {
        Self {
            confidence_scorer: ConfidenceScorer::new(),
        }
    }

    pub fn detect_indicator_with_conflict_resolution_and_evidence(
        &self,
        languages: &[Arc<Indicator>],
        matched_files: &[MatchedFile],
        evidence: &mut DetectionEvidence,
    ) -> Option<Arc<Indicator>> {
        let mut candidates: Vec<(usize, f32)> = Vec::with_capacity(languages.len());

        for (idx, language) in languages.iter().enumerate() {
            let weighted_score = self.confidence_scorer.calculate_indicator_score(
                language,
                matched_files,
                languages,
            );
            if weighted_score > 0.1 {
                evidence.add_confidence_factor(ConfidenceFactor::new(
                    format!("language_{}", language.name),
                    weighted_score,
                    1.0,
                    format!("Score for {} language detection", language.name),
                ));

                for file in matched_files {
                    for pattern in &language.files {
                        if simple_wildcard_match(pattern, &file.filename) {
                            let item = if pattern.contains("*.") {
                                EvidenceItem::indicator_file(
                                    file.relative_path.clone(),
                                    pattern.clone(),
                                    0.8,
                                )
                            } else {
                                EvidenceItem::manifest_file(
                                    file.relative_path.clone(),
                                    pattern.clone(),
                                    0.8,
                                )
                            };
                            evidence.add_indicator_evidence(item);
                        }
                    }
                }

                candidates.push((idx, weighted_score));
            }
        }

        if candidates.is_empty() {
            evidence.add_confidence_factor(ConfidenceFactor::new(
                "no_language_found".to_string(),
                0.0,
                1.0,
                "No language patterns matched".to_string(),
            ));
            return None;
        }

        let best_idx = self.resolve_language_conflicts(languages, candidates, matched_files);

        evidence.add_confidence_factor(ConfidenceFactor::new(
            "selected_language".to_string(),
            1.0,
            1.0,
            format!(
                "Selected {} as best language match",
                languages[best_idx].name
            ),
        ));

        Some(languages[best_idx].clone())
    }

    pub fn resolve_language_conflicts(
        &self,
        languages: &[Arc<Indicator>],
        candidates: Vec<(usize, f32)>,
        matched_files: &[MatchedFile],
    ) -> usize {
        if candidates.len() == 1 {
            return candidates[0].0;
        }

        let mut enhanced_candidates: Vec<(usize, f32, f32)> = Vec::with_capacity(candidates.len());

        for (idx, base_score) in candidates {
            let language = &languages[idx];
            let context_bonus =
                self.confidence_scorer
                    .calculate_context_bonus(language, matched_files, languages);
            let quality_score =
                self.confidence_scorer
                    .calculate_quality_score(language, matched_files, languages);
            let enhanced_score = base_score + context_bonus;

            enhanced_candidates.push((idx, enhanced_score, quality_score));
        }

        enhanced_candidates
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if enhanced_candidates.len() >= 2 {
            let score_gap = enhanced_candidates[0].1 - enhanced_candidates[1].1;
            if score_gap > 0.3 {
                return enhanced_candidates[0].0;
            }
        }

        self.resolve_by_confidence_tiers(languages, enhanced_candidates)
    }

    fn resolve_by_confidence_tiers(
        &self,
        languages: &[Arc<Indicator>],
        candidates: Vec<(usize, f32, f32)>,
    ) -> usize {
        let high_confidence: Vec<_> = candidates
            .iter()
            .filter(|(_, score, _)| *score >= 0.8)
            .cloned()
            .collect();
        let medium_confidence: Vec<_> = candidates
            .iter()
            .filter(|(_, score, _)| *score >= 0.5 && *score < 0.8)
            .cloned()
            .collect();
        let low_confidence: Vec<_> = candidates
            .iter()
            .filter(|(_, score, _)| *score < 0.5)
            .cloned()
            .collect();

        if !high_confidence.is_empty() {
            self.resolve_by_score(languages, high_confidence)
        } else if !medium_confidence.is_empty() {
            self.resolve_by_priority_with_score_override(languages, medium_confidence)
        } else {
            self.resolve_by_priority(languages, low_confidence)
        }
    }

    fn resolve_by_score(
        &self,
        languages: &[Arc<Indicator>],
        candidates: Vec<(usize, f32, f32)>,
    ) -> usize {
        candidates
            .iter()
            .max_by(|a, b| {
                let score_cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                if score_cmp == std::cmp::Ordering::Equal {
                    let lang_a = &languages[a.0];
                    let lang_b = &languages[b.0];
                    lang_b.priority.cmp(&lang_a.priority)
                } else {
                    score_cmp
                }
            })
            .map(|(idx, _, _)| *idx)
            .unwrap_or(candidates[0].0)
    }

    fn resolve_by_priority_with_score_override(
        &self,
        languages: &[Arc<Indicator>],
        candidates: Vec<(usize, f32, f32)>,
    ) -> usize {
        let mut best_candidate = &candidates[0];

        for candidate in &candidates[1..] {
            let current_lang = &languages[best_candidate.0];
            let candidate_lang = &languages[candidate.0];

            if candidate_lang.priority < current_lang.priority {
                best_candidate = candidate;
            } else if candidate_lang.priority == current_lang.priority.saturating_add(1) {
                let score_diff = candidate.1 - best_candidate.1;
                if score_diff > 0.4 {
                    best_candidate = candidate;
                }
            } else if candidate_lang.priority == current_lang.priority
                && candidate.1 > best_candidate.1
            {
                best_candidate = candidate;
            }
        }

        best_candidate.0
    }

    fn resolve_by_priority(
        &self,
        languages: &[Arc<Indicator>],
        candidates: Vec<(usize, f32, f32)>,
    ) -> usize {
        candidates
            .iter()
            .min_by_key(|(idx, _, _)| languages[*idx].priority)
            .map(|(idx, _, _)| *idx)
            .unwrap_or(candidates[0].0)
    }
}

impl Default for IndicatorResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionEvidence;

    use crate::detection::matchers::test_helpers::helpers::{
        create_test_file, create_test_indicator_with_priority,
    };

    #[test]
    fn test_language_resolver_creation() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();

        let empty_languages = vec![];
        let empty_files = vec![];
        let mut evidence = DetectionEvidence::new();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &empty_languages,
            &empty_files,
            &mut evidence,
        );

        assert!(result.is_none());

        assert!(!evidence.confidence_factors.is_empty());
        Ok(())
    }

    #[test]
    fn test_detect_language_no_matches() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Rust",
                vec!["Cargo.toml", "*.rs"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                vec!["package.json", "*.js"],
                2,
            )),
        ];

        let files = vec![create_test_file("README.md", "README.md")];
        let mut evidence = DetectionEvidence::new();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &languages,
            &files,
            &mut evidence,
        );

        assert!(
            result.is_none(),
            "Should return None when no patterns match"
        );
        assert!(
            !evidence.confidence_factors.is_empty(),
            "Should add evidence for no match"
        );
        Ok(())
    }

    #[test]
    fn test_detect_language_single_match() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Rust",
                vec!["Cargo.toml", "*.rs"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                vec!["package.json", "*.js"],
                2,
            )),
        ];

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];
        let mut evidence = DetectionEvidence::new();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &languages,
            &files,
            &mut evidence,
        );

        assert!(result.is_some(), "Should detect a language");
        assert_eq!(
            result.ok_or("Failed to get language result")?.name,
            "Rust",
            "Should detect Rust"
        );
        assert!(
            !evidence.confidence_factors.is_empty(),
            "Should add evidence"
        );
        Ok(())
    }

    #[test]
    fn test_resolve_language_conflicts_single_candidate() -> Result<(), Box<dyn std::error::Error>>
    {
        let resolver = IndicatorResolver::new();
        let languages = vec![Arc::new(create_test_indicator_with_priority(
            "Rust",
            vec!["*.rs"],
            1,
        ))];
        let candidates = vec![(0, 0.8)];
        let files = vec![create_test_file("main.rs", "src/main.rs")];

        let result = resolver.resolve_language_conflicts(&languages, candidates, &files);
        assert_eq!(result, 0, "Should return the single candidate");
        Ok(())
    }

    #[test]
    fn test_resolve_language_conflicts_clear_winner() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority("Rust", vec!["*.rs"], 1)),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                vec!["*.js"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.9), (1, 0.5)];
        let files = vec![
            create_test_file("main.rs", "src/main.rs"),
            create_test_file("app.js", "src/app.js"),
        ];

        let result = resolver.resolve_language_conflicts(&languages, candidates, &files);
        assert_eq!(result, 0, "Should select clear winner (Rust)");
        Ok(())
    }

    #[test]
    fn test_resolve_by_score() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Lower Score",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Higher Score",
                vec!["*.b"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.7, 1.0), (1, 0.9, 1.0)];

        let result = resolver.resolve_by_score(&languages, candidates);
        assert_eq!(result, 1, "Should select language with higher score");
        Ok(())
    }

    #[test]
    fn test_resolve_by_score_tie_breaker() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Higher Priority",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Lower Priority",
                vec!["*.b"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.8, 1.0), (1, 0.8, 1.0)];

        let result = resolver.resolve_by_score(&languages, candidates);
        assert_eq!(result, 0, "Should use priority as tiebreaker");
        Ok(())
    }

    #[test]
    fn test_resolve_by_priority() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Lower Priority",
                vec!["*.a"],
                2,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Higher Priority",
                vec!["*.b"],
                1,
            )),
        ];
        let candidates = vec![(0, 0.7, 1.0), (1, 0.6, 1.0)];

        let result = resolver.resolve_by_priority(&languages, candidates);
        assert_eq!(result, 1, "Should select higher priority language");
        Ok(())
    }

    #[test]
    fn test_resolve_by_priority_with_score_override() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Higher Priority",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Lower Priority",
                vec!["*.b"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.5, 1.0), (1, 0.95, 1.0)];

        let result = resolver.resolve_by_priority_with_score_override(&languages, candidates);
        assert_eq!(
            result, 1,
            "Score should override priority when difference >0.4"
        );
        Ok(())
    }

    #[test]
    fn test_resolve_by_priority_with_score_no_override() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Higher Priority",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Lower Priority",
                vec!["*.b"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.6, 1.0), (1, 0.8, 1.0)];

        let result = resolver.resolve_by_priority_with_score_override(&languages, candidates);
        assert_eq!(result, 0, "Priority should win when score difference <0.4");
        Ok(())
    }

    #[test]
    fn test_confidence_tiers_high_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Medium",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority("High", vec!["*.b"], 2)),
        ];
        let candidates = vec![(0, 0.7, 1.0), (1, 0.9, 1.0)];

        let result = resolver.resolve_by_confidence_tiers(&languages, candidates);
        assert_eq!(result, 1, "High confidence should win");
        Ok(())
    }

    #[test]
    fn test_confidence_tiers_medium_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![
            Arc::new(create_test_indicator_with_priority(
                "Medium1",
                vec!["*.a"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "Medium2",
                vec!["*.b"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.6, 1.0), (1, 0.7, 1.0)];

        let result = resolver.resolve_by_confidence_tiers(&languages, candidates);
        assert!(result == 0 || result == 1);
        Ok(())
    }

    #[test]
    fn test_evidence_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::new();
        let languages = vec![Arc::new(create_test_indicator_with_priority(
            "Rust",
            vec!["Cargo.toml"],
            1,
        ))];
        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];
        let mut evidence = DetectionEvidence::new();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &languages,
            &files,
            &mut evidence,
        );

        assert!(result.is_some());
        assert!(
            !evidence.confidence_factors.is_empty(),
            "Should add confidence factors"
        );
        assert!(
            !evidence.indicator_evidence.is_empty(),
            "Should add language evidence"
        );
        Ok(())
    }
}
