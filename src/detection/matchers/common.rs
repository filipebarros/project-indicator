use crate::types::FrameworkMatch;
pub fn calculate_confidence(required: usize, matched: usize) -> f32 {
    if required == 0 {
        return 0.0;
    }
    let base = (matched as f32 / required as f32) * 0.9;
    if matched == required {
        base + 0.1
    } else {
        base
    }
}

pub fn calculate_dependency_confidence(required: &[String], matched: &[String]) -> f32 {
    calculate_confidence(required.len(), matched.len())
}

pub fn sort_framework_matches(matches: &mut [FrameworkMatch]) {
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.framework.priority.cmp(&b.framework.priority))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dependency_confidence() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(calculate_confidence(1, 1), 1.0);

        let confidence = calculate_confidence(2, 1);
        assert!(confidence > 0.0 && confidence < 1.0);
        assert_eq!(confidence, 0.45);

        assert_eq!(calculate_confidence(1, 0), 0.0);

        assert_eq!(calculate_confidence(0, 1), 0.0);
        Ok(())
    }

    #[test]
    fn test_confidence_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let confidence = calculate_confidence(3, 3);
        assert_eq!(confidence, 1.0);

        let confidence = calculate_confidence(1, 2);
        assert_eq!(confidence, 1.8);

        let confidence = calculate_confidence(10, 7);
        assert_eq!(confidence, 0.63);

        let confidence = calculate_confidence(1, 0);
        assert_eq!(confidence, 0.0);
        Ok(())
    }

    #[test]
    fn test_confidence_precision() -> Result<(), Box<dyn std::error::Error>> {
        let confidence = calculate_confidence(3, 2);
        assert_eq!(confidence, 0.6);

        let confidence = calculate_confidence(4, 1);
        assert_eq!(confidence, 0.225);
        Ok(())
    }

    #[test]
    fn test_sort_framework_matches() -> Result<(), Box<dyn std::error::Error>> {
        use crate::types::{DetectionType, FrameworkDetector};

        let framework1 = FrameworkDetector {
            name: "High Confidence, Low Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 5,
            files: vec![],
            root_indicators: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "High Confidence, High Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let framework3 = FrameworkDetector {
            name: "Low Confidence, High Priority".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let mut matches = vec![
            FrameworkMatch::new(framework1, 0.9, vec!["file1".to_string()]),
            FrameworkMatch::new(framework2, 0.9, vec!["file2".to_string()]),
            FrameworkMatch::new(framework3, 0.5, vec!["file3".to_string()]),
        ];

        sort_framework_matches(&mut matches);

        assert_eq!(matches[0].framework.name, "High Confidence, High Priority");
        assert_eq!(matches[1].framework.name, "High Confidence, Low Priority");
        assert_eq!(matches[2].framework.name, "Low Confidence, High Priority");
        Ok(())
    }

    #[test]
    fn test_sort_framework_matches_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        use crate::types::{DetectionType, FrameworkDetector};

        let framework1 = FrameworkDetector {
            name: "First".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let framework2 = FrameworkDetector {
            name: "Second".to_string(),
            detection: DetectionType::FileExists { files: vec![] },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        };

        let mut matches = vec![
            FrameworkMatch::new(framework1, 0.8, vec![]),
            FrameworkMatch::new(framework2, 0.8, vec![]),
        ];

        sort_framework_matches(&mut matches);

        assert_eq!(matches.len(), 2);
        assert!(matches[0].confidence == 0.8);
        assert!(matches[1].confidence == 0.8);
        Ok(())
    }

    #[test]
    fn test_sort_empty_matches() -> Result<(), Box<dyn std::error::Error>> {
        let mut matches: Vec<FrameworkMatch> = vec![];
        sort_framework_matches(&mut matches);
        assert!(matches.is_empty());
        Ok(())
    }

    #[test]
    fn test_confidence_bounds() -> Result<(), Box<dyn std::error::Error>> {
        let confidence = calculate_confidence(1, 1);
        assert_eq!(confidence, 1.0);

        let confidence = calculate_confidence(1, 1);
        assert_eq!(confidence, 1.0);

        let confidence = calculate_confidence(2, 0);
        assert_eq!(confidence, 0.0);
        Ok(())
    }
}
