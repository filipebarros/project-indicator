//! A shell prompt runs this binary on every keystroke-adjacent redraw, so
//! expected conditions must stay silent on stderr and a closed stdout must
//! never panic.

mod common;

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn command(home: &TempDir) -> Command {
    common::cli(home.path())
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

fn write_user_config(home: &TempDir, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("config.toml"), text)?;
    Ok(())
}

/// The opposite case: a config that cannot be used IS worth a warning, once
/// per run on stderr, while detection carries on with the built-in template.
#[test]
fn test_an_unparsable_config_warns_and_detection_still_works(
) -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\n",
    )?;
    write_user_config(&home, "this is not [ valid toml {{{")?;

    let out = run_in(project.path(), &home)?;

    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Ignoring unusable config"), "{stderr}");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(json["indicator"], "Rust");
    Ok(())
}

#[test]
fn test_malformed_cargo_toml_is_silent() -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    fs::write(project.path().join("Cargo.toml"), "[package\nname =")?;
    let out = run_in(project.path(), &home)?;
    assert_silent_stderr(&out);
    // still a Rust project by file name; the unreadable manifest only
    // costs it the framework lookup
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(json["indicator"], "Rust");
    assert_eq!(json["frameworks"], serde_json::json!([]));
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

/// Run the binary, killing it at `deadline`: `None` means it hung.
#[cfg(unix)]
fn run_with_deadline(
    mut cmd: Command,
    deadline: std::time::Duration,
) -> Result<Option<Output>, Box<dyn std::error::Error>> {
    use std::process::Stdio;
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let start = std::time::Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > deadline {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(Some(child.wait_with_output()?))
}

/// A cloned repository decides what sits at these paths. A FIFO opens only
/// when a writer shows up, so reading one blocks the prompt forever.
#[cfg(unix)]
fn make_fifo(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("mkfifo").arg(path).status()?;
    assert!(status.success(), "mkfifo failed");
    Ok(())
}

#[cfg(unix)]
fn assert_a_fifo_named_does_not_hang(
    fifo: &str,
    other_files: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let (home, project) = (TempDir::new()?, TempDir::new()?);
    for (name, content) in other_files {
        fs::write(project.path().join(name), content)?;
    }
    make_fifo(&project.path().join(fifo))?;
    let mut cmd = command(&home);
    cmd.arg(project.path())
        .args(["--no-cache", "--format", "json"]);

    let out = run_with_deadline(cmd, std::time::Duration::from_secs(5))?;

    assert!(out.is_some(), "the binary hung reading a FIFO named {fifo}");
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_a_fifo_manifest_does_not_hang_the_prompt() -> Result<(), Box<dyn std::error::Error>> {
    assert_a_fifo_named_does_not_hang("package.json", &[])
}

#[cfg(unix)]
#[test]
fn test_a_fifo_gitignore_does_not_hang_the_prompt() -> Result<(), Box<dyn std::error::Error>> {
    assert_a_fifo_named_does_not_hang(".gitignore", &[("package.json", "{}")])
}
