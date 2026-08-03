#[cfg(test)]
pub mod helpers {
    use crate::types::{DetectionType, Framework};
    use std::fs;
    use tempfile::TempDir;

    pub fn create_file(
        filename: &str,
        content: &str,
    ) -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join(filename);
        fs::write(file_path, content)?;
        Ok(temp_dir)
    }

    fn create_framework_detector(name: &str, detection: DetectionType, priority: u8) -> Framework {
        Framework {
            name: name.to_string(),
            ecosystems: vec![],
            detection,
            icon: None,
            color: None,
            priority,
            files: vec![],
            root_indicators: vec![],
        }
    }

    pub fn create_test_indicator(name: &str, patterns: Vec<&str>) -> crate::types::Indicator {
        crate::types::Indicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        )
    }

    pub fn create_test_indicator_with_priority(
        name: &str,
        patterns: Vec<&str>,
        priority: u8,
    ) -> crate::types::Indicator {
        crate::types::Indicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            priority,
            vec![],
        )
    }

    pub fn create_test_file(filename: &str, path: &str) -> crate::types::MatchedFile {
        crate::types::MatchedFile::new(filename.to_string(), path.to_string())
    }

    pub fn create_test_framework_generic(name: &str, priority: u8) -> Framework {
        create_framework_detector(
            name,
            DetectionType::FileExists {
                files: vec![format!("{}.toml", name.to_lowercase())],
            },
            priority,
        )
    }

    pub fn create_test_indicator_with_indicators(
        name: &str,
        indicators: Vec<(&str, f32)>,
    ) -> crate::types::Indicator {
        use crate::types::IndicatorContext;

        crate::types::Indicator::with_root_indicators(
            name.to_string(),
            vec![],
            "#000000".to_string(),
            "🔧".to_string(),
            1,
            vec![],
            indicators
                .into_iter()
                .map(|(pattern, weight)| crate::types::RootIndicator {
                    pattern: pattern.to_string(),
                    weight,
                    context: IndicatorContext::default(),
                })
                .collect(),
        )
    }
}
