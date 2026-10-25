use project_indicator::types::*;
use std::sync::Arc;

#[test]
fn test_detection_result_display() {
    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies(vec!["react".to_string()]),
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
    };

    let language = Indicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        vec![],
    );

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result_with_framework =
        DetectionResult::new(Some(Arc::new(language.clone())), vec![framework_match], 0.9);

    assert_eq!(result_with_framework.display_icon(), Some("⚛️"));
    assert_eq!(result_with_framework.display_color(), Some("#61DAFB"));

    let result_language_only = DetectionResult::new(Some(Arc::new(language)), vec![], 0.8);

    assert_eq!(result_language_only.display_icon(), Some("󰛦"));
    assert_eq!(result_language_only.display_color(), Some("#3178C6"));
}
