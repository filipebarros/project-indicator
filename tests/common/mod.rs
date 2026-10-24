//! Helpers shared by the integration tests (each test crate uses a subset).
#![allow(dead_code)]

use project_indicator::{
    config::Config,
    types::{DetectionType, Ecosystem, Framework, Indicator},
};
use std::fs;
use tempfile::TempDir;

/// A temp directory holding `(relative path, content)` files
pub fn create_test_project(files: &[(&str, &str)]) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;

    for (file_path, content) in files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, content)?;
    }

    Ok(temp_dir)
}

fn indicator(
    name: &str,
    files: [&str; 2],
    color: &str,
    icon: &str,
    priority: u8,
    eco: Ecosystem,
) -> Indicator {
    Indicator::new(
        name.to_string(),
        files.iter().map(|f| f.to_string()).collect(),
        color.to_string(),
        icon.to_string(),
        priority,
        vec![eco],
    )
}

fn framework(
    name: &str,
    eco: Ecosystem,
    dependency: &str,
    icon: &str,
    color: &str,
    priority: u8,
) -> Framework {
    Framework {
        name: name.to_string(),
        ecosystems: vec![eco],
        detection: DetectionType::Dependencies {
            dependencies: vec![dependency.to_string()],
        },
        icon: Some(icon.to_string()),
        color: Some(color.to_string()),
        priority,
        root_indicators: vec![],
    }
}

/// TypeScript, Rust and Python with React, Next.js (priority 0), Rocket and
/// Django
pub fn create_test_config() -> Config {
    use Ecosystem::{Cargo, Npm, Pypi};
    Config {
        frameworks: vec![
            framework("React", Npm, "react", "⚛️", "#61DAFB", 1),
            framework("Next.js", Npm, "next", "▲", "#000000", 0),
            framework("Rocket", Cargo, "rocket", "🚀", "#D33847", 1),
            framework("Django", Pypi, "django", "📝", "#1E293B", 1),
        ],
        indicators: vec![
            indicator(
                "TypeScript",
                ["tsconfig.json", "*.ts"],
                "#3178C6",
                "󰛦",
                1,
                Npm,
            ),
            indicator("Rust", ["Cargo.toml", "*.rs"], "#DEA584", "", 1, Cargo),
            indicator(
                "Python",
                ["*.py", "requirements.txt"],
                "#3776AB",
                "",
                2,
                Pypi,
            ),
        ],
        ..Default::default()
    }
}
