mod common;

use std::fs;
use std::path::Path;
use std::process::Output;
use std::time::Duration;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run(home: &Path, path: &Path, extra: &[&str]) -> Result<Output, std::io::Error> {
    common::cli(home)
        .arg(path)
        .args(["--format", "json"])
        .args(extra)
        .env("RUST_LOG", "project_indicator=debug")
        .output()
}

fn entry_count(home: &Path) -> usize {
    fs::read_dir(home.join("xdg-cache/project-indicator/results"))
        .map_or(0, |entries| entries.count())
}

fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn test_repository_cache_is_shared_from_deep_directories() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            (
                "apps/shop/pom.xml",
                "<project><dependencies><dependency><artifactId>spring-boot-starter</artifactId></dependency></dependencies></project>",
            ),
            ("apps/shop/src/main/java/com/acme/foo/Hello.java", "class Hello {}"),
        ],
    )?;
    std::thread::sleep(Duration::from_millis(60));

    let root = run(home.path(), repo.path(), &[])?;
    success(&root);
    let deep = run(
        home.path(),
        &repo.path().join("apps/shop/src/main/java/com/acme"),
        &[],
    )?;
    success(&deep);

    assert_eq!(root.stdout, deep.stdout);
    assert_eq!(entry_count(home.path()), 1);
    let stderr = String::from_utf8_lossy(&deep.stderr);
    assert!(
        stderr.contains(&format!(
            "persistent cache hit for {}",
            repo.path().canonicalize()?.display()
        )),
        "{stderr}"
    );
    Ok(())
}

#[test]
fn test_deep_repository_changes_match_fresh_output() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            ("app/package.json", r#"{"dependencies":{"react":"18"}}"#),
        ],
    )?;
    std::thread::sleep(Duration::from_millis(60));
    let before = run(home.path(), repo.path(), &[])?;
    success(&before);

    common::write_files(
        repo.path(),
        &[(
            "nested/deep/package.json",
            r#"{"dependencies":{"vue":"3"}}"#,
        )],
    )?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    success(&fresh);
    success(&cached);
    assert_ne!(before.stdout, fresh.stdout);
    assert_eq!(fresh.stdout, cached.stdout);
    assert!(String::from_utf8_lossy(&cached.stdout).contains("Vue"));

    common::write_files(
        repo.path(),
        &[(
            "nested/deep/package.json",
            r#"{"dependencies":{"svelte":"4"}}"#,
        )],
    )?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    assert_eq!(fresh.stdout, cached.stdout);
    assert!(String::from_utf8_lossy(&cached.stdout).contains("Svelte"));

    fs::remove_file(repo.path().join("nested/deep/package.json"))?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    assert_eq!(fresh.stdout, cached.stdout);
    assert_eq!(before.stdout, cached.stdout);
    Ok(())
}

#[test]
fn test_repository_entry_is_not_served_after_vcs_marker_is_removed() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            ("Cargo.toml", "[package]\nname = \"x\"\n"),
        ],
    )?;
    let initialized = common::cli(home.path()).args(["config", "init"]).output()?;
    success(&initialized);
    let config = home.path().join("xdg-config/project-indicator/config.toml");
    let text =
        fs::read_to_string(&config)?.replace("require_vcs_root = false", "require_vcs_root = true");
    fs::write(config, text)?;
    std::thread::sleep(Duration::from_millis(60));
    let before = run(home.path(), repo.path(), &[])?;
    success(&before);
    assert!(String::from_utf8_lossy(&before.stdout).contains("Rust"));

    fs::remove_dir_all(repo.path().join(".git"))?;
    let after = run(home.path(), repo.path(), &[])?;
    success(&after);
    let json: serde_json::Value = serde_json::from_slice(&after.stdout)?;
    assert_eq!(json["indicator"], serde_json::Value::Null);
    assert!(json.get("projects").is_none());
    Ok(())
}

