use crate::detection::confidence_scorer::calculate_indicator_score;
use crate::patterns::matches_pattern;
use crate::types::{ConfidenceFactor, DetectionEvidence, EvidenceItem, Indicator, MatchedFile};
use std::sync::Arc;

/// Real source evidence for `indicator` at the scan root: a source file
/// (`*.ext` pattern) there, or a standard source directory
/// (`src/main/kotlin`, a path pattern). A file that is itself some
/// indicator's root marker does not count — `build.gradle.kts` is Kotlin
/// *build* configuration, not Kotlin sources, however well it matches
/// `*.kts`. A directory holding only `app.js` is a JavaScript directory,
/// and a bare `src/main/kotlin` says the same about a sources-only checkout.
pub(crate) fn root_source_evidence(
    indicator: &Indicator,
    matched_files: &[MatchedFile],
    all_indicators: &[Arc<Indicator>],
) -> bool {
    root_source_count(indicator, matched_files, all_indicators) > 0
}

/// How many distinct root source files (or probed source directories) back
/// `indicator`: five .py files beside one stray .js file are not a tie.
fn root_source_count(
    indicator: &Indicator,
    matched_files: &[MatchedFile],
    all_indicators: &[Arc<Indicator>],
) -> usize {
    let is_marker_name = |name: &str| {
        all_indicators
            .iter()
            .flat_map(|other| &other.root_indicators)
            .any(|root| !root.pattern.contains('/') && matches_pattern(name, &root.pattern))
    };
    matched_files
        .iter()
        .filter(|file| {
            indicator.files.iter().any(|pattern| {
                if pattern.contains('/') {
                    file.relative_path == *pattern
                } else if pattern.starts_with("*.") {
                    file.depth == 0
                        && matches_pattern(&file.filename, pattern)
                        && !is_marker_name(&file.filename)
                } else {
                    false
                }
            })
        })
        .count()
}

