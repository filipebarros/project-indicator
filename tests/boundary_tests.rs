//! Boundary directories (home) must be recognised even when `$HOME` is a
//! symlink: the engine works on canonical paths, so a non-canonical home
//! would never compare equal.

#[cfg(unix)]
#[test]
fn test_symlinked_home_is_still_a_boundary() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new()?;
    let real_home = tmp.path().join("realhome");
    std::fs::create_dir(&real_home)?;
    std::fs::write(real_home.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
    let link_home = tmp.path().join("linkhome");
    std::os::unix::fs::symlink(&real_home, &link_home)?;

    let run = |home: &std::path::Path| -> Result<String, Box<dyn std::error::Error>> {
        let out = Command::new(env!("CARGO_BIN_EXE_project-indicator"))
            .args([
                real_home.to_str().unwrap_or("."),
                "--no-cache",
                "--format",
                "json",
            ])
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", tmp.path().join("none"))
            .env("XDG_CACHE_HOME", tmp.path().join("cache"))
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    };

    let direct = run(&real_home)?;
    let via_link = run(&link_home)?;
    assert!(
        direct.contains("\"indicator\":null"),
        "home is a boundary: {direct}"
    );
    assert_eq!(
        direct, via_link,
        "a symlinked HOME must behave like the real one"
    );
    Ok(())
}
