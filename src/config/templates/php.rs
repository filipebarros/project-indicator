use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_php_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "PHP".to_string(),
        vec![
            "*.php".to_string(),
            "composer.json".to_string(),
            "composer.lock".to_string(),
        ],
        "#777bb4".to_string(),
        nerd_icon("e608"),
        8,
        vec![
            {
                let mut fw = framework(
                    "Laravel",
                    DetectionType::PHPEcosystem {
                        packages: vec!["laravel/framework".to_string()],
                    },
                    Some(nerd_icon("e73f")),
                    Some("#ff2d20"),
                    1,
                    vec![root_indicator(
                        "artisan",
                        0.9,
                        IndicatorContext::FrameworkRoot,
                    )],
                );
                fw.files = vec!["artisan".to_string()];
                fw
            },
            simple_framework(
                "Symfony",
                DetectionType::PHPEcosystem {
                    packages: vec!["symfony/framework-bundle".to_string()],
                },
                Some(nerd_icon("e757")),
                Some("#000000"),
                2,
            ),
        ],
        vec![
            root_indicator("composer.json", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("composer.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}
