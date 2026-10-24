use super::shared::{
    dependency_framework, nerd_icon, root_indicator, root_indicator_grouped, with_roots,
};
use crate::constants::{COMPOSER_JSON, COMPOSER_LOCK, PHP_EXTENSION};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_php_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "PHP".to_string(),
        vec![
            PHP_EXTENSION.to_string(),
            COMPOSER_JSON.to_string(),
            COMPOSER_LOCK.to_string(),
            "phpunit.xml".to_string(),
            "phpunit.xml.dist".to_string(),
        ],
        "#777bb4".to_string(),
        nerd_icon("e73d"),
        8,
        vec![Ecosystem::Packagist],
        vec![
            root_indicator(COMPOSER_JSON, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(COMPOSER_LOCK, 0.8, IndicatorContext::LanguageRoot),
            // The checked-in template (.dist) and the local copy are
            // alternative forms of the same PHPUnit config.
            root_indicator_grouped(
                "phpunit.xml.dist",
                0.6,
                IndicatorContext::LanguageRoot,
                "php-phpunit-config",
            ),
            root_indicator_grouped(
                "phpunit.xml",
                0.6,
                IndicatorContext::LanguageRoot,
                "php-phpunit-config",
            ),
        ],
    )
}

pub fn php_frameworks() -> Vec<Framework> {
    vec![
        with_roots(
            dependency_framework(
                "Laravel",
                &[Ecosystem::Packagist],
                &["laravel/framework"],
                Some("e73f"),
                Some("#ff2d20"),
                1,
            ),
            vec![root_indicator(
                "artisan",
                0.9,
                IndicatorContext::FrameworkRoot,
            )],
        ),
        dependency_framework(
            "Symfony",
            &[Ecosystem::Packagist],
            &["symfony/framework-bundle"],
            Some("e757"),
            Some("#8b949e"),
            2,
        ),
        dependency_framework(
            "CodeIgniter",
            &[Ecosystem::Packagist],
            &["codeigniter4/framework"],
            Some("e780"),
            Some("#ee4623"),
            3,
        ),
        dependency_framework(
            "Yii",
            &[Ecosystem::Packagist],
            &["yiisoft/yii2"],
            Some("e782"),
            Some("#0073bb"),
            5,
        ),
    ]
}
