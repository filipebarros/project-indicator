use super::shared::root_indicator;
use crate::types::{Indicator, IndicatorContext};

pub fn create_v_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "V".to_string(),
        // *.v is also Verilog/Coq's extension — an accepted collision, since
        // neither is in the catalog; v.mod is the decisive signal
        vec!["v.mod".to_string(), "*.v".to_string(), "*.vsh".to_string()],
        "#5d87bf".to_string(),
        "V".to_string(),
        7,
        vec![],
        vec![root_indicator(
            "v.mod",
            0.95,
            IndicatorContext::LanguageRoot,
        )],
    )
}
