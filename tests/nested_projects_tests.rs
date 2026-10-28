//! A VCS repository reports every project beneath its root in stable order,
//! including nested and source-only projects. Plain folders without a VCS
//! marker keep the existing bounded non-VCS behavior.

mod common;

use common::create_test_project;
use project_indicator::config::built_in_config;
use project_indicator::detection::engine::{MAX_NESTED_EXAMINED, MAX_NESTED_SHOWN};
use project_indicator::detection::DetectionEngine;
use project_indicator::types::DetectionResult;
use std::fs;
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn detect(root: &Path) -> Result<DetectionResult, Box<dyn std::error::Error>> {
    let config = built_in_config();
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);
    Ok(engine.detect(root)?)
}

fn names(result: &DetectionResult) -> Vec<&str> {
    result.nested.iter().map(|p| p.name.as_str()).collect()
}

const AXUM_CARGO: &str = "[package]\nname = \"b\"\n[dependencies]\naxum = \"0.7\"\n";
const REACT_PACKAGE: &str = r#"{"dependencies":{"react":"18"}}"#;

#[test]
fn test_a_container_reports_each_subproject_with_its_frameworks() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("backend/Cargo.toml", AXUM_CARGO),
        ("frontend/package.json", REACT_PACKAGE),
    ])?;

    let result = detect(dir.path())?;

    assert!(
        result.indicator.is_none(),
        "the folder itself is not a project"
    );
    assert_eq!(names(&result), ["backend", "frontend"]);
    let languages: Vec<&str> = result
        .nested
        .iter()
        .map(|p| p.indicator.name.as_str())
        .collect();
    assert_eq!(languages, ["Rust", "JavaScript"]);
    let backend: Vec<&str> = result.nested[0]
        .frameworks
        .iter()
        .map(|f| f.framework.name.as_str())
        .collect();
    let frontend: Vec<&str> = result.nested[1]
        .frameworks
        .iter()
        .map(|f| f.framework.name.as_str())
        .collect();
    assert_eq!(backend, ["Axum"]);
    assert_eq!(frontend, ["React"]);
    Ok(())
}

#[test]
fn test_a_plain_folder_without_a_vcs_marker_is_never_scanned() -> TestResult {
    // ~/Developer-style folders full of repos stay a single blank prompt
    let dir = create_test_project(&[("backend/Cargo.toml", AXUM_CARGO)])?;

    assert!(detect(dir.path())?.nested.is_empty());
    Ok(())
}

#[test]
fn test_a_worktree_git_file_also_triggers_the_scan() -> TestResult {
    // in worktrees and submodules .git is a FILE, not a directory
    let dir = create_test_project(&[
        (".git", "gitdir: /elsewhere\n"),
        ("backend/Cargo.toml", AXUM_CARGO),
    ])?;

    assert_eq!(names(&detect(dir.path())?), ["backend"]);
    Ok(())
}

#[test]
fn test_projects_two_levels_down_are_found() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("apps/web/package.json", REACT_PACKAGE),
        ("services/api/go.mod", "module x\n"),
    ])?;

    assert_eq!(names(&detect(dir.path())?), ["apps/web", "services/api"]);
    Ok(())
}

#[test]
fn test_nested_projects_inside_projects_are_also_found() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("apps/web/package.json", REACT_PACKAGE),
        ("backend/Cargo.toml", AXUM_CARGO),
        ("backend/tools/package.json", REACT_PACKAGE),
    ])?;

    assert_eq!(
        names(&detect(dir.path())?),
        ["backend", "apps/web", "backend/tools"]
    );
    Ok(())
}

#[test]
fn test_nested_markers_are_found_beneath_a_root_project() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("Cargo.toml", AXUM_CARGO),
        ("csharp/App.csproj", "<Project/>"),
        (
            "typescript/package.json",
            r#"{"devDependencies":{"typescript":"5"}}"#,
        ),
        ("java/src/main/java/Main.java", "class Main {}"),
        ("source_only/main.rs", "fn main() {}"),
        ("source_only/main.py", "print('not a project')"),
    ])?;

    let result = detect(dir.path())?;

    assert_eq!(names(&result), [".", "csharp", "java", "typescript"]);
    let languages: Vec<&str> = result
        .nested
        .iter()
        .map(|project| project.indicator.name.as_str())
        .collect();
    assert_eq!(languages, ["Rust", "C#", "Java", "TypeScript"]);
    Ok(())
}

#[test]
fn test_pnpm_style_packages_directories_are_examined() -> TestResult {
    // packages/* is THE standard pnpm/yarn monorepo layout: it must not be
    // treated as a dependency/noise directory by the subproject scan
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("packages/ui/package.json", REACT_PACKAGE),
    ])?;

    assert_eq!(names(&detect(dir.path())?), ["packages/ui"]);
    Ok(())
}