#[test]
fn test_nested_path_marker_creation_invalidates_cache() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            ("Cargo.toml", "[package]\nname = \"root\"\n"),
        ],
    )?;
    fs::create_dir_all(repo.path().join("child/src/main"))?;
    std::thread::sleep(Duration::from_millis(60));
    let initial = run(home.path(), repo.path(), &[])?;
    success(&initial);
    assert_eq!(entry_count(home.path()), 1);

    common::write_files(
        repo.path(),
        &[("child/src/main/java/Main.java", "class Main {}")],
    )?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    success(&fresh);
    success(&cached);

    assert_eq!(cached.stdout, fresh.stdout);
    assert!(String::from_utf8_lossy(&cached.stdout).contains("Java"));
    let stderr = String::from_utf8_lossy(&cached.stderr);
    assert!(!stderr.contains("persistent cache hit"), "{stderr}");
    Ok(())
}

#[test]
fn test_nested_exact_marker_creation_invalidates_cache() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            ("Cargo.toml", "[package]\nname = \"root\"\n"),
        ],
    )?;
    fs::create_dir(repo.path().join("child"))?;
    std::thread::sleep(Duration::from_millis(60));
    let initial = run(home.path(), repo.path(), &[])?;
    success(&initial);
    assert_eq!(entry_count(home.path()), 1);

    common::write_files(
        repo.path(),
        &[("child/package.json", r#"{"dependencies":{"react":"18"}}"#)],
    )?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    success(&fresh);
    success(&cached);

    assert_eq!(cached.stdout, fresh.stdout);
    assert!(String::from_utf8_lossy(&cached.stdout).contains("JavaScript"));
    let stderr = String::from_utf8_lossy(&cached.stderr);
    assert!(!stderr.contains("persistent cache hit"), "{stderr}");
    Ok(())
}

#[test]
fn test_manifest_edit_reuses_unrelated_repository_directories() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            (
                "apps/web/package.json",
                r#"{"dependencies":{"react":"18"}}"#,
            ),
            ("apps/api/package.json", r#"{"dependencies":{"vue":"3"}}"#),
        ],
    )?;
    std::thread::sleep(Duration::from_millis(60));
    let initial = run(home.path(), repo.path(), &[])?;
    success(&initial);
    assert_eq!(entry_count(home.path()), 1);

    common::write_files(
        repo.path(),
        &[(
            "apps/web/package.json",
            r#"{"dependencies":{"svelte":"4"}}"#,
        )],
    )?;
    let fresh = run(home.path(), repo.path(), &["--no-cache"])?;
    let cached = run(home.path(), repo.path(), &[])?;
    success(&fresh);
    success(&cached);

    assert_eq!(cached.stdout, fresh.stdout);
    let stderr = String::from_utf8_lossy(&cached.stderr);
    let reused = stderr
        .split("Repository scan reused ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|count| count.parse::<usize>().ok())
        .ok_or_else(|| format!("missing reuse metric in {stderr}"))?;
    assert!(reused > 0, "no unrelated directories reused: {stderr}");
    Ok(())
}

#[test]
fn test_cache_ttl_flag_skips_only_fresh_evidence_validation() -> TestResult {
    let repo = TempDir::new()?;
    let home = TempDir::new()?;
    common::write_files(
        repo.path(),
        &[
            (".git/HEAD", ""),
            ("app/package.json", r#"{"dependencies":{"react":"18"}}"#),
        ],
    )?;
    std::thread::sleep(Duration::from_millis(60));
    let initial = run(home.path(), repo.path(), &[])?;
    success(&initial);

    common::write_files(
        repo.path(),
        &[("app/package.json", r#"{"dependencies":{"vue":"3"}}"#)],
    )?;
    let ttl = run(home.path(), repo.path(), &["--cache-ttl", "60"])?;
    success(&ttl);
    assert_eq!(ttl.stdout, initial.stdout);
    assert!(String::from_utf8_lossy(&ttl.stderr).contains("persistent cache hit"));

    let exact = run(home.path(), repo.path(), &[])?;
    success(&exact);
    assert_ne!(exact.stdout, initial.stdout);
    assert!(String::from_utf8_lossy(&exact.stdout).contains("Vue"));
    Ok(())
}
