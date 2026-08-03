use super::shared::{nerd_icon, root_indicator};
use crate::types::{Indicator, IndicatorContext};

pub fn create_terraform_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Terraform".to_string(),
        vec![
            "*.tf".to_string(),
            "*.tfvars".to_string(),
            ".terraform.lock.hcl".to_string(),
        ],
        "#7b42bc".to_string(),
        nerd_icon("f1062"),
        // Low priority: infra config should not outrank an application
        // language in a mixed repo
        12,
        vec![],
        vec![
            root_indicator(".terraform.lock.hcl", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("main.tf", 0.85, IndicatorContext::LanguageRoot),
        ],
    )
}
