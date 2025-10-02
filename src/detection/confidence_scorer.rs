use crate::detection::pattern_matching::PatternMatcher;
use crate::types::{
    ConfidenceFactor, DetectionEvidence, DirectoryType, MatchedFile, ProjectIndicator,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct ConfidenceScorer {
    pattern_matcher: Arc<PatternMatcher>,
    score_cache: Mutex<HashMap<String, f32>>,
}

impl ConfidenceScorer {
    pub fn new() -> Self {
        Self::with_pattern_matcher(Arc::new(PatternMatcher::new()))
    }

    pub fn with_pattern_matcher(pattern_matcher: Arc<PatternMatcher>) -> Self {
        Self {
            pattern_matcher,
            score_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn clear_cache(&self) {
        match self.score_cache.lock() {
            Ok(mut cache) => {
                cache.clear();
            }
            Err(e) => {
                log::error!("ConfidenceScorer cache lock poisoned during clear: {}", e);
            }
        }
    }

    pub fn cache_stats(&self) -> usize {
        match self.score_cache.lock() {
            Ok(cache) => cache.len(),
            Err(e) => {
                log::warn!("ConfidenceScorer cache lock poisoned during stats: {}", e);
                0
            }
        }
    }

    pub fn get_pattern_importance(
        &self,
        pattern: &str,
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        self.get_pattern_importance_with_global(pattern, languages, &[])
    }

    pub fn get_pattern_importance_with_global(
        &self,
        pattern: &str,
        languages: &[Arc<ProjectIndicator>],
        global_indicators: &[crate::types::RootIndicator],
    ) -> f32 {
        for root_indicator in global_indicators {
            if self
                .pattern_matcher
                .matches_pattern(pattern, &root_indicator.pattern)
            {
                return root_indicator.weight;
            }
        }

        for language in languages {
            for root_indicator in &language.root_indicators {
                if self
                    .pattern_matcher
                    .matches_pattern(pattern, &root_indicator.pattern)
                {
                    return root_indicator.weight;
                }
            }
            for framework in &language.frameworks {
                for root_indicator in &framework.root_indicators {
                    if self
                        .pattern_matcher
                        .matches_pattern(pattern, &root_indicator.pattern)
                    {
                        return root_indicator.weight;
                    }
                }
            }
        }

        // Default weight for patterns not matching any root indicator
        0.5
    }

    pub fn calculate_language_score(
        &self,
        language: &Arc<ProjectIndicator>,
        matched_files: &[MatchedFile],
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        if matched_files.is_empty() {
            return 0.0;
        }

        let cache_key = format!("{}:{}", language.name, matched_files.len());

        match self.score_cache.lock() {
            Ok(cache) => {
                if let Some(&cached_score) = cache.get(&cache_key) {
                    return cached_score;
                }
            }
            Err(e) => {
                log::warn!(
                    "ConfidenceScorer cache lock poisoned during read for '{}': {}",
                    language.name,
                    e
                );
                // Continue with score calculation as fallback
            }
        }

        let mut weighted_score = 0.0;
        let mut max_possible_score = 0.0;

        for pattern in &language.files {
            let pattern_importance = self.get_pattern_importance(pattern, languages);
            max_possible_score += pattern_importance;
        }

        for root_indicator in &language.root_indicators {
            max_possible_score += root_indicator.weight;
        }

        for pattern in &language.files {
            let pattern_importance = self.get_pattern_importance(pattern, languages);

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

        let root_indicator_bonus = self.calculate_root_indicator_bonus(language, matched_files);
        weighted_score += root_indicator_bonus;

        let final_score = if max_possible_score > 0.0 {
            (weighted_score / max_possible_score).min(1.0)
        } else {
            0.0
        };

        match self.score_cache.lock() {
            Ok(mut cache) => {
                cache.insert(cache_key, final_score);
            }
            Err(e) => {
                log::warn!(
                    "ConfidenceScorer cache lock poisoned during insert for '{}': {}",
                    language.name,
                    e
                );
                // Still return the computed score
            }
        }
        final_score
    }

    pub fn calculate_language_score_with_evidence(
        &self,
        language: &Arc<ProjectIndicator>,
        matched_files: &[MatchedFile],
        evidence: &mut DetectionEvidence,
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        let score = self.calculate_language_score(language, matched_files, languages);

        evidence.add_confidence_factor(ConfidenceFactor::new(
            "final_confidence".to_string(),
            score,
            1.0,
            format!("Final confidence score for {}", language.name),
        ));

        score
    }

    pub fn quick_termination_check(
        &self,
        matched_files: &[MatchedFile],
        languages: &[Arc<ProjectIndicator>],
    ) -> Option<bool> {
        if matched_files.len() == 1 {
            let file = &matched_files[0];
            if file.depth == 0 && self.get_pattern_importance(&file.filename, languages) >= 0.9 {
                return Some(true);
            }

            for language in languages {
                for root_indicator in &language.root_indicators {
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
                        && self.get_pattern_importance(&file.filename, languages) >= 0.9
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
        language: &Arc<ProjectIndicator>,
        matched_files: &[MatchedFile],
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        let mut bonus = 0.0;

        let root_files = matched_files
            .iter()
            .filter(|f| {
                f.depth == 0
                    && language
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
                    && language
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
                self.get_pattern_importance(&f.filename, languages) >= 0.9
                    && language
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
        language: &Arc<ProjectIndicator>,
        matched_files: &[MatchedFile],
    ) -> f32 {
        let mut bonus = 0.0;

        for root_indicator in &language.root_indicators {
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

    pub fn calculate_framework_confidence(
        &mut self,
        framework: &crate::types::FrameworkDetector,
        base_confidence: f32,
        matched_files: &[MatchedFile],
    ) -> f32 {
        let mut confidence = base_confidence;

        let root_indicator_bonus =
            self.calculate_framework_root_indicator_bonus(framework, matched_files);
        confidence += root_indicator_bonus;

        confidence.min(1.0)
    }

    fn calculate_framework_root_indicator_bonus(
        &mut self,
        framework: &crate::types::FrameworkDetector,
        matched_files: &[MatchedFile],
    ) -> f32 {
        let mut bonus = 0.0;

        for root_indicator in &framework.root_indicators {
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
        language: &Arc<ProjectIndicator>,
        matched_files: &[MatchedFile],
        languages: &[Arc<ProjectIndicator>],
    ) -> f32 {
        let mut quality_score = 0.0;

        for file in matched_files {
            if !language.files.iter().any(|pattern| {
                self.pattern_matcher
                    .matches_pattern(&file.filename, pattern)
            }) {
                continue;
            }

            let base_score = file.weight();
            let pattern_importance = self.get_pattern_importance(&file.filename, languages);

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
        languages: &[Arc<ProjectIndicator>],
    ) -> bool {
        if let Some(should_terminate) = self.quick_termination_check(matched_files, languages) {
            return should_terminate;
        }

        if matched_files.len() == 1 {
            let file = &matched_files[0];
            if file.depth == 0 && self.get_pattern_importance(&file.filename, languages) >= 0.9 {
                return true;
            }

            for language in languages {
                for root_indicator in &language.root_indicators {
                    if self
                        .pattern_matcher
                        .matches_pattern(&file.filename, &root_indicator.pattern)
                        && root_indicator.weight >= 0.8
                    {
                        return true;
                    }
                }
            }
        }

        if matched_files.len() >= 2 {
            let strong_root_indicators: Vec<_> = matched_files
                .iter()
                .filter(|file| {
                    file.depth == 0
                        && self.get_pattern_importance(&file.filename, languages) >= 0.9
                        && file.weight() >= 1.0
                })
                .collect();

            let mut has_strong_root_indicator = false;
            for language in languages {
                for root_indicator in &language.root_indicators {
                    let has_matching_root_file = matched_files.iter().any(|file| {
                        file.depth == 0
                            && self
                                .pattern_matcher
                                .matches_pattern(&file.filename, &root_indicator.pattern)
                    });

                    if has_matching_root_file && root_indicator.weight >= 0.8 {
                        has_strong_root_indicator = true;
                        break;
                    }
                }
                if has_strong_root_indicator {
                    break;
                }
            }

            if !strong_root_indicators.is_empty() || has_strong_root_indicator {
                for language in languages {
                    let confidence =
                        self.calculate_language_score(language, matched_files, languages);

                    if confidence >= 0.9 {
                        return true;
                    }
                    if confidence >= 0.7
                        && (!strong_root_indicators.is_empty() || has_strong_root_indicator)
                    {
                        return true;
                    }
                    if confidence >= 0.6 && has_strong_root_indicator {
                        return true;
                    }
                }
            }
        }

        if matched_files.len() >= 5 {
            for language in languages {
                let confidence = self.calculate_language_score(language, matched_files, languages);
                if confidence >= 0.6 {
                    return true;
                }
            }
        }

        matched_files.len() >= 12
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
        ProjectIndicator,
    };

    use crate::detection::matchers::test_helpers::helpers::{
        create_test_file, create_test_language,
    };

    fn create_test_languages() -> Vec<Arc<ProjectIndicator>> {
        vec![Arc::new(create_test_language(
            "Rust",
            vec!["Cargo.toml", "*.rs"],
        ))]
    }

    #[test]
    fn test_basic_language_scoring() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_language("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("main.rs", "src/main.rs"),
        ];

        let score = scorer.calculate_language_score(&rust_lang, &files, &create_test_languages());
        assert!(score > 0.0, "Should have positive score for matching files");
        assert!(score <= 1.0, "Score should not exceed 1.0");
        Ok(())
    }

    #[test]
    fn test_empty_files_returns_zero() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_language("Rust", vec!["Cargo.toml", "*.rs"]));

        let score = scorer.calculate_language_score(&rust_lang, &[], &create_test_languages());
        assert_eq!(score, 0.0, "Empty files should return zero score");
        Ok(())
    }

    #[test]
    fn test_context_bonus_for_root_files() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_language("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let bonus = scorer.calculate_context_bonus(&rust_lang, &files, &create_test_languages());
        assert!(bonus > 0.0, "Should have bonus for root files");
        assert!(bonus <= 0.3, "Bonus should be capped at 0.3");
        Ok(())
    }

    #[test]
    fn test_quality_score_calculation() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_language("Rust", vec!["Cargo.toml", "*.rs"]));

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("main.rs", "src/main.rs"),
        ];

        let quality = scorer.calculate_quality_score(&rust_lang, &files, &create_test_languages());
        assert!(quality > 0.0, "Should have positive quality score");
        Ok(())
    }

    #[test]
    fn test_early_termination_single_important_file() -> Result<(), Box<dyn std::error::Error>> {
        use crate::detection::matchers::test_helpers::helpers::create_test_language_with_indicators;

        let mut scorer = ConfidenceScorer::new();
        let languages = vec![Arc::new(create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.9)],
        ))];

        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let should_terminate = scorer.should_terminate_early(&files, &languages);
        assert!(
            should_terminate,
            "Should terminate early for important root file"
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_high_confidence() -> Result<(), Box<dyn std::error::Error>> {
        use crate::detection::matchers::test_helpers::helpers::create_test_language_with_indicators;

        let mut scorer = ConfidenceScorer::new();
        let languages = vec![Arc::new(create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.9), ("*.lock", 0.9)],
        ))];

        let files = vec![
            create_test_file("Cargo.toml", "Cargo.toml"),
            create_test_file("Cargo.lock", "Cargo.lock"),
        ];

        let should_terminate = scorer.should_terminate_early(&files, &languages);

        assert!(
            should_terminate,
            "Should terminate early with high-confidence Rust files"
        );
        Ok(())
    }

    #[test]
    fn test_early_termination_many_files() -> Result<(), Box<dyn std::error::Error>> {
        let mut scorer = ConfidenceScorer::new();
        let languages = vec![Arc::new(create_test_language("JavaScript", vec!["*.js"]))];

        let files: Vec<_> = (0..15)
            .map(|i| create_test_file(&format!("file{}.js", i), &format!("src/file{}.js", i)))
            .collect();

        let should_terminate = scorer.should_terminate_early(&files, &languages);
        assert!(should_terminate, "Should terminate early for many files");
        Ok(())
    }

    #[test]
    fn test_evidence_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let scorer = ConfidenceScorer::new();
        let rust_lang = Arc::new(create_test_language("Rust", vec!["Cargo.toml"]));
        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let mut evidence = DetectionEvidence::new();
        let score = scorer.calculate_language_score_with_evidence(
            &rust_lang,
            &files,
            &mut evidence,
            &create_test_languages(),
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
        let rust_lang = Arc::new(ProjectIndicator::with_root_indicators(
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

        let score = scorer.calculate_language_score(&rust_lang, &files, &create_test_languages());
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
        let rust_lang = Arc::new(ProjectIndicator::with_root_indicators(
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

        let score = scorer.calculate_language_score(&rust_lang, &files, &create_test_languages());
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
        let rust_lang = Arc::new(ProjectIndicator::with_root_indicators(
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

        let languages = vec![rust_lang];
        let files = vec![create_test_file("Cargo.toml", "Cargo.toml")];

        let should_terminate = scorer.should_terminate_early(&files, &languages);
        assert!(
            should_terminate,
            "Should terminate early with strong root indicator"
        );
        Ok(())
    }
}
