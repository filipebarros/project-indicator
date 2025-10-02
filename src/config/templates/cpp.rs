use super::shared::{framework, nerd_icon, root_indicator};
use crate::constants::{
    CC_EXTENSION, CPP_EXTENSION, CPP_HEADER_EXTENSION, CXX_EXTENSION, C_EXTENSION,
    C_HEADER_EXTENSION, HXX_HEADER_EXTENSION, QT_PRI_EXTENSION, QT_PRO_EXTENSION,
    VCXPROJ_EXTENSION,
};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_cpp_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "C++".to_string(),
        vec![
            CPP_EXTENSION.to_string(),
            CXX_EXTENSION.to_string(),
            CC_EXTENSION.to_string(),
            C_EXTENSION.to_string(),
            C_HEADER_EXTENSION.to_string(),
            CPP_HEADER_EXTENSION.to_string(),
            HXX_HEADER_EXTENSION.to_string(),
            "CMakeLists.txt".to_string(),
            "Makefile".to_string(),
            VCXPROJ_EXTENSION.to_string(),
        ],
        "#00599c".to_string(),
        nerd_icon("e7a3"),
        9,
        vec![framework(
            "Qt",
            DetectionType::FileExists {
                files: vec![QT_PRO_EXTENSION.to_string(), QT_PRI_EXTENSION.to_string()],
            },
            Some(nerd_icon("e87d")),
            Some("#41cd52"),
            1,
            vec![root_indicator(
                QT_PRO_EXTENSION,
                0.9,
                IndicatorContext::BuildSystem,
            )],
        )],
        vec![
            root_indicator("CMakeLists.txt", 0.95, IndicatorContext::BuildSystem),
            root_indicator("Makefile", 0.9, IndicatorContext::BuildSystem),
        ],
    )
}
