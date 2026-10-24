//! A shell prompt runs this binary on every keystroke-adjacent redraw, so
//! expected conditions must stay silent on stderr and a closed stdout must
//! never panic.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn command(home: &TempDir) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_project-indicator"));
    cmd.env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .env_remove("RUST_LOG");
    cmd
}

fn run_in(dir: &std::path::Path, home: &TempDir) -> Result<Output, std::io::Error> {
    command(home)
        .arg(dir)
        .args(["--no-cache", "--format", "json"])
        .output()
}

fn assert_silent_stderr(out: &Output) {
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "",
        "stderr must stay empty"
    );
}

#[test]
fn test_empty_package_json_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(project.path().join("package.json"), "")?;
    assert_silent_stderr(&run_in(project.path(), &home)?);
    Ok(())
}

#[test]
fn test_malformed_cargo_toml_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(project.path().join("Cargo.toml"), "[package\nname =")?;
    assert_silent_stderr(&run_in(project.path(), &home)?);
    Ok(())
}

#[test]
fn test_running_in_a_boundary_directory_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    assert_silent_stderr(&run_in(home.path(), &home)?);
    Ok(())
}

#[test]
fn test_oversized_directory_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    for i in 0..600 {
        fs::write(project.path().join(format!("f{i}.txt")), "")?;
    }
    assert_silent_stderr(&run_in(project.path(), &home)?);
    Ok(())
}

/// The reader goes away BEFORE the binary prints (like `| head -0`). The
/// shell wrapper delays the exec so closing the pipe always wins the race.
#[cfg(unix)]
#[test]
fn test_closed_stdout_does_not_panic() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Stdio;

    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\n",
    )?;
    let mut child = Command::new("sh")
        .args([
            "-c",
            "sleep 0.5; exec \"$0\" \"$@\"",
            env!("CARGO_BIN_EXE_project-indicator"),
        ])
        .arg(project.path())
        .arg("--no-cache")
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path().join("xdg-cache"))
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    drop(child.stdout.take());
    let out = child.wait_with_output()?;
    assert_ne!(out.status.code(), Some(101), "must not panic on EPIPE");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    Ok(())
}

#[test]
fn test_utf8_bom_package_json_is_parsed() -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend(br#"{"dependencies":{"react":"18"}}"#);
    fs::write(project.path().join("package.json"), bytes)?;
    let out = run_in(project.path(), &home)?;
    assert_silent_stderr(&out);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("React"),
        "BOM must not hide dependencies: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    Ok(())
}
