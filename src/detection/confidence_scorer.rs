use crate::detection::pattern_matching::PatternMatcher;
use crate::types::{
    ConfidenceFactor, DetectionEvidence, DirectoryType, Framework, Indicator, MatchedFile,
};
use std::collections::HashSet;
use std::sync::Arc;

pub struct ConfidenceScorer {
    pattern_matcher: Arc<PatternMatcher>,
    /// Framework catalog: framework root-indicator weights contribute to
    /// pattern importance
    frameworks: Arc<Vec<Framework>>,
}

impl ConfidenceScorer {
    pub fn new() -> Self {
        Self::with_pattern_matcher(Arc::new(PatternMatcher::new()))
    }

    pub fn with_pattern_matcher(pattern_matcher: Arc<PatternMatcher>) -> Self {
        Self::with_catalog(pattern_matcher, Arc::new(Vec::new()))
    }

    pub fn with_catalog(
        pattern_matcher: Arc<PatternMatcher>,
        frameworks: Arc<Vec<Framework>>,
    ) -> Self {
        Self {
            pattern_matcher,
            frameworks,
        }
    }

    pub fn get_pattern_importance(&self, pattern: &str, indicators: &[Arc<Indicator>]) -> f32 {
        for indicator in indicators {
            for root_indicator in &indicator.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return root_indicator.weight;
                }
            }
        }

        for framework in self.frameworks.iter() {
            for root_indicator in &framework.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return root_indicator.weight;
                }
            }
        }

        // Default weight for patterns not matching any root indicator
        0.5
    }

    pub fn calculate_indicator_score(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> f32 {
        if matched_files.is_empty() {
            return 0.0;
        }

        let mut weighted_score = 0.0;
        let mut max_possible_score = 0.0;

        for pattern in &indicator.files {
            let pattern_importance = self.get_pattern_importance(pattern, indicators);
            max_possible_score += pattern_importance;
        }

        for root_indicator in &indicator.root_indicators {
            max_possible_score += root_indicator.weight;
        }

        for pattern in &indicator.files {
            let pattern_importance = self.get_pattern_importance(pattern, indicators);

            let best_match_weight = matched_files
                .iter()
                .filter(|file| {
                    self.pattern_matcher
                        .matches_pattern(&file.filename, pattern)
                })
                .map(|file| file.weight())
                .fold(0.0f32, |a, b| a.max(b));

            if best_match_weight > 0.0 {
                weighted_score += best_match_weight * pattern_importance;
            }
        }

        let root_indicator_bonus = self.calculate_root_indicator_bonus(indicator, matched_files);
        weighted_score += root_indicator_bonus;

        if max_possible_score > 0.0 {
            (weighted_score / max_possible_score).min(1.0)
        } else {
            0.0
        }
    }

    /// Strongest root-indicator weight among the indicator's root indicators
    /// that match a file at the project root, or 0.0 when none match.
    ///
    /// Used to floor the displayed confidence: a matched high-weight root
    /// manifest (e.g. Cargo.toml at 0.95) identifies the project regardless
    /// of how few of the indicator's other patterns matched.
    pub fn strongest_root_match(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
    ) -> f32 {
        matched_files
            .iter()
            .filter(|file| file.depth == 0)
            .flat_map(|file| {
                indicator
                    .root_indicators
                    .iter()
                    .filter_map(|root_indicator| {
                        self.pattern_matcher
                            .matches_pattern(&file.filename, &root_indicator.pattern)
                            .then_some(root_indicator.weight)
                    })
            })
            .fold(0.0f32, f32::max)
    }

    pub fn calculate_indicator_score_with_evidence(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
        evidence: &mut DetectionEvidence,
        indicators: &[Arc<Indicator>],
    ) -> f32 {
        let score = self.calculate_indicator_score(indicator, matched_files, indicators);

        evidence.add_confidence_factor(ConfidenceFactor::new(
            "final_confidence".to_string(),
            score,
            1.0,
            format!("Final confidence score for {}", indicator.name),
        ));

        score
    }

    pub fn quick_termination_check(
        &self,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> Option<bool> {
        if matched_files.len() == 1 {
            let file = &matched_files[0];
            if file.depth == 0 && self.get_pattern_importance(&file.filename, indicators) >= 0.9 {
                return Some(true);
            }

            for indicator in indicators {
                for root_indicator in &indicator.root_indicators {
                    if self
                        .pattern_matcher
                        .matches_pattern(&file.filename, &root_indicator.pattern)
                        && root_indicator.weight >= 0.8
                    {
                        return Some(true);
                    }
                }
            }
        }

        if matched_files.len() >= 2 {
            let strong_root_indicators_count = matched_files
                .iter()
                .filter(|file| {
                    file.depth == 0
                        && self.get_pattern_importance(&file.filename, indicators) >= 0.9
                        && file.weight() >= 1.0
                })
                .count();

            if strong_root_indicators_count >= 2 {
                return Some(true);
            }
        }

        None
    }

    pub fn calculate_context_bonus(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> f32 {
        let mut bonus = 0.0;

        let root_files = matched_files
            .iter()
            .filter(|f| {
                f.depth == 0
                    && indicator
                        .files
                        .iter()
                        .any(|pattern| self.pattern_matcher.matches_pattern(&f.filename, pattern))
            })
            .count();

        if root_files > 0 {
            bonus += 0.1 * (root_files as f32).min(2.0);
        }

        let important_dirs: HashSet<String> = matched_files
            .iter()
            .filter(|f| {
                f.directory_type != DirectoryType::Test
                    && f.directory_type != DirectoryType::Dependencies
                    && indicator
                        .files
                        .iter()
                        .any(|pattern| self.pattern_matcher.matches_pattern(&f.filename, pattern))
            })
            .map(|f| f.relative_path.split('/').next().unwrap_or("").to_string())
            .collect();

        if important_dirs.len() >= 2 {
            bonus += 0.2;
        }

        let config_files = matched_files
            .iter()
            .filter(|f| {
                self.get_pattern_importance(&f.filename, indicators) >= 0.9
                    && indicator
                        .files
                        .iter()
                        .any(|pattern| self.pattern_matcher.matches_pattern(&f.filename, pattern))
            })
            .count();

        if config_files > 0 {
            bonus += 0.1 * (config_files as f32).min(1.0);
        }

        bonus.min(0.3)
    }

    fn calculate_root_indicator_bonus(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
    ) -> f32 {
        let mut bonus = 0.0;

        for root_indicator in &indicator.root_indicators {
            let has_matching_file = matched_files.iter().any(|file| {
                file.depth == 0
                    && self
                        .pattern_matcher
                        .matches_pattern(&file.filename, &root_indicator.pattern)
            });

            if has_matching_file {
                bonus += root_indicator.weight;
            }
        }

        bonus
    }

    pub fn calculate_quality_score(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> f32 {
        let mut quality_score = 0.0;

        for file in matched_files {
            if !indicator.files.iter().any(|pattern| {
                self.pattern_matcher
                    .matches_pattern(&file.filename, pattern)
            }) {
                continue;
            }

            let base_score = file.weight();
            let pattern_importance = self.get_pattern_importance(&file.filename, indicators);

            let quality_multiplier = match (file.depth, &file.directory_type) {
                (0, _) if pattern_importance >= 0.9 => 2.0,
                (0..=1, DirectoryType::Source) => 1.5,
                (_, _) if pattern_importance >= 0.8 => 1.2,
                _ => 1.0,
            };

            quality_score += base_score * pattern_importance * quality_multiplier;
        }

        quality_score
    }

    pub fn should_terminate_early(
        &mut self,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> bool {
        // First, check quick termination conditions
        if let Some(should_terminate) = self.quick_termination_check(matched_files, indicators) {
            return should_terminate;
        }

        // Additional extended checks for edge cases
        self.calculate_extended_termination(matched_files, indicators)
    }

    /// Extended termination logic for cases not covered by quick checks.
    ///
    /// This method performs more expensive operations like confidence score
    /// calculations and is only called after quick checks fail.
    fn calculate_extended_termination(
        &mut self,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> bool {
        // Check if we have enough evidence with moderate confidence scores
        if matched_files.len() >= 2 {
            let has_strong_indicators = self.has_strong_indicators(matched_files, indicators);

            if has_strong_indicators
                && self.check_confidence_thresholds(
                    matched_files,
                    indicators,
                    has_strong_indicators,
                )
            {
                return true;
            }
        }

        // For 5+ matched files, use lower confidence threshold
        if matched_files.len() >= 5 {
            for indicator in indicators {
                let confidence =
                    self.calculate_indicator_score(indicator, matched_files, indicators);
                if confidence >= 0.6 {
                    return true;
                }
            }
        }

        // Hard limit: always terminate after 12 matches
        matched_files.len() >= 12
    }

    /// Checks if matched files contain strong indicators (root files with high importance).
    fn has_strong_indicators(
        &self,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
    ) -> bool {
        // Check for pattern-based strong indicators
        let has_pattern_indicators = matched_files.iter().any(|file| {
            file.depth == 0
                && self.get_pattern_importance(&file.filename, indicators) >= 0.9
                && file.weight() >= 1.0
        });

        if has_pattern_indicators {
            return true;
        }

        // Check for root indicator matches
        for indicator in indicators {
            for root_indicator in &indicator.root_indicators {
                let has_matching_root_file = matched_files.iter().any(|file| {
                    file.depth == 0
                        && self
                            .pattern_matcher
                            .matches_pattern(&file.filename, &root_indicator.pattern)
                });

                if has_matching_root_file && root_indicator.weight >= 0.8 {
                    return true;
                }
            }
        }

        false
    }

    /// Checks if any indicator meets confidence thresholds for early termination.
    fn check_confidence_thresholds(
        &mut self,
        matched_files: &[MatchedFile],
        indicators: &[Arc<Indicator>],
        has_strong_indicators: bool,
    ) -> bool {
        for indicator in indicators {
            let confidence = self.calculate_indicator_score(indicator, matched_files, indicators);

            // Very high confidence always terminates
            if confidence >= 0.9 {
                return true;
            }

            // High confidence with strong indicators
            if confidence >= 0.7 && has_strong_indicators {
                return true;
            }

            // Moderate confidence with very strong indicators
            if confidence >= 0.6 && has_strong_indicators {
                return true;
            }
        }

        false
    }
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        detection::confidence_scorer::ConfidenceScorer,
        types::{DetectionEvidence, IndicatorContext},
        Indicator,
    };

    use crate::detection::matchers::test_helpers::helpers::{
        create_test_file, create_test_indicator,
    };

    fn create_test_indicators() -> Vec<Arc<Indicator>> {
        vec![Arc::new(create_test_indicator(
            "Rust",
            vec!["Cargo.toml", "*.rs"],
        ))]
    }

    #[test]
    fn test_basic_indicator_scoring() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("main.rs", "src/main.rs"),
        ];

        let score = scorer.calculate_indicator_score(&rust_lang, &files, &create_test_indicators());
        assert!(score > 0.0, "Should have positive score for matching files");
        assert!(score <= 1.0, "Score should not exceed 1.0");
        Ok(())
    }

    #[test]
    fn test_empty_files_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let score = scorer.calculate_indicator_score(&rust_lang, &[], &create_test_indicators());
        assert_eq!(score, 0.0, "Empty files should return zero score");
        Ok(())
    }

    #[test]
    fn test_context_bonus_for_root_files() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let bonus = scorer.calculate_context_bonus(&rust_lang, &files, &create_test_indicators());
        assert!(bonus > 0.0, "Should have bonus for root files");
        assert!(bonus <= 0.3, "Bonus should be capped at 0.3");
        Ok(())
    }

    #[test]
    fn test_quality_score_calculation() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("main.rs", "src/main.rs"),
        ];

        let quality = scorer.calculate_quality_score(&rust_lang, &files, &create_test_indicators());
        assert!(quality > 0.0, "Should have positive quality score");
        Ok(())
    }

    #[test]
    fn test_early_termination_single_important_file() -> Result<(), Box<dyn std::error::Error>> {
        use crate::detection::matchers::test_helpers::helpers::create_test_indicator_with_indicators;

        let mut scorer = ConfidenceScorer::new();
        let indicators = vec![Arc::new(create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.9)],
        ))];

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let should_terminate = scorer.should_terminate_early(&files, &indicators);
        assert!(
            should_terminate,
            "Should terminate early for important root file"
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_high_confidence() -> Result<(), Box<dyn std::error::Error>> {
        use crate::detection::matchers::test_helpers::helpers::create_test_indicator_with_indicators;

        let mut scorer = ConfidenceScorer::new();
        let indicators = vec![Arc::new(create_test_indicator_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.9), ("*.lock", 0.9)],
        ))];

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("Cargo.lock", "Cargo.lock"),
        ];

        let should_terminate = scorer.should_terminate_early(&files, &indicators);

        assert!(
            should_terminate,
            "Should terminate early with high-confidence Rust files"
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_many_files() -> Result<(), Box<dyn std::error::Error>> {
        let mut scorer = ConfidenceScorer::new();
        let indicators = vec![Arc::new(create_test_indicator("JavaScript", vec!["*.js"]))];

        let files: Vec<_> = (0..15)
            .map(|i| create_test_file(&format!("file{}.js", i), &format!("src/file{}.js", i)))
            .collect();

        let should_terminate = scorer.should_terminate_early(&files, &indicators);
        assert!(should_terminate, "Should terminate early for many files");
        Ok(())
    }

    #[test]
    fn test_evidence_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml"]));
        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let mut evidence = DetectionEvidence::new();
        let score = scorer.calculate_indicator_score_with_evidence(
            &rust_lang,
            &files,
            &mut evidence,
            &create_test_indicators(),
        );

        assert!(score > 0.0);
        assert!(
            !evidence.confidence_factors.is_empty(),
            "Should add confidence factors"
        );
        Ok(())
    }

    #[test]
    fn test_root_indicator_bonus() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(Indicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![crate::types::RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 0.9,
                context: IndicatorContext::LanguageRoot,
            }],
        ));

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("main.rs", "src/main.rs"),
        ];

        let score = scorer.calculate_indicator_score(&rust_lang, &files, &create_test_indicators());
        assert!(
            score > 0.0,
            "Should have positive score with root indicator"
        );
        assert!(score <= 1.0, "Score should not exceed 1.0");
        Ok(())
    }

    #[test]
    fn test_root_indicator_no_match() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(Indicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![crate::types::RootIndicator {
                pattern: "package.json".to_string(),
                weight: 0.9,
                context: IndicatorContext::LanguageRoot,
            }],
        ));

        let files = vec![create_test_file("main.rs", "src/main.rs")];

        let score = scorer.calculate_indicator_score(&rust_lang, &files, &create_test_indicators());
        assert!(score > 0.0, "Should still have score from regular files");
        assert!(
            score < 1.0,
            "Score should be less than 1.0 without root indicator"
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_with_root_indicator() -> Result<(), Box<dyn std::error::Error>> {
        let mut scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(Indicator::with_root_indicators(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![crate::types::RootIndicator {
                pattern: "Cargo.toml".to_string(),
                weight: 0.9,
                context: IndicatorContext::LanguageRoot,
            }],
        ));

        let indicators = vec![rust_lang];
        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let should_terminate = scorer.should_terminate_early(&files, &indicators);
        assert!(
            should_terminate,
            "Should terminate early with strong root indicator"
        );
        Ok(())
    }
}
