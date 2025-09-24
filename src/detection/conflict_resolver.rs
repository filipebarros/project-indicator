use crate::types::{IndicatorContext, ProjectIndicator, RootIndicator};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct IndicatorConflict {
    pub pattern: String,
    pub conflicting_sources: Vec<ConflictSource>,
    pub resolution_strategy: ResolutionStrategy,
    pub resolved_weight: f32,
    pub confidence_penalty: f32,
}

#[derive(Debug, Clone)]
pub struct ConflictSource {
    pub source_type: SourceType,
    pub source_name: String,
    pub weight: f32,
    pub context: IndicatorContext,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceType {
    Language,
    Framework,
    VcsDefault,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionStrategy {
    MaxWeight,
    ContextAwareAverage,
    LanguagePreference,
    VcsPreference,
    ModeWeight,
}

pub struct ConflictResolver {
    pub detected_conflicts: Vec<IndicatorConflict>,
    pub config: ResolverConfig,
}

#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub conflict_threshold: f32,
    pub apply_confidence_penalty: bool,
    pub default_strategy: ResolutionStrategy,
    pub warn_conflicts: bool,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            conflict_threshold: 0.15,
            apply_confidence_penalty: true,
            default_strategy: ResolutionStrategy::ContextAwareAverage,
            warn_conflicts: true,
        }
    }
}

impl ConflictResolver {
    pub fn new(config: ResolverConfig) -> Self {
        Self {
            detected_conflicts: Vec::new(),
            config,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ResolverConfig::default())
    }

    pub fn resolve_indicators(
        &mut self,
        languages: &[ProjectIndicator],
        vcs_indicators: &[RootIndicator],
    ) -> Vec<RootIndicator> {
        let mut pattern_sources: HashMap<String, Vec<ConflictSource>> = HashMap::new();

        for indicator in vcs_indicators {
            pattern_sources
                .entry(indicator.pattern.clone())
                .or_default()
                .push(ConflictSource {
                    source_type: SourceType::VcsDefault,
                    source_name: "VCS".to_string(),
                    weight: indicator.weight,
                    context: indicator.context.clone(),
                });
        }

        for language in languages {
            for indicator in &language.root_indicators {
                pattern_sources
                    .entry(indicator.pattern.clone())
                    .or_default()
                    .push(ConflictSource {
                        source_type: SourceType::Language,
                        source_name: language.name.clone(),
                        weight: indicator.weight,
                        context: indicator.context.clone(),
                    });
            }

            for framework in &language.frameworks {
                for indicator in &framework.root_indicators {
                    pattern_sources
                        .entry(indicator.pattern.clone())
                        .or_default()
                        .push(ConflictSource {
                            source_type: SourceType::Framework,
                            source_name: format!("{}/{}", language.name, framework.name),
                            weight: indicator.weight,
                            context: indicator.context.clone(),
                        });
                }
            }
        }

        let mut resolved_indicators = Vec::new();
        for (pattern, sources) in pattern_sources {
            let resolved = self.resolve_single_pattern(pattern, sources);
            resolved_indicators.push(resolved);
        }

        resolved_indicators
    }

    fn resolve_single_pattern(
        &mut self,
        pattern: String,
        sources: Vec<ConflictSource>,
    ) -> RootIndicator {
        if sources.len() == 1 {
            return RootIndicator {
                pattern,
                weight: sources[0].weight,
                context: sources[0].context.clone(),
            };
        }

        let min_weight = sources
            .iter()
            .map(|s| s.weight)
            .fold(f32::INFINITY, f32::min);
        let max_weight = sources
            .iter()
            .map(|s| s.weight)
            .fold(f32::NEG_INFINITY, f32::max);
        let weight_difference = max_weight - min_weight;

        let is_conflict = weight_difference > self.config.conflict_threshold;

        let strategy = self.choose_resolution_strategy(&sources, is_conflict);
        let resolved_weight = self.apply_resolution_strategy(&sources, &strategy);

        let confidence_penalty = if is_conflict && self.config.apply_confidence_penalty {
            let variance = self.calculate_weight_variance(&sources);
            (variance * sources.len() as f32 * 0.1).min(0.3)
        } else {
            0.0
        };

        if is_conflict {
            let conflict = IndicatorConflict {
                pattern: pattern.clone(),
                conflicting_sources: sources.clone(),
                resolution_strategy: strategy,
                resolved_weight,
                confidence_penalty,
            };
            self.detected_conflicts.push(conflict);

            if self.config.warn_conflicts {
                self.log_conflict_warning(&pattern, &sources, resolved_weight);
            }
        }

        RootIndicator {
            pattern,
            weight: resolved_weight,
            context: sources[0].context.clone(),
        }
    }

