//! `config edit` hands the config path to $EDITOR; its outcome must be honest.

#![cfg(unix)]

mod common;

use std::process::Output;
use tempfile::TempDir;

fn edit_with(editor: &str, home: &TempDir) -> Result<Output, std::io::Error> {
    common::cli(home.path())
        .args(["config", "edit"])
        .env("EDITOR", editor)
        .output()
}

#[test]
fn test_failing_editor_fails_the_command() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let out = edit_with("false", &home)?;
    assert!(!out.status.success(), "a failing editor must not exit 0");
    assert!(String::from_utf8_lossy(&out.stderr).contains("Editor exited"));
    Ok(())
}

#[test]
fn test_config_directory_exists_for_the_editor() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let out = edit_with("true", &home)?;
    assert!(out.status.success());
    assert!(
        home.path()
            .join("xdg-config")
            .join("project-indicator")
            .is_dir(),
        "the editor must be able to save the new file"
    );
    Ok(())
}
