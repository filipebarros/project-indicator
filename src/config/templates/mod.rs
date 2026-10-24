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
use crate::types::{DetectionConfig, DisplayConfig, Indicator};
use anyhow::Result;

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

pub struct TemplateGenerator;

impl TemplateGenerator {
    /// `(name, description)` of each template, in display order
    pub const TEMPLATES: [(&'static str, &'static str); 8] = [
        (
            "minimal",
            "Minimal template with essential languages and basic framework detection",
        ),
        (
            "full",
            "Comprehensive template with all supported languages and frameworks",
        ),
        (
            "rust-dev",
            "Rust development template with Actix, Rocket, Axum and cargo ecosystem",
        ),
        (
            "python-dev",
            "Python development template with Django, Flask, FastAPI and package management",
        ),
        (
            "web-dev",
            "Web development template optimized for JavaScript and TypeScript with React, Vue, Angular, and Node.js",
        ),
        (
            "mobile-dev",
            "Mobile development template supporting React Native, Flutter, iOS (Swift), Android (Kotlin), and Xamarin",
        ),
        (
            "data-science",
            "Data science template with Python, R, Julia, and Scala for machine learning and analytics",
        ),
        (
            "enterprise",
            "Enterprise template with comprehensive language support for large-scale applications",
        ),
    ];

    /// Builds only the requested template (default: minimal)
    pub fn generate_template(template_name: Option<&str>) -> Result<Config> {
        let name = template_name.unwrap_or("minimal");
        Self::build(name).ok_or_else(|| {
            let names: Vec<&str> = Self::TEMPLATES.iter().map(|(name, _)| *name).collect();
            anyhow::anyhow!(
                "Template '{}' not found. Available templates: {}",
                name,
                names.join(", ")
            )
        })
    }

    pub fn list_templates() -> Vec<(String, String)> {
        Self::TEMPLATES
            .iter()
            .map(|(name, description)| (name.to_string(), description.to_string()))
            .collect()
    }

    fn build(name: &str) -> Option<Config> {
        // (show_frameworks, max_frameworks), (max_upward_traversal,
        // require_vcs_root), max_depth, indicators
        let template = |display: (bool, usize),
                        detection: (usize, bool),
                        max_depth: usize,
                        indicators: Vec<Indicator>| Config {
            display: DisplayConfig {
                show_frameworks: display.0,
                max_frameworks: display.1,
            },
            detection: DetectionConfig {
                max_upward_traversal: detection.0,
                require_vcs_root: detection.1,
                max_depth,
            },
            frameworks: framework_catalog(),
            indicators,
            ..Config::default()
        };

        Some(match name {
            "minimal" => template(
                (false, 2),
                (2, false),
                1,
                vec![
                    create_rust_indicator(),
                    create_javascript_indicator(),
                    create_python_indicator(),
                    create_go_indicator(),
                ],
            ),
            "full" => template(
                (true, 5),
                (4, false),
                1,
                vec![
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
                ],
            ),
            "rust-dev" => template((true, 3), (3, false), 1, vec![create_rust_indicator()]),
            "python-dev" => template((true, 4), (3, false), 3, vec![create_python_indicator()]),
            "web-dev" => template(
                (true, 5),
                (4, false),
                3,
                vec![create_javascript_indicator(), create_typescript_indicator()],
            ),
            "mobile-dev" => template(
                (true, 4),
                (3, false),
                3,
                vec![
                    create_javascript_indicator(),
                    create_typescript_indicator(),
                    create_dart_indicator(),
                    create_swift_indicator(),
                    create_kotlin_indicator(),
                    create_csharp_indicator(),
                ],
            ),
            "data-science" => template(
                (true, 8),
                (3, false),
                3,
                vec![
                    create_python_indicator(),
                    create_r_indicator(),
                    create_julia_indicator(),
                    create_scala_indicator(),
                ],
            ),
            "enterprise" => template(
                (true, 6),
                (5, true),
                4,
                vec![
                    create_rust_indicator(),
                    create_python_indicator(),
                    create_go_indicator(),
                    create_java_indicator(),
                    create_csharp_indicator(),
                    create_php_indicator(),
                    create_ruby_indicator(),
                    create_javascript_indicator(),
                ],
            ),
            _ => return None,
        })
    }
}