    fn choose_resolution_strategy(
        &self,
        sources: &[ConflictSource],
        is_conflict: bool,
    ) -> ResolutionStrategy {
        if sources
            .iter()
            .any(|s| s.source_type == SourceType::VcsDefault)
        {
            return ResolutionStrategy::VcsPreference;
        }

        if !is_conflict {
            return self.config.default_strategy.clone();
        }

        let has_language_sources = sources
            .iter()
            .any(|s| s.source_type == SourceType::Language);
        let has_framework_sources = sources
            .iter()
            .any(|s| s.source_type == SourceType::Framework);

        if has_language_sources && has_framework_sources {
            ResolutionStrategy::LanguagePreference
        } else {
            ResolutionStrategy::ContextAwareAverage
        }
    }

    fn apply_resolution_strategy(
        &self,
        sources: &[ConflictSource],
        strategy: &ResolutionStrategy,
    ) -> f32 {
        match strategy {
            ResolutionStrategy::MaxWeight => sources
                .iter()
                .map(|s| s.weight)
                .fold(f32::NEG_INFINITY, f32::max),
            ResolutionStrategy::VcsPreference => sources
                .iter()
                .filter(|s| s.source_type == SourceType::VcsDefault)
                .map(|s| s.weight)
                .fold(f32::NEG_INFINITY, f32::max)
                .max(
                    sources
                        .iter()
                        .map(|s| s.weight)
                        .fold(f32::NEG_INFINITY, f32::max),
                ),
            ResolutionStrategy::LanguagePreference => {
                let language_weights: Vec<f32> = sources
                    .iter()
                    .filter(|s| s.source_type == SourceType::Language)
                    .map(|s| s.weight)
                    .collect();

                if !language_weights.is_empty() {
                    language_weights
                        .iter()
                        .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
                } else {
                    sources
                        .iter()
                        .map(|s| s.weight)
                        .fold(f32::NEG_INFINITY, f32::max)
                }
            }
            ResolutionStrategy::ContextAwareAverage => {
                let weighted_sum: f32 = sources
                    .iter()
                    .map(|s| s.weight * s.context.base_priority())
                    .sum();
                let priority_sum: f32 = sources.iter().map(|s| s.context.base_priority()).sum();

                if priority_sum > 0.0 {
                    weighted_sum / priority_sum
                } else {
                    sources.iter().map(|s| s.weight).sum::<f32>() / sources.len() as f32
                }
            }
            ResolutionStrategy::ModeWeight => {
                let mut weight_counts: HashMap<i32, usize> = HashMap::new();
                for source in sources {
                    let rounded_weight = (source.weight * 100.0).round() as i32;
                    *weight_counts.entry(rounded_weight).or_insert(0) += 1;
                }

                let most_common_weight = weight_counts
                    .iter()
                    .max_by_key(|(_, &count)| count)
                    .map(|(&weight, _)| weight as f32 / 100.0)
                    .unwrap_or_else(|| {
                        sources.iter().map(|s| s.weight).sum::<f32>() / sources.len() as f32
                    });

                most_common_weight
            }
        }
    }

    fn calculate_weight_variance(&self, sources: &[ConflictSource]) -> f32 {
        if sources.len() < 2 {
            return 0.0;
        }

        let mean = sources.iter().map(|s| s.weight).sum::<f32>() / sources.len() as f32;
        let variance = sources
            .iter()
            .map(|s| (s.weight - mean).powi(2))
            .sum::<f32>()
            / sources.len() as f32;

        variance.sqrt()
    }

    fn log_conflict_warning(
        &self,
        pattern: &str,
        sources: &[ConflictSource],
        resolved_weight: f32,
    ) {
        eprintln!("⚠️  Root indicator conflict detected for '{}':", pattern);
        for source in sources {
            eprintln!(
                "   {} '{}' claims weight {} (context: {:?})",
                match source.source_type {
                    SourceType::Language => "Language",
                    SourceType::Framework => "Framework",
                    SourceType::VcsDefault => "VCS",
                },
                source.source_name,
                source.weight,
                source.context
            );
        }
        eprintln!("   → Resolved to weight: {:.3}", resolved_weight);
    }

    pub fn get_conflict_summary(&self) -> String {
        if self.detected_conflicts.is_empty() {
            return "No root indicator conflicts detected.".to_string();
        }

        let mut summary = format!(
            "Root Indicator Conflicts Summary ({} conflicts):\n",
            self.detected_conflicts.len()
        );

        for conflict in &self.detected_conflicts {
            summary.push_str(&format!(
                "• '{}': {} sources, resolved weight: {:.3}, penalty: {:.1}%\n",
                conflict.pattern,
                conflict.conflicting_sources.len(),
                conflict.resolved_weight,
                conflict.confidence_penalty * 100.0
            ));
        }

        summary
    }

