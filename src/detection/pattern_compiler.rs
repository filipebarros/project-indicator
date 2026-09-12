use crate::types::Indicator;
use std::collections::HashSet;
use std::sync::Arc;

/// Extracts the deduplicated set of file patterns referenced by a set of indicators.
pub struct PatternCompiler {
    unique_patterns: Arc<Vec<String>>,
}

impl PatternCompiler {
    pub fn new(indicators: &[Arc<Indicator>]) -> Self {
        let total_patterns_estimate: usize = indicators
            .iter()
            .map(|indicator| indicator.files.len())
            .sum();
        let mut all_patterns = HashSet::with_capacity(total_patterns_estimate);

        for indicator in indicators {
            for pattern in &indicator.files {
                all_patterns.insert(pattern.clone());
            }
        }

        let unique_patterns: Vec<String> = all_patterns.into_iter().collect();

        Self {
            unique_patterns: Arc::new(unique_patterns),
        }
    }

    pub fn unique_patterns(&self) -> Arc<Vec<String>> {
        self.unique_patterns.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Indicator;

    fn create_test_indicator(name: &str, patterns: Vec<&str>) -> Arc<Indicator> {
        Arc::new(Indicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        ))
    }

    #[test]
    fn test_pattern_compiler_creation() {
        let indicators = vec![
            create_test_indicator("Rust", vec!["*.rs", "Cargo.toml", "src/**/*.rs"]),
            create_test_indicator("JavaScript", vec!["*.js", "package.json", "**/*.test.js"]),
        ];

        let compiler = PatternCompiler::new(&indicators);

        let patterns = compiler.unique_patterns();
        assert_eq!(patterns.len(), 6);
        assert!(patterns.contains(&"*.rs".to_string()));
        assert!(patterns.contains(&"src/**/*.rs".to_string()));
        assert!(patterns.contains(&"**/*.test.js".to_string()));
        assert!(patterns.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn test_pattern_deduplication() {
        let indicators = vec![
            create_test_indicator("TypeScript", vec!["*.ts", "package.json"]),
            create_test_indicator("JavaScript", vec!["*.js", "package.json"]),
        ];

        let compiler = PatternCompiler::new(&indicators);

        assert_eq!(compiler.unique_patterns().len(), 3);
    }

    #[test]
    fn test_empty_indicators() {
        let indicators: Vec<Arc<Indicator>> = vec![];
        let compiler = PatternCompiler::new(&indicators);

        assert_eq!(compiler.unique_patterns().len(), 0);
    }
}
