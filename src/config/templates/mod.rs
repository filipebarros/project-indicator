pub mod cpp;
pub mod csharp;
pub mod dart;
pub mod elixir;
pub mod go;
pub mod java;
pub mod javascript;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod php;
pub mod python;
pub mod r;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod typescript;
pub mod zig;

pub mod shared;

pub use shared::{generate_root_indicators_simple_max_weight, vcs_root_indicators};

use crate::config::Config;
use crate::types::{ConfigMeta, DetectionConfig, DisplayConfig, TrackingConfig};
use anyhow::Result;
use shared::ConfigBuilder;
use std::collections::HashMap;

use cpp::create_cpp_language;
use csharp::create_csharp_language;
use dart::create_dart_language;
use elixir::create_elixir_language;
use go::create_go_language;
use java::create_java_language;
use javascript::create_javascript_language;
use julia::create_julia_language;
use kotlin::create_kotlin_language;
use lua::create_lua_language;
use php::create_php_language;
use python::create_python_language;
use r::create_r_language;
use ruby::create_ruby_language;
use rust::create_rust_language;
use scala::create_scala_language;
use swift::create_swift_language;
use typescript::create_typescript_language;
use zig::create_zig_language;

pub struct ConfigTemplate {
    pub name: String,
    pub description: String,
    pub config: Config,
}

impl ConfigTemplate {
    pub fn new(name: String, description: String, config: Config) -> Self {
        Self {
            name,
            description,
            config,
        }
    }
}

pub struct TemplateGenerator;

impl TemplateGenerator {
    pub fn get_available_templates() -> HashMap<String, ConfigTemplate> {
        let mut templates = HashMap::new();

        templates.insert("minimal".to_string(), create_minimal_template());
        templates.insert("full".to_string(), create_full_template());

        templates.insert("rust-dev".to_string(), create_rust_dev_template());
        templates.insert("python-dev".to_string(), create_python_dev_template());
        templates.insert("web-dev".to_string(), create_web_dev_template());
        templates.insert("mobile-dev".to_string(), create_mobile_dev_template());
        templates.insert("data-science".to_string(), create_data_science_template());
        templates.insert("enterprise".to_string(), create_enterprise_template());

        templates
    }

