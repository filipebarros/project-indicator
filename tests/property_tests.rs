use project_indicator::config::built_in_config;
use project_indicator::detection::confidence_scorer::calculate_indicator_score;
use project_indicator::detection::pattern_processor::PatternProcessor;
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

/// The built-in configuration's indicators and scorer, built once for all cases
fn scoring_fixture() -> &'static Vec<Arc<Indicator>> {
    static FIXTURE: LazyLock<Vec<Arc<Indicator>>> = LazyLock::new(|| {
        let config = built_in_config();
        config.indicators.into_iter().map(Arc::new).collect()
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
        let indicators = scoring_fixture();

        for indicator in indicators {
            let score = calculate_indicator_score(indicator, &files, indicators);
            prop_assert!((0.0..=1.0).contains(&score), "{}: {score}", indicator.name);
            prop_assert_eq!(
                score,
                calculate_indicator_score(indicator, &files, indicators)
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Invariant: whatever fast paths `PatternProcessor` has, it accepts
    /// exactly the filenames that some pattern matches.
    #[test]
    fn should_scan_file_agrees_with_matching_every_pattern(
        patterns in prop::collection::vec("[ab.ü*]{0,6}", 0..6),
        filename in "[ab.ü]{0,8}",
    ) {
        let expected = patterns.iter().any(|p| matches_pattern(&filename, p));
        let processor = PatternProcessor::new(patterns.into());
        prop_assert_eq!(processor.should_scan_file(&filename), expected, "{:?}", filename);
    }
}

/// One unambiguous marker per language, with content that passes its check.
/// Shared files (package.json, the Node lockfiles, the JVM build files) are
/// deliberately absent: they are arbitrated, not decided alone.
const SOLE_MARKERS: &[(&str, &str, &str)] = &[
    ("Cargo.toml", "[package]\nname = \"x\"\n", "Rust"),
    ("go.mod", "module x\n", "Go"),
    ("Gemfile", "source 'https://rubygems.org'\n", "Ruby"),
    ("mix.exs", "defmodule X.MixProject do\nend\n", "Elixir"),
    ("pubspec.yaml", "name: x\n", "Dart"),
    ("composer.json", "{}", "PHP"),
    ("Package.swift", "// swift-tools-version:5.9\n", "Swift"),
    ("gleam.toml", "name = \"x\"\n", "Gleam"),
    ("v.mod", "Module { name: 'x' }\n", "V"),
    ("pyproject.toml", "[project]\nname = \"x\"\n", "Python"),
    ("DESCRIPTION", "Package: x\nVersion: 0.1.0\n", "R"),
    ("build.sbt", "name := \"x\"\n", "Scala"),
    ("build.zig", "", "Zig"),
];

/// Tool configs, version pins and generic files: never identifying evidence.
const NOISE_FILES: &[(&str, &str)] = &[
    (".npmrc", "registry=https://example.test\n"),
    (".nvmrc", "20\n"),
    (".python-version", "3.12\n"),
    (".ruby-version", "3.3.0\n"),
    ("Makefile", "all:\n\ttrue\n"),
    ("phpunit.xml", "<phpunit/>\n"),
    ("analysis_options.yaml", "linter:\n"),
    ("rust-toolchain.toml", "[toolchain]\nchannel = \"stable\"\n"),
    ("README.md", "# x\n"),
    ("notes.txt", ""),
];

fn detect_in(files: &[(&str, &str)]) -> Result<Option<String>, TestCaseError> {
    let dir = tempfile::TempDir::new().map_err(|e| TestCaseError::fail(e.to_string()))?;
    for (name, content) in files {
        std::fs::write(dir.path().join(name), content)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
    }
    let config = built_in_config();
    let engine = project_indicator::detection::DetectionEngine::with_config(
        config.indicators,
        config.frameworks,
        config.detection,
    );
    let result = engine
        .detect(dir.path())
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    Ok(result.indicator.map(|i| i.name.clone()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// End-to-end invariant: a folder holding exactly one language's marker
    /// is that language, and any pile of tool configs and version pins
    /// beside it changes nothing.
    #[test]
    fn test_one_marker_decides_whatever_noise_sits_beside_it(
        marker_idx in 0..SOLE_MARKERS.len(),
        noise in proptest::sample::subsequence(NOISE_FILES.to_vec(), 0..NOISE_FILES.len()),
    ) {
        let (file, content, language) = SOLE_MARKERS[marker_idx];

        let alone = detect_in(&[(file, content)])?;
        prop_assert_eq!(alone.as_deref(), Some(language), "{} alone", file);

        let mut files = vec![(file, content)];
        files.extend(noise.iter().copied());
        let with_noise = detect_in(&files)?;
        prop_assert_eq!(
            with_noise.as_deref(),
            Some(language),
            "{} beside {} noise files",
            file,
            files.len() - 1
        );
    }

    /// The mirror invariant: noise alone identifies nothing.
    #[test]
    fn test_noise_alone_is_no_project(
        noise in proptest::sample::subsequence(NOISE_FILES.to_vec(), 1..NOISE_FILES.len()),
    ) {
        prop_assert_eq!(detect_in(&noise)?, None);
    }
}