#[test]
fn test_noise_directories_are_not_examined() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("node_modules/pkg/package.json", REACT_PACKAGE),
        ("vendor/lib/composer.json", "{}"),
        ("real/go.mod", "module x\n"),
    ])?;

    assert_eq!(names(&detect(dir.path())?), ["real"]);
    Ok(())
}

#[test]
fn test_root_and_nested_projects_are_aggregated() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("Cargo.toml", AXUM_CARGO),
        ("tools/package.json", REACT_PACKAGE),
    ])?;

    let result = detect(dir.path())?;

    assert!(result.indicator.is_none());
    assert_eq!(names(&result), [".", "tools"]);
    Ok(())
}

#[test]
fn test_a_folder_inside_a_project_is_not_a_container() -> TestResult {
    let dir = create_test_project(&[
        ("Cargo.toml", AXUM_CARGO),
        ("crates/inner/package.json", REACT_PACKAGE),
    ])?;

    let result = detect(&dir.path().join("crates"))?;

    assert!(result.indicator.is_some(), "found by walking up");
    assert!(result.nested.is_empty());
    Ok(())
}

#[test]
fn test_subdirectories_without_a_project_are_ignored() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("docs/readme.md", ""),
        ("backend/Cargo.toml", AXUM_CARGO),
        ("notes.txt", ""),
    ])?;

    assert_eq!(names(&detect(dir.path())?), ["backend"]);
    Ok(())
}

#[test]
fn test_hidden_directories_and_symlinks_are_skipped() -> TestResult {
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        (".hidden/Cargo.toml", AXUM_CARGO),
        ("real/package.json", REACT_PACKAGE),
    ])?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link"))?;

    assert_eq!(names(&detect(dir.path())?), ["real"]);
    Ok(())
}

#[test]
fn test_every_project_is_reported_in_name_order() -> TestResult {
    let dir = create_test_project(&[(".git/HEAD", "")])?;
    for i in 0..(MAX_NESTED_SHOWN + 3) {
        let sub = dir.path().join(format!("p{i:02}"));
        fs::create_dir(&sub)?;
        fs::write(sub.join("go.mod"), "module x\n")?;
    }

    let result = detect(dir.path())?;

    let expected: Vec<String> = (0..(MAX_NESTED_SHOWN + 3))
        .map(|i| format!("p{i:02}"))
        .collect();
    assert_eq!(names(&result), expected);
    Ok(())
}

#[test]
fn test_projects_beyond_the_old_examined_limit_are_found() -> TestResult {
    let dir = create_test_project(&[(".git/HEAD", "")])?;
    for i in 0..(MAX_NESTED_EXAMINED + 4) {
        fs::create_dir(dir.path().join(format!("d{i:02}")))?;
    }
    for i in MAX_NESTED_EXAMINED..(MAX_NESTED_EXAMINED + 4) {
        fs::write(dir.path().join(format!("d{i:02}/go.mod")), "module x\n")?;
    }

    let expected: Vec<String> = (MAX_NESTED_EXAMINED..(MAX_NESTED_EXAMINED + 4))
        .map(|i| format!("d{i:02}"))
        .collect();
    assert_eq!(names(&detect(dir.path())?), expected);
    Ok(())
}

fn run_cli(
    home: &Path,
    dir: &Path,
    extra: &[&str],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let out = common::cli(home)
        .arg(dir)
        .args(["--format", "json"])
        .args(extra)
        .output()?;
    Ok(serde_json::from_slice(&out.stdout)?)
}

#[test]
fn test_the_home_directory_is_never_a_workspace_folder() -> TestResult {
    // the cli helper points $HOME at the directory we pass: a boundary
    let home = tempfile::TempDir::new()?;
    fs::create_dir_all(home.path().join("backend"))?;
    fs::write(home.path().join("backend/Cargo.toml"), AXUM_CARGO)?;

    fs::write(home.path().join(".git"), "gitdir: x\n")?;

    let out = run_cli(home.path(), home.path(), &[])?;

    assert!(out.get("projects").is_none(), "{out}");
    Ok(())
}

#[test]
fn test_subprojects_are_found_with_require_vcs_root_on() -> TestResult {
    // the children have no .git of their own: each is rejected as a ROOT by
    // require_vcs_root but still identified from its own markers
    let dir = create_test_project(&[
        (".git/HEAD", ""),
        ("backend/Cargo.toml", AXUM_CARGO),
        ("frontend/package.json", REACT_PACKAGE),
    ])?;
    let mut config = built_in_config();
    config.detection.require_vcs_root = true;
    let engine =
        DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);

    let result = engine.detect(dir.path())?;

    assert_eq!(names(&result), ["backend", "frontend"]);
    Ok(())
}
