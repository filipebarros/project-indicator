pub mod bun;
pub mod cpp;
pub mod csharp;
pub mod dart;
pub mod deno;
pub mod elixir;
pub mod frameworks;
pub mod gleam;
pub mod go;
pub mod java;
pub mod javascript;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nix;
pub mod odin;
pub mod php;
pub mod python;
pub mod r;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod swift;
pub mod terraform;
pub mod typescript;
pub mod v;
pub mod zig;

pub mod shared;

use crate::config::Config;
use anyhow::Result;
use shared::ConfigBuilder;

use bun::create_bun_indicator;
use cpp::create_cpp_indicator;
use csharp::create_csharp_indicator;
use dart::create_dart_indicator;
use deno::create_deno_indicator;
use elixir::create_elixir_indicator;
use frameworks::framework_catalog;
use gleam::create_gleam_indicator;
use go::create_go_indicator;
use java::create_java_indicator;
use javascript::create_javascript_indicator;
use julia::create_julia_indicator;
use kotlin::create_kotlin_indicator;
use lua::create_lua_indicator;
use nix::create_nix_indicator;
use odin::create_odin_indicator;
use php::create_php_indicator;
use python::create_python_indicator;
use r::create_r_indicator;
use ruby::create_ruby_indicator;
use rust::create_rust_indicator;
use scala::create_scala_indicator;
use swift::create_swift_indicator;
use terraform::create_terraform_indicator;
use typescript::create_typescript_indicator;
use v::create_v_indicator;
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
    /// Template names in display order
    pub const TEMPLATE_NAMES: [&'static str; 8] = [
        "minimal",
        "full",
        "rust-dev",
        "python-dev",
        "web-dev",
        "mobile-dev",
        "data-science",
        "enterprise",
    ];

    fn build(name: &str) -> Option<ConfigTemplate> {
        Some(match name {
            "minimal" => create_minimal_template(),
            "full" => create_full_template(),
            "rust-dev" => create_rust_dev_template(),
            "python-dev" => create_python_dev_template(),
            "web-dev" => create_web_dev_template(),
            "mobile-dev" => create_mobile_dev_template(),
            "data-science" => create_data_science_template(),
            "enterprise" => create_enterprise_template(),
            _ => return None,
        })
    }

    /// Builds only the requested template (default: minimal)
    pub fn generate_template(template_name: Option<&str>) -> Result<Config> {
        let name = template_name.unwrap_or("minimal");
        let template = Self::build(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Template '{}' not found. Available templates: {}",
                name,
                Self::TEMPLATE_NAMES.join(", ")
            )
        })?;
        Ok(template.config)
    }

    pub fn list_templates() -> Vec<(String, String)> {
        Self::TEMPLATE_NAMES
            .iter()
            .filter_map(|name| Self::build(name).map(|t| (name.to_string(), t.description)))
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
        .display(false, 2)
        .detection(2, false)
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
        create_gleam_indicator(),
        create_odin_indicator(),
        create_v_indicator(),
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

    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 5)
        .detection(4, false)
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
        .display(true, 3)
        .detection(3, false)
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
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 4)
        .detection(3, false)
        .max_depth(3)
        .build();

    let config = Config {
        meta,
        display,
        detection,
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
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 5)
        .detection(4, false)
        .max_depth(3)
        .build();

    let config = Config {
        meta,
        display,
        detection,
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
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 4)
        .detection(3, false)
        .max_depth(3)
        .build();

    let config = Config {
        meta,
        display,
        detection,
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
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 8)
        .detection(3, false)
        .max_depth(3)
        .build();

    let config = Config {
        meta,
        display,
        detection,
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
    let (meta, display, detection) = ConfigBuilder::new()
        .display(true, 6)
        .detection(5, true)
        .max_depth(4)
        .build();

    let config = Config {
        meta,
        display,
        detection,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_listing_order_is_stable() {
        let names = || -> Vec<String> {
            TemplateGenerator::list_templates()
                .into_iter()
                .map(|(n, _)| n)
                .collect()
        };
        let first = names();
        assert_eq!(first.first().map(String::as_str), Some("minimal"));
        for _ in 0..10 {
            assert_eq!(first, names(), "listing order must not vary between calls");
        }
        let err = match TemplateGenerator::generate_template(Some("nope")) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("unknown template must fail"),
        };
        assert!(
            err.ends_with(&first.join(", ")),
            "error lists templates in order: {err}"
        );
    }
}