    pub fn clear_conflicts(&mut self) {
        self.detected_conflicts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_indicator(pattern: &str, weight: f32) -> RootIndicator {
        RootIndicator {
            pattern: pattern.to_string(),
            weight,
            context: IndicatorContext::default(),
        }
    }

    use crate::detection::matchers::test_helpers::helpers::create_test_language_with_indicators;

    #[test]
    fn test_no_conflicts_single_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = ConflictResolver::with_defaults();
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![("Cargo.toml", 0.9)],
        )];
        let vcs = vec![create_test_indicator(".git", 1.0)];

        let result = resolver.resolve_indicators(&languages, &vcs);

        assert_eq!(result.len(), 2);
        assert!(resolver.detected_conflicts.is_empty());
        Ok(())
    }

    #[test]
    fn test_conflict_detection() -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = ConflictResolver::with_defaults();
        let languages = vec![
            create_test_language_with_indicators("JavaScript", vec![("package.json", 0.7)]),
            create_test_language_with_indicators("TypeScript", vec![("package.json", 0.95)]),
        ];
        let vcs = vec![];

        let result = resolver.resolve_indicators(&languages, &vcs);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pattern, "package.json");
        assert!(!resolver.detected_conflicts.is_empty());
        assert_eq!(resolver.detected_conflicts[0].conflicting_sources.len(), 2);
        Ok(())
    }

    #[test]
    fn test_vcs_preference() -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = ConflictResolver::with_defaults();
        let languages = vec![create_test_language_with_indicators(
            "Rust",
            vec![(".git", 0.5)],
        )];
        let vcs = vec![create_test_indicator(".git", 1.0)];

        let result = resolver.resolve_indicators(&languages, &vcs);

        let git_indicator = result
            .iter()
            .find(|i| i.pattern == ".git")
            .ok_or("Failed to find .git indicator")?;
        assert_eq!(git_indicator.weight, 1.0);
        Ok(())
    }

    #[test]
    fn test_context_aware_resolution() -> Result<(), Box<dyn std::error::Error>> {
        let config = ResolverConfig::default();
        let mut resolver = ConflictResolver::new(config);

        let languages = vec![
            create_test_language_with_indicators("JavaScript", vec![("package.json", 0.8)]),
            create_test_language_with_indicators("TypeScript", vec![("package.json", 0.9)]),
        ];

        let result = resolver.resolve_indicators(&languages, &[]);
        let package_indicator = result
            .iter()
            .find(|i| i.pattern == "package.json")
            .ok_or("Failed to find package.json indicator")?;

        assert!(package_indicator.weight > 0.8 && package_indicator.weight < 0.9);
        Ok(())
    }

    #[test]
    fn test_context_from_configuration() -> Result<(), Box<dyn std::error::Error>> {
        let vcs_indicator = RootIndicator {
            pattern: ".git".to_string(),
            weight: 1.0,
            context: IndicatorContext::VersionControl,
        };

        let language_indicator = RootIndicator {
            pattern: "package.json".to_string(),
            weight: 0.9,
            context: IndicatorContext::LanguageRoot,
        };

        let framework_indicator = RootIndicator {
            pattern: "next.config.js".to_string(),
            weight: 0.8,
            context: IndicatorContext::FrameworkRoot,
        };

        assert_eq!(vcs_indicator.context, IndicatorContext::VersionControl);
        assert_eq!(language_indicator.context, IndicatorContext::LanguageRoot);
        assert_eq!(framework_indicator.context, IndicatorContext::FrameworkRoot);

        assert_eq!(IndicatorContext::VersionControl.base_priority(), 1.0);
        assert_eq!(IndicatorContext::LanguageRoot.base_priority(), 0.9);
        assert_eq!(IndicatorContext::FrameworkRoot.base_priority(), 0.8);
        Ok(())
    }

    #[test]
    fn test_confidence_penalty_calculation() -> Result<(), Box<dyn std::error::Error>> {
        let mut resolver = ConflictResolver::with_defaults();
        let languages = vec![
            create_test_language_with_indicators("JavaScript", vec![("package.json", 0.6)]),
            create_test_language_with_indicators("TypeScript", vec![("package.json", 0.9)]),
        ];

        resolver.resolve_indicators(&languages, &[]);

        assert!(!resolver.detected_conflicts.is_empty());
        let conflict = &resolver.detected_conflicts[0];
        assert!(conflict.confidence_penalty > 0.0);
        assert!(conflict.confidence_penalty <= 0.3);
        Ok(())
    }
}
