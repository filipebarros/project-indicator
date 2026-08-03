use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::constants::{COMPOSER_JSON, COMPOSER_LOCK, PHP_EXTENSION};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_php_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "PHP".to_string(),
        vec![
            PHP_EXTENSION.to_string(),
            COMPOSER_JSON.to_string(),
            COMPOSER_LOCK.to_string(),
        ],
        "#777bb4".to_string(),
        nerd_icon("e73d"),
        8,
        vec![Ecosystem::Packagist],
        vec![
            root_indicator(COMPOSER_JSON, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(COMPOSER_LOCK, 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn php_frameworks() -> Vec<Framework> {
    vec![
        {
            let mut fw = framework(
                "Laravel",
                vec![Ecosystem::Packagist],
                DetectionType::Dependencies {
                    dependencies: vec!["laravel/framework".to_string()],
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
            vec![Ecosystem::Packagist],
            DetectionType::Dependencies {
                dependencies: vec!["symfony/framework-bundle".to_string()],
            },
            Some(nerd_icon("e757")),
            Some("#000000"),
            2,
        ),
        simple_framework(
            "CodeIgniter",
            vec![Ecosystem::Packagist],
            DetectionType::Dependencies {
                dependencies: vec!["codeigniter4/framework".to_string()],
            },
            Some(nerd_icon("e780")),
            Some("#ee4623"),
            3,
        ),
        simple_framework(
            "Yii",
            vec![Ecosystem::Packagist],
            DetectionType::Dependencies {
                dependencies: vec!["yiisoft/yii2".to_string()],
            },
            Some(nerd_icon("e782")),
            Some("#0073bb"),
            5,
        ),
    ]
}
