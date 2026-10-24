//! `config validate` hints must point at the file that is actually failing.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

const INVALID_BUT_PARSEABLE: &str = r##"
[display]
show_frameworks = true
max_frameworks = 0

[[indicators]]
name = "Rust"
files = ["Cargo.toml"]
color = "#dea584"
icon = "R"
priority = 5
"##;

fn validate_in(cwd: &std::path::Path, home: &TempDir) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_project-indicator"))
        .args(["config", "validate"])
        .current_dir(cwd)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .output()
}

#[test]
fn test_hint_for_shadowing_cwd_file_does_not_suggest_config_edit(
) -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(
        project.path().join("project-indicator.toml"),
        INVALID_BUT_PARSEABLE,
    )?;

    let out = validate_in(project.path(), &home)?;
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success());
    assert!(text.contains("validation failed"), "{text}");
    assert!(
        !text.contains("Run 'project-indicator config edit'"),
        "'config edit' opens a different file than the failing one: {text}"
    );
    Ok(())
}

#[test]
fn test_hint_for_default_location_file_suggests_config_edit(
) -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    fs::write(config_dir.join("config.toml"), INVALID_BUT_PARSEABLE)?;

    let out = validate_in(project.path(), &home)?;
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success());
    assert!(text.contains("config edit"), "{text}");
    Ok(())
}
