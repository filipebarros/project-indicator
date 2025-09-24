use project_indicator::{
    cli::{Cli, RootIndicatorAction},
    config::{generate_root_indicators_simple_max_weight, vcs_root_indicators, Config},
    detection::{
        conflict_resolver::{ConflictResolver, ResolverConfig},
        DetectionEngine,
    },
    Result,
};

pub fn handle_root_indicators_command(_cli: &Cli, action: &RootIndicatorAction) -> Result<()> {
    let config = Config::load_default()?;

    match action {
        RootIndicatorAction::Conflicts {
            detailed,
            compare_legacy,
            show_strategies,
        } => {
            println!("Root Indicator Conflict Analysis");
            println!("===============================");

            let mut resolver = ConflictResolver::new(ResolverConfig {
                conflict_threshold: 0.05,
                apply_confidence_penalty: true,
                default_strategy: project_indicator::detection::conflict_resolver::ResolutionStrategy::ContextAwareAverage,
                warn_conflicts: *detailed,
            });

            let vcs_indicators = vcs_root_indicators();
            let enhanced_indicators =
                resolver.resolve_indicators(&config.languages, &vcs_indicators);

            if resolver.detected_conflicts.is_empty() {
                println!("✅ No conflicts detected in root indicators.");
            } else {
                println!(
                    "⚠️  {} conflicts detected:",
                    resolver.detected_conflicts.len()
                );
                println!();

                for conflict in &resolver.detected_conflicts {
                    println!("📁 Pattern: '{}'", conflict.pattern);
                    println!("   Sources: {}", conflict.conflicting_sources.len());

                    if *detailed {
                        for source in &conflict.conflicting_sources {
                            println!("     • {} '{}': weight={:.3} (context: {:?})",
                                match source.source_type {
                                    project_indicator::detection::conflict_resolver::SourceType::Language => "Language",
                                    project_indicator::detection::conflict_resolver::SourceType::Framework => "Framework",
                                    project_indicator::detection::conflict_resolver::SourceType::VcsDefault => "VCS",
                                },
                                source.source_name,
                                source.weight,
                                source.context
                            );
                        }
                    }

                    println!("   Resolved weight: {:.3}", conflict.resolved_weight);
                    if *show_strategies {
                        println!("   Strategy: {:?}", conflict.resolution_strategy);
                    }
                    if conflict.confidence_penalty > 0.0 {
                        println!(
                            "   Confidence penalty: {:.1}%",
                            conflict.confidence_penalty * 100.0
                        );
                    }
                    println!();
                }
            }

            if *compare_legacy {
                println!("Simple Max-Weight Comparison:");
                println!("============================");
                let simple_indicators =
                    generate_root_indicators_simple_max_weight(&config.languages);

                println!("Context-Aware vs Simple Max-Weight differences:");
                for enhanced in &enhanced_indicators {
                    if let Some(simple) = simple_indicators
                        .iter()
                        .find(|l| l.pattern == enhanced.pattern)
                    {
                        let diff = enhanced.weight - simple.weight;
                        if diff.abs() > 0.001 {
                            println!(
                                "  '{}': Context-Aware={:.3}, Simple={:.3}, Diff={:+.3}",
                                enhanced.pattern, enhanced.weight, simple.weight, diff
                            );
                        }
                    }
                }
            }

            Ok(())
        }
        RootIndicatorAction::List {
            language,
            framework,
            conflicts_only,
        } => {
            println!("Root Indicators List");
            println!("===================");

            let mut resolver = ConflictResolver::with_defaults();
            let vcs_indicators = vcs_root_indicators();
            resolver.resolve_indicators(&config.languages, &vcs_indicators);

            let conflicting_patterns: std::collections::HashSet<String> = resolver
                .detected_conflicts
                .iter()
                .map(|c| c.pattern.clone())
                .collect();

            for lang in &config.languages {
                if let Some(filter_lang) = language {
                    if !lang.name.eq_ignore_ascii_case(filter_lang) {
                        continue;
                    }
                }

                println!("\n🔤 Language: {}", lang.name);
                for indicator in &lang.root_indicators {
                    let is_conflicting = conflicting_patterns.contains(&indicator.pattern);
                    if *conflicts_only && !is_conflicting {
                        continue;
                    }
                    let conflict_mark = if is_conflicting { " ⚠️" } else { "" };
                    println!(
                        "  📍 {} (weight: {:.3}){}",
                        indicator.pattern, indicator.weight, conflict_mark
                    );
                }

                for fw in &lang.frameworks {
                    if let Some(filter_fw) = framework {
                        if !fw.name.eq_ignore_ascii_case(filter_fw) {
                            continue;
                        }
                    }

                    if !fw.root_indicators.is_empty() {
                        println!("  🔧 Framework: {}", fw.name);
                        for indicator in &fw.root_indicators {
                            let is_conflicting = conflicting_patterns.contains(&indicator.pattern);
                            if *conflicts_only && !is_conflicting {
                                continue;
                            }
                            let conflict_mark = if is_conflicting { " ⚠️" } else { "" };
                            println!(
                                "    📍 {} (weight: {:.3}){}",
                                indicator.pattern, indicator.weight, conflict_mark
                            );
                        }
                    }
                }
            }

            println!("\n📋 VCS Indicators:");
            for indicator in &vcs_indicators {
                let is_conflicting = conflicting_patterns.contains(&indicator.pattern);
                if *conflicts_only && !is_conflicting {
                    continue;
                }
                let conflict_mark = if is_conflicting { " ⚠️" } else { "" };
                println!(
                    "  📍 {} (weight: {:.3}){}",
                    indicator.pattern, indicator.weight, conflict_mark
                );
            }

            Ok(())
        }
        RootIndicatorAction::Validate { strict, suggest } => {
            println!("Root Indicator Validation");
            println!("========================");

            let mut issues_found = 0;

            let mut resolver = ConflictResolver::with_defaults();
            let vcs_indicators = vcs_root_indicators();
            resolver.resolve_indicators(&config.languages, &vcs_indicators);

            if !resolver.detected_conflicts.is_empty() {
                issues_found += resolver.detected_conflicts.len();
                println!(
                    "❌ {} root indicator conflicts detected",
                    resolver.detected_conflicts.len()
                );
                for conflict in &resolver.detected_conflicts {
                    println!(
                        "   • '{}' has {} conflicting sources",
                        conflict.pattern,
                        conflict.conflicting_sources.len()
                    );
                }
                println!();
            }

            if *strict {
                let common_patterns =
                    vec!["package.json", "Cargo.toml", "go.mod", "pyproject.toml"];
                let mut found_patterns = std::collections::HashSet::new();

                for lang in &config.languages {
                    for indicator in &lang.root_indicators {
                        found_patterns.insert(indicator.pattern.as_str());
                    }
                }

                for pattern in &common_patterns {
                    if !found_patterns.contains(pattern) {
                        println!("⚠️  Missing common root indicator: {}", pattern);
                        issues_found += 1;
                    }
                }

                for lang in &config.languages {
                    for indicator in &lang.root_indicators {
                        if indicator.weight < 0.1 {
                            println!(
                                "⚠️  Very low weight ({:.3}) for '{}' in {}",
                                indicator.weight, indicator.pattern, lang.name
                            );
                            issues_found += 1;
                        } else if indicator.weight > 0.99 {
                            println!(
                                "⚠️  Very high weight ({:.3}) for '{}' in {}",
                                indicator.weight, indicator.pattern, lang.name
                            );
                            issues_found += 1;
                        }
                    }
                }
            }

            if issues_found == 0 {
                println!("✅ No issues found in root indicator configuration.");
            } else {
                println!("\n📊 Summary: {} issues found", issues_found);
            }

            if *suggest && issues_found > 0 {
                println!("\n💡 Suggestions:");
                println!("  • Use 'project-indicator root-indicators conflicts --detailed' for detailed conflict analysis");
                println!("  • Consider standardizing weights across similar file types");
                println!("  • Use context-aware resolution for better conflict handling");
                println!(
                    "  • Review framework-specific indicators for overlap with language indicators"
                );
            }

            Ok(())
        }
        RootIndicatorAction::Stats => {
            println!("Root Indicator Performance Statistics");
            println!("===================================");

            let config = Config::load_default()?;

            let engine = DetectionEngine::new(config.languages);
            let stats = engine.get_root_indicator_stats();

            println!("📊 Overview:");
            println!("  Total languages: {}", stats.total_languages);
            println!(
                "  Language root indicators: {}",
                stats.total_language_indicators
            );
            println!(
                "  Framework root indicators: {}",
                stats.total_framework_indicators
            );
            println!(
                "  Early termination patterns: {}",
                stats.early_termination_patterns
            );

            let early_termination_ratio =
                if stats.total_language_indicators + stats.total_framework_indicators > 0 {
                    (stats.early_termination_patterns as f64)
                        / ((stats.total_language_indicators + stats.total_framework_indicators)
                            as f64)
                        * 100.0
                } else {
                    0.0
                };

            println!("\n⚡ Performance Potential:");
            println!("  Early termination ratio: {:.1}%", early_termination_ratio);

            if early_termination_ratio > 50.0 {
                println!("  ✅ Good: Many patterns support early termination");
            } else if early_termination_ratio > 25.0 {
                println!("  ⚠️  Moderate: Some patterns support early termination");
            } else {
                println!("  ❌ Low: Few patterns support early termination");
            }

            println!("\n💡 Early termination provides significant performance benefits by");
            println!("   skipping expensive file scanning when definitive indicators are found.");

            Ok(())
        }
    }
}
