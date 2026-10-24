use crate::detection::confidence_scorer::ConfidenceScorer;
use crate::patterns::matches_pattern;
use crate::types::{
    ConfidenceFactor, DetectionEvidence, EvidenceItem, Framework, Indicator, MatchedFile,
};
use std::sync::Arc;

#[derive(Default)]
pub struct IndicatorResolver {
    confidence_scorer: ConfidenceScorer,
}

impl IndicatorResolver {
    /// A resolver that scores with the same framework catalog as the engine
    pub fn with_catalog(frameworks: Arc<[Framework]>) -> Self {
        Self {
            confidence_scorer: ConfidenceScorer::with_catalog(frameworks),
        }
    }

    pub fn detect_indicator_with_conflict_resolution_and_evidence(
        &self,
        indicators: &[Arc<Indicator>],
        matched_files: &[MatchedFile],
        evidence: &mut DetectionEvidence,
    ) -> Option<Arc<Indicator>> {
        let mut candidates: Vec<(usize, f32)> = Vec::with_capacity(indicators.len());

        for (idx, indicator) in indicators.iter().enumerate() {
            let weighted_score = self.confidence_scorer.calculate_indicator_score(
                indicator,
                matched_files,
                indicators,
            );
            if weighted_score > 0.1 {
                evidence.add_confidence_factor(ConfidenceFactor::new(
                    format!("indicator_{}", indicator.name),
                    weighted_score,
                    format!("Score for {} indicator detection", indicator.name),
                ));

                for file in matched_files {
                    for pattern in &indicator.files {
                        if matches_pattern(&file.filename, pattern) {
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
                "no_indicator_found".to_string(),
                0.0,
                "No indicator patterns matched".to_string(),
            ));
            return None;
        }

        let best_idx = self.resolve_indicator_conflicts(indicators, candidates, matched_files);

        evidence.add_confidence_factor(ConfidenceFactor::new(
            "selected_indicator".to_string(),
            1.0,
            format!(
                "Selected {} as best indicator match",
                indicators[best_idx].name
            ),
        ));

        Some(indicators[best_idx].clone())
    }

    fn resolve_indicator_conflicts(
        &self,
        indicators: &[Arc<Indicator>],
        candidates: Vec<(usize, f32)>,
        matched_files: &[MatchedFile],
    ) -> usize {
        if candidates.len() == 1 {
            return candidates[0].0;
        }

        let mut scored: Vec<(usize, f32)> = candidates
            .into_iter()
            .map(|(idx, base_score)| {
                let context_bonus = self.confidence_scorer.calculate_context_bonus(
                    &indicators[idx],
                    matched_files,
                    indicators,
                );
                (idx, base_score + context_bonus)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        if scored[0].1 - scored[1].1 > 0.3 {
            return scored[0].0;
        }

        // Reasonably confident candidates (>= 0.5) compete on score, with
        // priority only breaking ties; below that the evidence is too thin
        // to rank by score, so the declared priority decides
        // (Reversed so a full tie keeps the first indicator, as `min_by_key`
        // below does; `max_by` alone would pick the last.)
        let confident = scored
            .iter()
            .rev()
            .filter(|(_, score)| *score >= 0.5)
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(indicators[b.0].priority.cmp(&indicators[a.0].priority))
            });
        match confident {
            Some(&(idx, _)) => idx,
            None => scored
                .iter()
                .min_by_key(|(idx, _)| indicators[*idx].priority)
                .map_or(scored[0].0, |&(idx, _)| idx),
        }
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
    fn test_detect_indicator_no_matches() {
        let resolver = IndicatorResolver::default();
        let indicators = vec![
            Arc::new(create_test_indicator_with_priority(
                "Rust",
                &["Cargo.toml", "*.rs"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                &["package.json", "*.js"],
                2,
            )),
        ];

        let files = vec![create_test_file("README.md", "README.md")];
        let mut evidence = DetectionEvidence::default();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &indicators,
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
    }

    #[test]
    fn test_detect_indicator_single_match() -> Result<(), Box<dyn std::error::Error>> {
        let resolver = IndicatorResolver::default();
        let indicators = vec![
            Arc::new(create_test_indicator_with_priority(
                "Rust",
                &["Cargo.toml", "*.rs"],
                1,
            )),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                &["package.json", "*.js"],
                2,
            )),
        ];

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];
        let mut evidence = DetectionEvidence::default();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &indicators,
            &files,
            &mut evidence,
        );

        assert!(result.is_some(), "Should detect an indicator");
        assert_eq!(
            result.ok_or("Failed to get indicator result")?.name,
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
    fn test_resolve_indicator_conflicts_single_candidate() {
        let resolver = IndicatorResolver::default();
        let indicators = vec![Arc::new(create_test_indicator_with_priority(
            "Rust",
            &["*.rs"],
            1,
        ))];
        let candidates = vec![(0, 0.8)];
        let files = vec![create_test_file("main.rs", "src/main.rs")];

        let result = resolver.resolve_indicator_conflicts(&indicators, candidates, &files);
        assert_eq!(result, 0, "Should return the single candidate");
    }

    #[test]
    fn test_resolve_indicator_conflicts_clear_winner() {
        let resolver = IndicatorResolver::default();
        let indicators = vec![
            Arc::new(create_test_indicator_with_priority("Rust", &["*.rs"], 1)),
            Arc::new(create_test_indicator_with_priority(
                "JavaScript",
                &["*.js"],
                2,
            )),
        ];
        let candidates = vec![(0, 0.9), (1, 0.5)];
        let files = vec![
            create_test_file("main.rs", "src/main.rs"),
            create_test_file("app.js", "src/app.js"),
        ];

        let result = resolver.resolve_indicator_conflicts(&indicators, candidates, &files);
        assert_eq!(result, 0, "Should select clear winner (Rust)");
    }

    /// Index chosen among indicators with the given priorities (no files, so
    /// no context bonus: the candidate scores decide)
    fn pick(priorities: &[u8], candidates: Vec<(usize, f32)>) -> usize {
        let indicators: Vec<Arc<Indicator>> = priorities
            .iter()
            .map(|&priority| Arc::new(create_test_indicator_with_priority("x", &["*.x"], priority)))
            .collect();
        IndicatorResolver::default().resolve_indicator_conflicts(&indicators, candidates, &[])
    }

    #[test]
    fn test_confident_candidates_compete_on_score() {
        assert_eq!(pick(&[1, 2], vec![(0, 0.7), (1, 0.9)]), 1);
    }

    #[test]
    fn test_confident_tie_falls_back_to_priority() {
        assert_eq!(pick(&[2, 1], vec![(0, 0.8), (1, 0.8)]), 1);
    }

    #[test]
    fn test_medium_tier_prefers_higher_score_over_priority() {
        // 0.79 vs 0.51 must not lose to priority
        assert_eq!(pick(&[1, 2], vec![(0, 0.51), (1, 0.79)]), 1);
    }

    #[test]
    fn test_below_the_confident_tier_priority_decides() {
        assert_eq!(pick(&[2, 1], vec![(0, 0.4), (1, 0.3)]), 1);
    }

    #[test]
    fn test_candidate_exactly_at_the_confident_boundary_competes_on_score() {
        // 0.5 is confident; 0.4 is not: the score wins over better priority
        assert_eq!(pick(&[2, 1], vec![(0, 0.5), (1, 0.4)]), 0);
    }

    #[test]
    fn test_full_tie_picks_the_first_indicator_in_config_order() {
        // same score, same priority: config order decides, like below the
        // confident tier
        assert_eq!(pick(&[1, 1], vec![(0, 0.8), (1, 0.8)]), 0);
        assert_eq!(pick(&[1, 1], vec![(0, 0.4), (1, 0.4)]), 0);
    }

    #[test]
    fn test_clear_leader_wins_regardless_of_priority() {
        assert_eq!(pick(&[9, 1], vec![(0, 0.95), (1, 0.4)]), 0);
    }

    #[test]
    fn test_wildcard_matched_files_recorded_as_evidence() {
        // Files matched via glob patterns must appear in indicator evidence:
        // the persistent cache builds its mtime-invalidation set from it
        let resolver = IndicatorResolver::default();
        let indicators = vec![Arc::new(create_test_indicator_with_priority(
            "Rust",
            &["*.rs"],
            5,
        ))];
        let files = vec![create_test_file("main.rs", "main.rs")];
        let mut evidence = DetectionEvidence::default();

        let result = resolver.detect_indicator_with_conflict_resolution_and_evidence(
            &indicators,
            &files,
            &mut evidence,
        );

        assert!(result.is_some());
        assert!(
            evidence
                .indicator_evidence
                .iter()
                .any(|e| e.file_path.contains("main.rs")),
            "glob-matched file must be recorded as evidence, got: {:?}",
            evidence.indicator_evidence
        );
    }
}
