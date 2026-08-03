use super::shared::{nerd_icon, root_indicator};
use crate::constants::{
    CC_EXTENSION, CPP_EXTENSION, CPP_HEADER_EXTENSION, CXX_EXTENSION, C_EXTENSION,
    C_HEADER_EXTENSION, HXX_HEADER_EXTENSION, VCXPROJ_EXTENSION,
};
use crate::types::{Indicator, IndicatorContext};

pub fn create_cpp_indicator() -> Indicator {
    Indicator::with_root_indicators(
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
        vec![],
        vec![
            root_indicator("CMakeLists.txt", 0.95, IndicatorContext::BuildSystem),
            root_indicator("Makefile", 0.9, IndicatorContext::BuildSystem),
        ],
    )
}