    pub fn generate_template(template_name: Option<&str>) -> Result<Config> {
        let templates = Self::get_available_templates();

        let template = match template_name {
            Some(name) => templates.get(name).ok_or_else(|| {
                anyhow::anyhow!(
                    "Template '{}' not found. Available templates: {}",
                    name,
                    templates
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
            None => templates
                .get("minimal")
                .ok_or_else(|| anyhow::anyhow!("Internal error: 'minimal' template not found"))?,
        };

        Ok(template.config.clone())
    }

    pub fn list_templates() -> Vec<(String, String)> {
        let templates = Self::get_available_templates();
        templates
            .into_iter()
            .map(|(name, template)| (name, template.description))
            .collect()
    }
}

pub fn create_minimal_template() -> ConfigTemplate {
    let languages = vec![
        create_rust_language(),
        create_javascript_language(),
        create_python_language(),
        create_go_language(),
    ];

    let (meta, display, detection) = ConfigBuilder::new()
        .display(false, 2, " | ")
        .detection(2, false, 0.7)
        .build();

    let config = Config {
        meta,
        display,
        detection,
        tracking: TrackingConfig::default(),
        languages,
    };

    ConfigTemplate::new(
        "minimal".to_string(),
        "Minimal template with essential languages and basic framework detection".to_string(),
        config,
    )
}

pub fn create_full_template() -> ConfigTemplate {
    let languages = vec![
        create_rust_language(),
        create_javascript_language(),
        create_typescript_language(),
        create_python_language(),
        create_go_language(),
        create_java_language(),
        create_csharp_language(),
        create_cpp_language(),
        create_php_language(),
        create_ruby_language(),
        create_swift_language(),
        create_kotlin_language(),
        create_dart_language(),
        create_elixir_language(),
        create_zig_language(),
        create_r_language(),
        create_julia_language(),
        create_scala_language(),
        create_lua_language(),
    ];

    // Threshold 0.3: a canonical single-language project (e.g. package.json +
    // tsconfig.json) scores ~0.36, so 0.4 would skip framework detection for
    // most real projects
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 5, " • ")
        .detection(4, false, 0.3)
        .build();

    let config = Config {
        meta,
        display,
        detection,
        tracking: TrackingConfig::default(),
        languages,
    };

    ConfigTemplate::new(
        "full".to_string(),
        "Comprehensive template with all supported languages and frameworks".to_string(),
        config,
    )
}

pub fn create_rust_dev_template() -> ConfigTemplate {
    let languages = vec![create_rust_language()];

    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 3, " + ")
        .detection(3, false, 0.6)
        .build();

    let config = Config {
        meta,
        display,
        detection,
        tracking: TrackingConfig::default(),
        languages,
    };

    ConfigTemplate::new(
        "rust-dev".to_string(),
        "Rust development template with Actix, Rocket, Axum and cargo ecosystem".to_string(),
        config,
    )
}

pub fn create_python_dev_template() -> ConfigTemplate {
    let config = Config {
        meta: ConfigMeta {
            version: "2.0".to_string(),
        },
        display: DisplayConfig {
            show_frameworks: true,
            max_frameworks: 4,
            framework_separator: " | ".to_string(),
        },
        detection: DetectionConfig {
            max_upward_traversal: 3,
            require_vcs_root: false,
            confidence_threshold: 0.6,
            root_indicators: vcs_root_indicators(),
            max_depth: 3,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        },
        tracking: TrackingConfig::default(),
        languages: vec![create_python_language()],
    };

    ConfigTemplate::new(
        "python-dev".to_string(),
        "Python development template with Django, Flask, FastAPI and package management"
            .to_string(),
        config,
    )
}

pub fn create_web_dev_template() -> ConfigTemplate {
    let config = Config {
        meta: ConfigMeta {
            version: "2.0".to_string(),
        },
        display: DisplayConfig {
            show_frameworks: true,
            max_frameworks: 5,
            framework_separator: ", ".to_string(),
        },
        detection: DetectionConfig {
            max_upward_traversal: 4,
            require_vcs_root: false,
            confidence_threshold: 0.4,
            root_indicators: vcs_root_indicators(),
            max_depth: 3,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        },
        tracking: TrackingConfig::default(),
        languages: vec![create_javascript_language(), create_typescript_language()],
    };

    ConfigTemplate::new(
        "web-dev".to_string(),
        "Web development template optimized for JavaScript and TypeScript with React, Vue, Angular, and Node.js".to_string(),
        config,
    )
}

pub fn create_mobile_dev_template() -> ConfigTemplate {
    let config = Config {
        meta: ConfigMeta {
            version: "2.0".to_string(),
        },
        display: DisplayConfig {
            show_frameworks: true,
            max_frameworks: 4,
            framework_separator: " + ".to_string(),
        },
        detection: DetectionConfig {
            max_upward_traversal: 3,
            require_vcs_root: false,
            confidence_threshold: 0.5,
            root_indicators: vcs_root_indicators(),
            max_depth: 3,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        },
        tracking: TrackingConfig::default(),
        languages: vec![
            create_javascript_language(),
            create_typescript_language(),
            create_dart_language(),
            create_swift_language(),
            create_kotlin_language(),
            create_csharp_language(),
        ],
    };

    ConfigTemplate::new(
        "mobile-dev".to_string(),
        "Mobile development template supporting React Native, Flutter, iOS (Swift), Android (Kotlin), and Xamarin".to_string(),
        config,
    )
}

pub fn create_data_science_template() -> ConfigTemplate {
    let config = Config {
        meta: ConfigMeta {
            version: "2.0".to_string(),
        },
        display: DisplayConfig {
            show_frameworks: true,
            max_frameworks: 8,
            framework_separator: " • ".to_string(),
        },
        detection: DetectionConfig {
            max_upward_traversal: 3,
            require_vcs_root: false,
            confidence_threshold: 0.4,
            root_indicators: vcs_root_indicators(),
            max_depth: 3,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        },
        tracking: TrackingConfig::default(),
        languages: vec![
            create_python_language(),
            create_r_language(),
            create_julia_language(),
            create_scala_language(),
        ],
    };

    ConfigTemplate::new(
        "data-science".to_string(),
        "Data science template with Python, R, Julia, and Scala for machine learning and analytics"
            .to_string(),
        config,
    )
}

pub fn create_enterprise_template() -> ConfigTemplate {
    let config = Config {
        meta: ConfigMeta {
            version: "2.0".to_string(),
        },
        display: DisplayConfig {
            show_frameworks: true,
            max_frameworks: 6,
            framework_separator: " | ".to_string(),
        },
        detection: DetectionConfig {
            max_upward_traversal: 5,
            require_vcs_root: true,
            confidence_threshold: 0.3,
            root_indicators: vcs_root_indicators(),
            max_depth: 4,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        },
        tracking: TrackingConfig::default(),
        languages: vec![
            create_rust_language(),
            create_python_language(),
            create_go_language(),
            create_java_language(),
            create_csharp_language(),
            create_php_language(),
            create_ruby_language(),
            create_javascript_language(),
        ],
    };

    ConfigTemplate::new(
        "enterprise".to_string(),
        "Enterprise template with comprehensive language support for large-scale applications"
            .to_string(),
        config,
    )
}
