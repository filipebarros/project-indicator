//! Helpers shared by the integration tests (each test crate uses a subset).
#![allow(dead_code)]

use project_indicator::{
    config::Config,
    types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext, RootIndicator},
};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// The binary under test, isolated from the real machine: `home` is `$HOME`
/// and holds the XDG config and cache directories (`xdg-config`, `xdg-cache`)
pub fn cli(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_project-indicator"));
    cmd.env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg-config"))
        .env("XDG_CACHE_HOME", home.join("xdg-cache"))
        .env_remove("RUST_LOG");
    cmd
}

/// A temp directory holding `(relative path, content)` files
pub fn create_test_project(files: &[(&str, &str)]) -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    write_files(temp_dir.path(), files)?;
    Ok(temp_dir)
}

/// Write each `(relative path, content)` under `root`, creating directories
pub fn write_files(root: &Path, files: &[(&str, &str)]) -> Result<(), Box<dyn std::error::Error>> {
    for (file_path, content) in files {
        let full_path = root.join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, content)?;
    }
    Ok(())
}

/// An indicator whose first file (the manifest) is also its root marker, the
/// way every built-in template and `config init` output declares one. An
/// indicator with no `root_indicators` is only ever identified by its source
/// files.
fn indicator(name: &str, files: [&str; 2], color: &str, icon: &str, eco: Ecosystem) -> Indicator {
    Indicator::with_root_indicators(
        name.to_string(),
        files.iter().map(|f| f.to_string()).collect(),
        color.to_string(),
        icon.to_string(),
        vec![eco],
        vec![RootIndicator {
            pattern: files[0].to_string(),
            weight: 0.95,
            context: IndicatorContext::LanguageRoot,
            alternative_group: None,
        }],
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
        detection: DetectionType::Dependencies(vec![dependency.to_string()]),
        icon: Some(icon.to_string()),
        color: Some(color.to_string()),
        priority,
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
            indicator("TypeScript", ["tsconfig.json", "*.ts"], "#3178C6", "󰛦", Npm),
            indicator("Rust", ["Cargo.toml", "*.rs"], "#DEA584", "", Cargo),
            indicator("Python", ["*.py", "requirements.txt"], "#3776AB", "", Pypi),
        ],
        ..Default::default()
    }
}
