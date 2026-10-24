use crate::patterns::matches_pattern;
use crate::types::{
    ConfidenceFactor, DetectionEvidence, DirectoryType, Framework, Indicator, MatchedFile,
    RootIndicator,
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
pub struct ConfidenceScorer {
    /// Framework catalog: framework root-indicator weights contribute to
    /// pattern importance
    frameworks: Arc<Vec<Framework>>,
}

impl ConfidenceScorer {
    pub fn with_catalog(frameworks: Arc<Vec<Framework>>) -> Self {
        Self { frameworks }
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
        indicators
            .iter()
            .flat_map(|indicator| &indicator.root_indicators)
            .chain(self.frameworks.iter().flat_map(|f| &f.root_indicators))
            .find(|root_indicator| matches_pattern(pattern, &root_indicator.pattern))
            .map_or((0.5, 1.0, None), |root_indicator| {
                (
                    root_indicator.weight,
                    root_indicator.context.base_priority(),
                    root_indicator.alternative_group.clone(),
                )
            })
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

        // Declared file patterns sharing an alternative_group (e.g. Python's
        // poetry.lock vs uv.lock) contribute their max, not their sum — the
        // same rule as for root indicators, and for the same reason: these
        // patterns are typically ALSO declared as root indicators, so the
        // denominator inflates unless both sides respect the grouping.
        let patterns: Vec<(&String, f32, Option<String>)> = indicator
            .files
            .iter()
            .map(|pattern| {
                let (weight, priority, group) = self.get_pattern_details(pattern, indicators);
                (pattern, weight * priority, group)
            })
            .collect();

        let max_possible_score =
            sum_with_grouped_max(
                patterns
                    .iter()
                    .map(|(_, importance, group)| (group.as_deref(), *importance)),
            ) + sum_with_grouped_max(weighted(indicator.root_indicators.iter()));

        let weighted_score =
            sum_with_grouped_max(patterns.iter().filter_map(|(pattern, importance, group)| {
                let best_match_weight = matched_files
                    .iter()
                    .filter(|file| matches_pattern(&file.filename, pattern))
                    .map(|file| file.weight())
                    .fold(0.0f32, f32::max);
                (best_match_weight > 0.0)
                    .then(|| (group.as_deref(), best_match_weight * importance))
            })) + self.calculate_root_indicator_bonus(indicator, matched_files);

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
                    .filter(|root_indicator| {
                        matches_pattern(&file.filename, &root_indicator.pattern)
                    })
                    .map(|root_indicator| root_indicator.weight)
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
        let is_indicator_file = |f: &MatchedFile| {
            indicator
                .files
                .iter()
                .any(|pattern| matches_pattern(&f.filename, pattern))
        };
        let mut bonus = 0.0;

        let root_files = matched_files
            .iter()
            .filter(|f| f.depth == 0 && is_indicator_file(f))
            .count();
        if root_files > 0 {
            bonus += 0.1 * (root_files as f32).min(2.0);
        }

        let important_dirs: HashSet<&str> = matched_files
            .iter()
            .filter(|f| {
                f.directory_type != DirectoryType::Test
                    && f.directory_type != DirectoryType::Dependencies
                    && is_indicator_file(f)
            })
            .map(|f| f.relative_path.split('/').next().unwrap_or(""))
            .collect();
        if important_dirs.len() >= 2 {
            bonus += 0.2;
        }

        let has_config_file = matched_files.iter().any(|f| {
            self.get_pattern_details(&f.filename, indicators).0 >= 0.9 && is_indicator_file(f)
        });
        if has_config_file {
            bonus += 0.1;
        }

        bonus.min(0.3)
    }

    /// Same grouping rule as the denominator (see `calculate_indicator_score`):
    /// matching two alternatives from the same group (e.g. a mid-migration
    /// repo with both bun.lock and bun.lockb) shouldn't double-count.
    fn calculate_root_indicator_bonus(
        &self,
        indicator: &Arc<Indicator>,
        matched_files: &[MatchedFile],
    ) -> f32 {
        sum_with_grouped_max(weighted(indicator.root_indicators.iter().filter(
            |root_indicator| {
                matched_files.iter().any(|file| {
                    file.depth == 0 && matches_pattern(&file.filename, &root_indicator.pattern)
                })
            },
        )))
    }
}

/// `(alternative group, weight × context authority)` of each root indicator
fn weighted<'a>(
    root_indicators: impl Iterator<Item = &'a RootIndicator>,
) -> impl Iterator<Item = (Option<&'a str>, f32)> {
    root_indicators.map(|root_indicator| {
        (
            root_indicator.alternative_group.as_deref(),
            root_indicator.weight * root_indicator.context.base_priority(),
        )
    })
}

