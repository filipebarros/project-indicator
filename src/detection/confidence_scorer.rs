use crate::detection::pattern_matching::PatternMatcher;
use crate::types::{
    ConfidenceFactor, DetectionEvidence, DirectoryType, Framework, Indicator, MatchedFile,
    RootIndicator,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct ConfidenceScorer {
    pattern_matcher: Arc<PatternMatcher>,
    /// Framework catalog: framework root-indicator weights contribute to
    /// pattern importance
    frameworks: Arc<Vec<Framework>>,
}

impl ConfidenceScorer {
    pub fn new() -> Self {
        Self::with_catalog(Arc::new(PatternMatcher::new()), Arc::new(Vec::new()))
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

    /// The weight, context authority, and alternative-group (if any) of the
    /// root indicator declaring `pattern`. Defaults to `(0.5, 1.0, None)`
    /// (neutral, ungrouped) when no indicator/framework declares this
    /// pattern as a root indicator at all — true for most plain
    /// source-extension globs.
    fn get_pattern_details(
        &self,
        pattern: &str,
        indicators: &[Arc<Indicator>],
    ) -> (f32, f32, Option<String>) {
        for indicator in indicators {
            for root_indicator in &indicator.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return (
                        root_indicator.weight,
                        root_indicator.context.base_priority(),
                        root_indicator.alternative_group.clone(),
                    );
                }
            }
        }

        for framework in self.frameworks.iter() {
            for root_indicator in &framework.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return (
                        root_indicator.weight,
                        root_indicator.context.base_priority(),
                        root_indicator.alternative_group.clone(),
                    );
                }
            }
        }

        (0.5, 1.0, None)
    }

    pub fn get_pattern_importance(&self, pattern: &str, indicators: &[Arc<Indicator>]) -> f32 {
        self.get_pattern_details(pattern, indicators).0
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

        // Declared file patterns sharing an alternative_group (e.g. Python's
        // poetry.lock vs uv.lock, both looked up via the same root-indicator
        // metadata as the loop below) contribute their max, not their sum —
        // same rule as root indicators, and for the same reason: these
        // patterns are typically ALSO declared as root indicators, so this
        // loop must respect the same grouping or the denominator inflates
        // right back up regardless of what the root-indicators loop does.
        let mut file_grouped_max: HashMap<String, f32> = HashMap::new();
        for pattern in &indicator.files {
            let (weight, priority, group) = self.get_pattern_details(pattern, indicators);
            let importance = weight * priority;
            match group {
                Some(g) => {
                    let entry = file_grouped_max.entry(g).or_insert(0.0);
                    *entry = entry.max(importance);
                }
                None => max_possible_score += importance,
            }
        }
        max_possible_score += file_grouped_max.values().sum::<f32>();

        // Root indicators sharing an alternative_group are mutually-exclusive
        // choices (e.g. Java's pom.xml vs build.gradle) — a real project uses
        // exactly one, so the group contributes its max weight rather than
        // the sum of every alternative. Ungrouped root indicators (the
        // common case: a manifest and its lockfile, which legitimately
        // co-occur) keep summing as before.
        max_possible_score += Self::sum_with_grouped_max(indicator.root_indicators.iter());

        let mut file_grouped_contribution: HashMap<String, f32> = HashMap::new();
        for pattern in &indicator.files {
            let (weight, priority, group) = self.get_pattern_details(pattern, indicators);
            let importance = weight * priority;

            let best_match_weight = matched_files
                .iter()
                .filter(|file| {
                    self.pattern_matcher
                        .matches_pattern(&file.filename, pattern)
                })
                .map(|file| file.weight())
                .fold(0.0f32, |a, b| a.max(b));

            if best_match_weight <= 0.0 {
                continue;
            }

            let contribution = best_match_weight * importance;
            match group {
                Some(g) => {
                    let entry = file_grouped_contribution.entry(g).or_insert(0.0);
                    *entry = entry.max(contribution);
                }
                None => weighted_score += contribution,
            }
        }
        weighted_score += file_grouped_contribution.values().sum::<f32>();

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
        // Same grouping rule as the denominator (see calculate_indicator_score):
        // matching two alternatives from the same group (e.g. a mid-migration
        // repo with both bun.lock and bun.lockb) shouldn't double-count.
        Self::sum_with_grouped_max(indicator.root_indicators.iter().filter(|root_indicator| {
            matched_files.iter().any(|file| {
                file.depth == 0
                    && self
                        .pattern_matcher
                        .matches_pattern(&file.filename, &root_indicator.pattern)
            })
        }))
    }

    /// Sums `weight * context.base_priority()` across `root_indicators`,
    /// except root indicators sharing an `alternative_group` contribute only
    /// their max within that group (mutually-exclusive alternatives), not
    /// their sum.
    fn sum_with_grouped_max<'a>(root_indicators: impl Iterator<Item = &'a RootIndicator>) -> f32 {
        let mut ungrouped_sum = 0.0_f32;
        let mut grouped_max: HashMap<&str, f32> = HashMap::new();

        for root_indicator in root_indicators {
            let weighted = root_indicator.weight * root_indicator.context.base_priority();
            match root_indicator.alternative_group.as_deref() {
                Some(group) => {
                    let entry = grouped_max.entry(group).or_insert(0.0);
                    *entry = entry.max(weighted);
                }
                None => ungrouped_sum += weighted,
            }
        }

        ungrouped_sum + grouped_max.values().sum::<f32>()
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
                alternative_group: None,
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
                alternative_group: None,
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
    fn test_grouped_alternatives_are_maxed_not_summed() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        // "a" and "b" are alternatives in group "g" (e.g. pom.xml vs
        // build.gradle); "c" is an ungrouped, independent signal. Only "a"
        // and "c" are present — "b" (the sibling alternative) is absent.
        let indicator = Arc::new(Indicator::with_root_indicators(
            "Test".to_string(),
            vec![],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![
                crate::types::RootIndicator {
                    pattern: "a".to_string(),
                    weight: 0.9,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: Some("g".to_string()),
                },
                crate::types::RootIndicator {
                    pattern: "b".to_string(),
                    weight: 0.5,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: Some("g".to_string()),
                },
                crate::types::RootIndicator {
                    pattern: "c".to_string(),
                    weight: 0.3,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: None,
                },
            ],
        ));

        let files = vec![create_test_file("a", "a"), create_test_file("c", "c")];

        let score = scorer.calculate_indicator_score(&indicator, &files, &create_test_indicators());
        // Denominator: max(0.9, 0.5) + 0.3 = 1.2. Numerator (root indicator
        // bonus): 0.9 (group max, "a" matched) + 0.3 ("c" matched) = 1.2.
        // Missing "b" — the sibling alternative — should NOT depress the
        // score at all, since "a" already covers that group.
        assert!(
            (score - 1.0).abs() < 0.001,
            "Matching one alternative in a group should not be penalized for a missing sibling, got {score}"
        );
        Ok(())
    }

    #[test]
    fn test_ungrouped_root_indicators_still_sum() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        // Regression guard: co-occurring evidence (e.g. a manifest and its
        // lockfile) must keep summing into the denominator, not be maxed,
        // or a project missing the second file would be scored as if it
        // were fully confident.
        let indicator = Arc::new(Indicator::with_root_indicators(
            "Test".to_string(),
            vec![],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![
                crate::types::RootIndicator {
                    pattern: "manifest".to_string(),
                    weight: 0.95,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: None,
                },
                crate::types::RootIndicator {
                    pattern: "lockfile".to_string(),
                    weight: 0.9,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: None,
                },
            ],
        ));

        let files = vec![create_test_file("manifest", "manifest")];

        let score = scorer.calculate_indicator_score(&indicator, &files, &create_test_indicators());
        // 0.95 / (0.95 + 0.9) ≈ 0.514 — the missing lockfile should still
        // depress the score, since these are not alternatives.
        assert!(
            (score - 0.5135).abs() < 0.01,
            "Ungrouped root indicators should still sum in the denominator, got {score}"
        );
        Ok(())
    }

    #[test]
    fn test_pattern_weight_and_priority() {
        let scorer = ConfidenceScorer::new();
        let indicators = vec![Arc::new(Indicator::with_root_indicators(
            "Test".to_string(),
            vec![],
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
            vec![
                crate::types::RootIndicator {
                    pattern: "source.lang".to_string(),
                    weight: 0.9,
                    context: IndicatorContext::LanguageRoot,
                    alternative_group: None,
                },
                crate::types::RootIndicator {
                    pattern: "build.tool".to_string(),
                    weight: 0.95,
                    context: IndicatorContext::BuildSystem,
                    alternative_group: None,
                },
            ],
        ))];

        let (weight, priority, group) = scorer.get_pattern_details("source.lang", &indicators);
        assert!((weight - 0.9).abs() < 0.001);
        assert!((priority - 0.9).abs() < 0.001);
        assert_eq!(group, None);

        let (weight, priority, group) = scorer.get_pattern_details("build.tool", &indicators);
        assert!((weight - 0.95).abs() < 0.001);
        assert!((priority - 0.7).abs() < 0.001);
        assert_eq!(group, None);

        // A pattern with no declared root indicator is unaffected (neutral).
        let (weight, priority, group) = scorer.get_pattern_details("*.unrelated", &indicators);
        assert!((weight - 0.5).abs() < 0.001);
        assert!((priority - 1.0).abs() < 0.001);
        assert_eq!(group, None);
    }

    #[test]
    fn test_files_loop_respects_alternative_group() -> Result<(), Box<dyn std::error::Error>> {
        // Regression guard: the root_indicators loop and
        // calculate_root_indicator_bonus respect
        // alternative_group, but calculate_indicator_score's separate loop
        // over `indicator.files` (used for glob-matching) looked up
        // importance via get_pattern_details without ever checking the
        // returned group — so a declared-but-absent alternative (e.g.
        // Python's uv.lock when poetry.lock is present) still inflated the
        // denominator through this second loop, undoing the fix. Compare a
        // grouped indicator against an otherwise-identical ungrouped one:
        // matching one alternative out of two should score BETTER when
        // grouped (the missing sibling isn't penalized) and worse when not.
        let scorer = ConfidenceScorer::new();
        let build = |grouped: bool| {
            let group = |g: &str| {
                if grouped {
                    Some(g.to_string())
                } else {
                    None
                }
            };
            Arc::new(Indicator::with_root_indicators(
                "Test".to_string(),
                vec!["a".to_string(), "b".to_string()],
                "#FF0000".to_string(),
                "🔥".to_string(),
                1,
                vec![],
                vec![
                    crate::types::RootIndicator {
                        pattern: "a".to_string(),
                        weight: 0.8,
                        context: IndicatorContext::LanguageRoot,
                        alternative_group: group("g"),
                    },
                    crate::types::RootIndicator {
                        pattern: "b".to_string(),
                        weight: 0.6,
                        context: IndicatorContext::LanguageRoot,
                        alternative_group: group("g"),
                    },
                ],
            ))
        };

        let files = vec![create_test_file("a", "a")];
        let grouped_indicator = build(true);
        let ungrouped_indicator = build(false);

        // Each call's `indicators` list contains only that same indicator,
        // so get_pattern_details resolves "a"/"b" to its own declaration —
        // putting both indicators in one shared list would let whichever
        // comes first in iteration order shadow the other's group metadata.
        let grouped_score = scorer.calculate_indicator_score(
            &grouped_indicator,
            &files,
            std::slice::from_ref(&grouped_indicator),
        );
        let ungrouped_score = scorer.calculate_indicator_score(
            &ungrouped_indicator,
            &files,
            std::slice::from_ref(&ungrouped_indicator),
        );

        assert!(
            grouped_score > ungrouped_score,
            "Matching one alternative should score higher when the sibling is grouped (not penalized as missing evidence): grouped={grouped_score}, ungrouped={ungrouped_score}"
        );
        Ok(())
    }
}
