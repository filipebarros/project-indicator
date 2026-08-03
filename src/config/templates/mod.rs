pub mod bun;
pub mod cpp;
pub mod csharp;
pub mod dart;
pub mod deno;
pub mod elixir;
pub mod frameworks;
pub mod go;
pub mod java;
pub mod javascript;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nix;
pub mod php;
pub mod python;
pub mod r;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod terraform;
pub mod typescript;
pub mod zig;

pub mod shared;

pub use shared::{generate_root_indicators_simple_max_weight, vcs_root_indicators};

use crate::config::Config;
use crate::types::{ConfigMeta, DetectionConfig, DisplayConfig};
use anyhow::Result;
use shared::ConfigBuilder;
use std::collections::HashMap;

use bun::create_bun_indicator;
use cpp::create_cpp_indicator;
use csharp::create_csharp_indicator;
use dart::create_dart_indicator;
use deno::create_deno_indicator;
use elixir::create_elixir_indicator;
use frameworks::framework_catalog;
use go::create_go_indicator;
use java::create_java_indicator;
use javascript::create_javascript_indicator;
use julia::create_julia_indicator;
use kotlin::create_kotlin_indicator;
use lua::create_lua_indicator;
use nix::create_nix_indicator;
use php::create_php_indicator;
use python::create_python_indicator;
use r::create_r_indicator;
use ruby::create_ruby_indicator;
use rust::create_rust_indicator;
use scala::create_scala_indicator;
use swift::create_swift_indicator;
use terraform::create_terraform_indicator;
use typescript::create_typescript_indicator;
use zig::create_zig_indicator;

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
        create_rust_indicator(),
        create_javascript_indicator(),
        create_python_indicator(),
        create_go_indicator(),
    ];

    let (meta, display, detection) = ConfigBuilder::new()
        .display(false, 2, " | ")
        .detection(2, false, 0.7)
        .build();

    let config = Config {
        meta,
        display,
        detection,
        frameworks: framework_catalog(),
        indicators: languages,
    };

    ConfigTemplate::new(
        "minimal".to_string(),
        "Minimal template with essential languages and basic framework detection".to_string(),
        config,
    )
}

pub fn create_full_template() -> ConfigTemplate {
    let languages = vec![
        create_rust_indicator(),
        create_javascript_indicator(),
        create_typescript_indicator(),
        create_python_indicator(),
        create_go_indicator(),
        create_java_indicator(),
        create_csharp_indicator(),
        create_cpp_indicator(),
        create_php_indicator(),
        create_ruby_indicator(),
        create_swift_indicator(),
        create_kotlin_indicator(),
        create_dart_indicator(),
        create_elixir_indicator(),
        create_zig_indicator(),
        create_r_indicator(),
        create_julia_indicator(),
        create_scala_indicator(),
        create_lua_indicator(),
        create_deno_indicator(),
        create_bun_indicator(),
        create_terraform_indicator(),
        create_nix_indicator(),
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
        frameworks: framework_catalog(),
        indicators: languages,
    };

    ConfigTemplate::new(
        "full".to_string(),
        "Comprehensive template with all supported languages and frameworks".to_string(),
        config,
    )
}

pub fn create_rust_dev_template() -> ConfigTemplate {
    let languages = vec![create_rust_indicator()];

    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 3, " + ")
        .detection(3, false, 0.6)
        .build();

    let config = Config {
        meta,
        display,
        detection,
        frameworks: framework_catalog(),
        indicators: languages,
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
            version: "3.0".to_string(),
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
        frameworks: framework_catalog(),
        indicators: vec![create_python_indicator()],
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
            version: "3.0".to_string(),
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
        frameworks: framework_catalog(),
        indicators: vec![create_javascript_indicator(), create_typescript_indicator()],
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
            version: "3.0".to_string(),
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
        frameworks: framework_catalog(),
        indicators: vec![
            create_javascript_indicator(),
            create_typescript_indicator(),
            create_dart_indicator(),
            create_swift_indicator(),
            create_kotlin_indicator(),
            create_csharp_indicator(),
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
            version: "3.0".to_string(),
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
        frameworks: framework_catalog(),
        indicators: vec![
            create_python_indicator(),
            create_r_indicator(),
            create_julia_indicator(),
            create_scala_indicator(),
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
            version: "3.0".to_string(),
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
        frameworks: framework_catalog(),
        indicators: vec![
            create_rust_indicator(),
            create_python_indicator(),
            create_go_indicator(),
            create_java_indicator(),
            create_csharp_indicator(),
            create_php_indicator(),
            create_ruby_indicator(),
            create_javascript_indicator(),
        ],
    };

    ConfigTemplate::new(
        "enterprise".to_string(),
        "Enterprise template with comprehensive language support for large-scale applications"
            .to_string(),
        config,
    )
}
