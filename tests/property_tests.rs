use project_indicator::config::TemplateGenerator;
use project_indicator::detection::confidence_scorer::ConfidenceScorer;
use project_indicator::patterns::matches_pattern;
use project_indicator::types::{Indicator, MatchedFile};
use proptest::prelude::*;
use std::sync::{Arc, LazyLock};

/// Naive recursive glob (`*` = any run of characters): the specification
/// `matches_pattern` must agree with.
fn reference_glob(text: &[char], pattern: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|i| reference_glob(&text[i..], rest)),
        Some((c, rest)) => text.first() == Some(c) && reference_glob(&text[1..], rest),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Invariant: `matches_pattern` agrees with a naive glob on every input,
    /// whatever the pattern's shape (exact, `*.ext`, `prefix*`, general).
    /// The alphabet includes `.`, `*` and a multibyte character so the
    /// extension/prefix shortcuts and char-boundary handling are exercised.
    #[test]
    fn test_matches_pattern_agrees_with_reference_glob(
        text in "[ab.ü]{0,8}",
        pattern in "[ab.ü*]{0,7}",
    ) {
        let t: Vec<char> = text.chars().collect();
        let p: Vec<char> = pattern.chars().collect();
        prop_assert_eq!(
            matches_pattern(&text, &pattern),
            reference_glob(&t, &p),
            "text={:?} pattern={:?}", text, pattern
        );
    }
}

/// The full template's indicators and scorer, built once for all cases
fn scoring_fixture() -> &'static (Vec<Arc<Indicator>>, ConfidenceScorer) {
    static FIXTURE: LazyLock<(Vec<Arc<Indicator>>, ConfidenceScorer)> = LazyLock::new(|| {
        let config = match TemplateGenerator::generate_template(Some("full")) {
            Ok(config) => config,
            Err(e) => panic!("full template: {e}"),
        };
        let indicators = config.indicators.into_iter().map(Arc::new).collect();
        (
            indicators,
            ConfidenceScorer::with_catalog(Arc::from(config.frameworks)),
        )
    });
    &FIXTURE
}

/// A scan match with a realistic name at a realistic depth
fn matched_file_strategy() -> impl Strategy<Value = MatchedFile> {
    const NAMES: [&str; 10] = [
        "Cargo.toml",
        "package.json",
        "main.rs",
        "app.js",
        "index.ts",
        "go.mod",
        "tool.py",
        "pom.xml",
        "README.md",
        "App.csproj",
    ];
    const DIRS: [&str; 5] = ["", "src/", "tests/", "node_modules/x/", "a/b/c/"];
    (
        prop::sample::select(&NAMES[..]),
        prop::sample::select(&DIRS[..]),
    )
        .prop_map(|(name, dir)| MatchedFile::new(name.to_string(), format!("{dir}{name}")))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Invariant: every indicator's score is within 0..=1 and does not
    /// change between calls (summation order must not leak into results).
    #[test]
    fn indicator_scores_are_bounded_and_stable(
        files in prop::collection::vec(matched_file_strategy(), 0..12)
    ) {
        let (indicators, scorer) = scoring_fixture();

        for indicator in indicators {
            let score = scorer.calculate_indicator_score(indicator, &files, indicators);
            prop_assert!((0.0..=1.0).contains(&score), "{}: {score}", indicator.name);
            prop_assert_eq!(
                score,
                scorer.calculate_indicator_score(indicator, &files, indicators)
            );
        }
    }
}