/// Resolves the markerless case: the scan root has no qualifying root
/// marker (content checks included), so only source files can make an
/// indicator a candidate — a marker *pattern* match whose content check
/// failed (a tool-only pyproject.toml) or a tool config identifies
/// nothing. Contested marker directories are arbitrated by the engine,
/// not here.
pub fn resolve_indicator(
    indicators: &[Arc<Indicator>],
    matched_files: &[MatchedFile],
    evidence: &mut DetectionEvidence,
) -> Option<Arc<Indicator>> {
    let mut candidates: Vec<(usize, f32, usize)> = Vec::with_capacity(indicators.len());

    for (idx, indicator) in indicators.iter().enumerate() {
        let weighted_score = calculate_indicator_score(indicator, matched_files, indicators);
        let sources = root_source_count(indicator, matched_files, indicators);
        if sources > 0 {
            evidence.add_confidence_factor(ConfidenceFactor::new(
                format!("indicator_{}", indicator.name),
                weighted_score,
                format!("Score for {} indicator detection", indicator.name),
            ));

            candidates.push((idx, weighted_score, sources));
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

    let best_idx = resolve_indicator_conflicts(candidates);

    let selected = &indicators[best_idx];
    for file in matched_files {
        for pattern in &selected.files {
            if matches_pattern(&file.filename, pattern) {
                let evidence_item = if pattern.contains("*.") {
                    EvidenceItem::indicator_file
                } else {
                    EvidenceItem::manifest_file
                };
                evidence.add_indicator_evidence(evidence_item(
                    file.relative_path.clone(),
                    pattern.clone(),
                    0.8,
                ));
            }
        }
    }

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

/// Markerless conflicts are rare (two languages' sources in one folder with
/// no manifest): the most root source files win (five .py beside one stray
/// .js is Python), then the higher ratio, then config order.
fn resolve_indicator_conflicts(candidates: Vec<(usize, f32, usize)>) -> usize {
    candidates
        .into_iter()
        .max_by(|a, b| {
            a.2.cmp(&b.2)
                .then(a.1.total_cmp(&b.1))
                // smaller index ranks higher, so a full tie keeps the
                // first indicator in config order
                .then(b.0.cmp(&a.0))
        })
        .map_or(0, |(idx, _, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionEvidence;

    use crate::detection::matchers::test_helpers::helpers::{
        create_test_file, create_test_indicator,
    };

    #[test]
    fn test_detect_indicator_no_matches() {
        let indicators = vec![
            Arc::new(create_test_indicator("Rust", &["Cargo.toml", "*.rs"])),
            Arc::new(create_test_indicator(
                "JavaScript",
                &["package.json", "*.js"],
            )),
        ];

        let files = vec![create_test_file("README.md", "README.md")];
        let mut evidence = DetectionEvidence::default();

        let result = resolve_indicator(&indicators, &files, &mut evidence);

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
        let indicators = vec![
            Arc::new(create_test_indicator("Rust", &["Cargo.toml", "*.rs"])),
            Arc::new(create_test_indicator(
                "JavaScript",
                &["package.json", "*.js"],
            )),
        ];

        // A root-level source file is what qualifies a candidate here: these
        // test indicators declare no root markers, and an exact-name config
        // file alone no longer creates a candidate.
        let files = vec![create_test_file("main.rs", "main.rs")];
        let mut evidence = DetectionEvidence::default();

        let result = resolve_indicator(&indicators, &files, &mut evidence);

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
        let candidates = vec![(0, 0.8, 0)];

        let result = resolve_indicator_conflicts(candidates);
        assert_eq!(result, 0, "Should return the single candidate");
    }

    #[test]
    fn test_resolve_indicator_conflicts_clear_winner() {
        let candidates = vec![(0, 0.9, 0), (1, 0.5, 0)];

        let result = resolve_indicator_conflicts(candidates);
        assert_eq!(result, 0, "Should select clear winner (Rust)");
    }

    /// Index chosen among candidates with the given `(config index, score)`
    fn pick(candidates: Vec<(usize, f32)>) -> usize {
        let candidates = candidates
            .into_iter()
            .map(|(idx, score)| (idx, score, 0))
            .collect();
        resolve_indicator_conflicts(candidates)
    }

    #[test]
    fn test_candidates_compete_on_score() {
        assert_eq!(pick(vec![(0, 0.7), (1, 0.9)]), 1);
    }

    #[test]
    fn test_more_root_source_files_win_before_the_ratio() {
        // five .py beside one stray .js: Python, whatever the ratios say
        let indicators = vec![
            Arc::new(create_test_indicator("Js", &["*.js"])),
            Arc::new(create_test_indicator("Py", &["*.py"])),
        ];
        let files: Vec<MatchedFile> = ["a.py", "b.py", "c.py", "d.py", "e.py", "x.js"]
            .iter()
            .map(|name| create_test_file(name, name))
            .collect();
        let mut evidence = DetectionEvidence::default();

        let winner = resolve_indicator(&indicators, &files, &mut evidence);

        assert_eq!(winner.map(|i| i.name.clone()).as_deref(), Some("Py"));
    }

    #[test]
    fn test_higher_score_wins_at_any_level_regardless_of_config_order() {
        // no confidence tiers: the ratio ranks even weak candidates, and a
        // later row with the higher score beats an earlier one
        assert_eq!(pick(vec![(0, 0.3), (1, 0.4)]), 1);
        assert_eq!(pick(vec![(0, 0.4), (1, 0.5)]), 1);
    }

    #[test]
    fn test_full_tie_picks_the_first_indicator_in_config_order() {
        // same score: config order decides
        assert_eq!(pick(vec![(0, 0.8), (1, 0.8)]), 0);
        assert_eq!(pick(vec![(0, 0.4), (1, 0.4)]), 0);
    }

    #[test]
    fn test_clear_leader_wins_regardless_of_config_order() {
        assert_eq!(pick(vec![(0, 0.4), (1, 0.95)]), 1);
    }

    #[test]
    fn test_wildcard_matched_files_recorded_as_evidence() {
        // Files matched via glob patterns must appear in indicator evidence:
        // the persistent cache builds its mtime-invalidation set from it
        let indicators = vec![Arc::new(create_test_indicator("Rust", &["*.rs"]))];
        let files = vec![create_test_file("main.rs", "main.rs")];
        let mut evidence = DetectionEvidence::default();

        let result = resolve_indicator(&indicators, &files, &mut evidence);

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

    #[test]
    fn test_only_the_selected_indicators_files_are_recorded_as_evidence() {
        let indicators = vec![
            Arc::new(create_test_indicator("Rust", &["*.rs"])),
            Arc::new(create_test_indicator("JavaScript", &["*.js"])),
        ];
        let files = vec![
            create_test_file("main.rs", "main.rs"),
            create_test_file("app.js", "app.js"),
        ];
        let mut evidence = DetectionEvidence::default();

        let winner = resolve_indicator(&indicators, &files, &mut evidence);

        assert_eq!(winner.map(|i| i.name.clone()), Some("Rust".to_string()));
        let recorded: Vec<&str> = evidence
            .indicator_evidence
            .iter()
            .map(|e| e.file_path.as_str())
            .collect();
        assert_eq!(recorded, ["main.rs"], "the loser's app.js is not evidence");
    }
}