/// Sums the weights, except entries sharing an alternative group
/// (mutually-exclusive choices, e.g. Java's pom.xml vs build.gradle — a real
/// project uses exactly one) contribute only their max within the group.
/// Ungrouped entries (a manifest and its lockfile legitimately co-occur) sum.
fn sum_with_grouped_max<'a>(entries: impl Iterator<Item = (Option<&'a str>, f32)>) -> f32 {
    let mut ungrouped_sum = 0.0_f32;
    // Sorted: f32 addition is order-sensitive, scores must not vary per run
    let mut grouped_max: BTreeMap<&str, f32> = BTreeMap::new();

    for (group, weight) in entries {
        match group {
            Some(group) => {
                let entry = grouped_max.entry(group).or_insert(0.0);
                *entry = entry.max(weight);
            }
            None => ungrouped_sum += weight,
        }
    }

    ungrouped_sum + grouped_max.values().sum::<f32>()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_framework_root_indicators_lend_weight_to_matched_config_files() {
        use crate::detection::matchers::test_helpers::helpers::{
            create_test_file, create_test_indicator,
        };
        use crate::types::{DetectionType, Ecosystem, Framework, RootIndicator};

        let nextjs = Framework {
            name: "Next.js".to_string(),
            ecosystems: vec![Ecosystem::Npm],
            detection: DetectionType::Dependencies {
                dependencies: vec!["next".to_string()],
            },
            icon: None,
            color: None,
            priority: 1,
            root_indicators: vec![RootIndicator {
                pattern: "next.config.js".to_string(),
                weight: 0.9,
                context: IndicatorContext::FrameworkRoot,
                alternative_group: None,
            }],
        };
        let javascript = Arc::new(create_test_indicator("JavaScript", vec!["*.js"]));
        let files = [create_test_file("next.config.js", "next.config.js")];
        let indicators = [javascript.clone()];

        let without =
            ConfidenceScorer::default().calculate_context_bonus(&javascript, &files, &indicators);
        let with = ConfidenceScorer::with_catalog(Arc::new(vec![nextjs])).calculate_context_bonus(
            &javascript,
            &files,
            &indicators,
        );

        assert!(with > without, "{with} should exceed {without}");
    }

    #[test]
    fn test_grouped_sum_does_not_depend_on_hash_order() {
        // f32 addition is not associative: 2^24 swallows a lone +1, so the
        // total reveals the summation order
        let entries = || {
            [("b", 16_777_216.0_f32), ("a", 1.0), ("c", 1.0)]
                .into_iter()
                .map(|(group, weight)| (Some(group), weight))
        };
        let first = super::sum_with_grouped_max(entries());
        for _ in 0..200 {
            assert_eq!(super::sum_with_grouped_max(entries()), first);
        }
    }

    use std::sync::Arc;

    use crate::{
        detection::confidence_scorer::ConfidenceScorer,
        types::{Indicator, IndicatorContext},
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
    fn test_empty_files_returns_zero() {
        let scorer = ConfidenceScorer::default();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let score = scorer.calculate_indicator_score(&rust_lang, &[], &create_test_indicators());
        assert_eq!(score, 0.0, "Empty files should return zero score");
    }

    #[test]
    fn test_context_bonus_for_root_files() {
        let scorer = ConfidenceScorer::default();
        let rust_lang = Arc::new(create_test_indicator("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let bonus = scorer.calculate_context_bonus(&rust_lang, &files, &create_test_indicators());
        assert!(bonus > 0.0, "Should have bonus for root files");
        assert!(bonus <= 0.3, "Bonus should be capped at 0.3");
    }

    #[test]
    fn test_root_indicator_no_match() {
        let scorer = ConfidenceScorer::default();
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
    }

    #[test]
    fn test_grouped_alternatives_are_maxed_not_summed() {
        let scorer = ConfidenceScorer::default();
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
    }

    #[test]
    fn test_ungrouped_root_indicators_still_sum() {
        let scorer = ConfidenceScorer::default();
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
    }

    #[test]
    fn test_pattern_weight_and_priority() {
        let scorer = ConfidenceScorer::default();
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
    fn test_files_loop_respects_alternative_group() {
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
        let scorer = ConfidenceScorer::default();
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
    }
}
