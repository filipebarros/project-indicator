use super::shared::{framework, nerd_icon, root_indicator};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_cpp_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "C++".to_string(),
        vec![
            "*.cpp".to_string(),
            "*.cxx".to_string(),
            "*.cc".to_string(),
            "*.c".to_string(),
            "*.h".to_string(),
            "*.hpp".to_string(),
            "*.hxx".to_string(),
            "CMakeLists.txt".to_string(),
            "Makefile".to_string(),
            "*.vcxproj".to_string(),
        ],
        "#00599c".to_string(),
        nerd_icon("e7a3"),
        9,
        vec![framework(
            "Qt",
            DetectionType::FileExists {
                files: vec!["*.pro".to_string(), "*.pri".to_string()],
            },
            Some(nerd_icon("e87d")),
            Some("#41cd52"),
            1,
            vec![root_indicator("*.pro", 0.9, IndicatorContext::BuildSystem)],
        )],
        vec![
            root_indicator("CMakeLists.txt", 0.95, IndicatorContext::BuildSystem),
            root_indicator("Makefile", 0.9, IndicatorContext::BuildSystem),
        ],
    )
}
