//! Property-based tests using proptest
//!
//! These tests verify invariants across randomized inputs to catch edge cases
//! that might be missed by example-based testing.

use project_indicator::patterns::matches_pattern;
use proptest::prelude::*;

/// Strategy for generating filenames
fn filename_strategy() -> impl Strategy<Value = String> {
    ("[a-z]{3,10}", "[a-z]{2,4}").prop_map(|(name, ext)| format!("{}.{}", name, ext))
}

/// Naive recursive glob (`*` = any run of characters): the specification
/// `matches_pattern`'s fast paths must agree with.
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
    /// whichever fast path (exact, `*.ext`, `prefix*`, general) it takes.
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

/// Test that pattern matching works correctly with edge case patterns
#[test]
fn test_edge_case_patterns() {
    let config = ProptestConfig::with_cases(100);

    proptest!(config, |(
        filename in filename_strategy()
    )| {

        prop_assert!(
            matches_pattern(&filename, "*"),
            "Pattern '*' should match '{}'",
            filename
        );

        prop_assert!(
            matches_pattern(&filename, &filename),
            "Exact match failed for '{}'",
            filename
        );

        prop_assert!(
            !matches_pattern(&filename, ""),
            "Empty pattern should not match '{}'",
            filename
        );
    });
}
