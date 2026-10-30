//! The persistent cache must be invisible: after ANY change to the project,
//! a cached run prints exactly what `--no-cache` prints. Each test builds a
//! project, warms the cache from `start`, applies edits, and compares.

mod common;

use common::write_files as write;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Files must age past the cache's mtime slack before a result is stored
const AGE: Duration = Duration::from_millis(60);

/// A project directory nested deeper than the cache's ancestor window (the
/// full template stamps 4 directories above the start), inside a temp tree
/// of its own. Parallel tests churn the shared temp root, and a churning
/// ancestor makes every run a miss, which would let "cached == fresh" pass
/// without the cache ever being consulted.
struct QuietProject {
    _outer: TempDir,
    root: std::path::PathBuf,
}

impl QuietProject {
    fn path(&self) -> &Path {
        &self.root
    }
}

fn quiet_project() -> Result<QuietProject, Box<dyn std::error::Error>> {
    let outer = TempDir::new()?;
    let root = outer.path().join("a/b/c/d/e/proj");
    fs::create_dir_all(&root)?;
    Ok(QuietProject {
        _outer: outer,
        root,
    })
}

/// The output of a run, and whether the persistent cache answered it
fn run_logged(
    home: &Path,
    start: &Path,
    extra: &[&str],
) -> Result<(String, bool), Box<dyn std::error::Error>> {
    let out = common::json_cli(home, start, extra)
        .env("RUST_LOG", "project_indicator=debug")
        .output()?;
    let hit = String::from_utf8_lossy(&out.stderr).contains("persistent cache hit");
    Ok((String::from_utf8_lossy(&out.stdout).into_owned(), hit))
}

fn run(home: &Path, start: &Path, extra: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    Ok(run_logged(home, start, extra)?.0)
}

/// Warm the cache from `root/start`, apply `edit`, then require the cached
/// run to equal a fresh one. `before` must differ from `after`, so the test
/// cannot pass by the edit changing nothing.
fn assert_cached_run_matches_fresh_after(
    initial: &[(&str, &str)],
    start: &str,
    edit: impl FnOnce(&Path) -> TestResult,
) -> TestResult {
    assert_cached_run_in_home(|_| Ok(()), "", initial, start, edit)
}

/// The general form: `prepare_home` runs before the first detection, and the
/// first result must contain `expected_in_first`
fn assert_cached_run_in_home(
    prepare_home: impl FnOnce(&Path) -> TestResult,
    expected_in_first: &str,
    initial: &[(&str, &str)],
    start: &str,
    edit: impl FnOnce(&Path) -> TestResult,
) -> TestResult {
    let (project, home) = (quiet_project()?, TempDir::new()?);
    prepare_home(home.path())?;
    write(project.path(), initial)?;
    let start = project.path().join(start);
    sleep(AGE);

    let before = run(home.path(), &start, &[])?;
    let (warm, hit) = run_logged(home.path(), &start, &[])?;
    assert!(
        before.contains(expected_in_first),
        "precondition: {expected_in_first:?} in {before}"
    );
    assert_eq!(before, warm, "warm run differs");
    assert!(
        common::cache_entry_count(home.path()) > 0 && hit,
        "precondition: the second run must be answered from the cache"
    );

    edit(project.path())?;
    let fresh = run(home.path(), &start, &["--no-cache"])?;
    let cached = run(home.path(), &start, &[])?;

    assert_ne!(
        before, fresh,
        "precondition: the edit must change the result"
    );
    assert_eq!(cached, fresh, "cache served a stale result");
    Ok(())
}

fn assert_cached_run_matches_fresh(
    initial: &[(&str, &str)],
    start: &str,
    edits: &[(&str, &str)],
) -> TestResult {
    assert_cached_run_matches_fresh_after(initial, start, |root| write(root, edits))
}

#[test]
fn test_edit_of_a_rejected_pyproject_candidate() -> TestResult {
    // sub/pyproject.toml is not a Python project (no [project]) so root
    // discovery walks on to the Rust root; making it one changes the root
    assert_cached_run_matches_fresh(
        &[
            ("Cargo.toml", "[package]\nname = \"x\"\n"),
            ("sub/pyproject.toml", "x = 1\n"),
        ],
        "sub",
        &[("sub/pyproject.toml", "[project]\nname = \"y\"\n")],
    )
}

#[test]
fn test_fix_of_a_rejected_pyproject_above_the_scan_path() -> TestResult {
    assert_cached_run_matches_fresh(
        &[("pyproject.toml", "x = 1\n"), ("sub/main.rs", "")],
        "sub",
        &[("pyproject.toml", "[project]\nname = \"x\"\n")],
    )
}

#[test]
fn test_typescript_dependency_added_to_an_ancestor_package_json() -> TestResult {
    assert_cached_run_matches_fresh(
        &[
            ("package.json", r#"{"dependencies":{}}"#),
            ("sub/index.ts", ""),
        ],
        "sub",
        &[("package.json", r#"{"devDependencies":{"typescript":"5"}}"#)],
    )
}

#[test]
fn test_manifest_dependency_edit() -> TestResult {
    assert_cached_run_matches_fresh(
        &[("package.json", r#"{"dependencies":{"react":"18"}}"#)],
        ".",
        &[("package.json", r#"{"dependencies":{"vue":"3"}}"#)],
    )
}

#[test]
fn test_lockfile_edit_when_the_manifest_declares_nothing() -> TestResult {
    // a virtual workspace declares nothing itself, so Cargo.lock decides
    assert_cached_run_matches_fresh(
        &[
            ("Cargo.toml", "[workspace]\nmembers = []\n"),
            (
                "Cargo.lock",
                "[[package]]\nname = \"axum\"\nversion = \"0.7\"\n",
            ),
        ],
        ".",
        &[(
            "Cargo.lock",
            "[[package]]\nname = \"tokio\"\nversion = \"1\"\n",
        )],
    )
}

#[test]
fn test_framework_config_file_created_in_a_subdirectory() -> TestResult {
    assert_cached_run_matches_fresh(
        &[
            ("package.json", r#"{"dependencies":{"react":"18"}}"#),
            ("src-tauri/Cargo.toml", "[package]\nname = \"t\"\n"),
        ],
        ".",
        &[("src-tauri/tauri.conf.json", "{}")],
    )
}

#[test]
fn test_indicator_file_added_to_the_root() -> TestResult {
    assert_cached_run_matches_fresh(&[("README.md", "")], ".", &[("go.mod", "module x\n")])
}

#[test]
fn test_in_place_gitignore_edit_hides_a_matched_file() -> TestResult {
    // an in-place edit leaves every directory mtime untouched
    assert_cached_run_matches_fresh(
        &[("main.go", "package main\n"), (".gitignore", "target\n")],
        ".",
        &[(".gitignore", "main.go\n")],
    )
}

#[test]
fn test_lockfile_only_a_matcher_reads_edited_in_place() -> TestResult {
    // no indicator pattern lists pubspec.lock; only the Dart matcher reads it
    assert_cached_run_matches_fresh(
        &[
            ("pubspec.yaml", "name: x\n"),
            (
                "pubspec.lock",
                "packages:\n  http:\n    dependency: direct\n",
            ),
        ],
        ".",
        &[(
            "pubspec.lock",
            "packages:\n  flutter:\n    dependency: direct\n",
        )],
    )
}

#[test]
fn test_gitignore_edit_hides_a_matched_file() -> TestResult {
    // root manifests ignore .gitignore by design; a plain source file does not
    assert_cached_run_matches_fresh(
        &[("main.go", "package main\n")],
        ".",
        &[(".gitignore", "main.go\n")],
    )
}

#[test]
fn test_manifest_deleted() -> TestResult {
    assert_cached_run_matches_fresh_after(&[("go.mod", "module x\n")], ".", |root| {
        Ok(fs::remove_file(root.join("go.mod"))?)
    })
}

#[test]
fn test_root_manifest_dependency_edit_seen_from_a_subdirectory() -> TestResult {
    assert_cached_run_matches_fresh(
        &[
            ("package.json", r#"{"dependencies":{"react":"18"}}"#),
            ("tsconfig.json", "{}"),
            ("app/.keep", ""),
        ],
        "app",
        &[("package.json", r#"{"dependencies":{"vue":"3"}}"#)],
    )
}

/// With `require_vcs_root` the project is only accepted because a VCS marker
/// exists at or above it: removing a marker ABOVE the root must be noticed.
#[test]
fn test_vcs_marker_above_the_root_removed_when_it_is_required() -> TestResult {
    let (project, home) = (quiet_project()?, TempDir::new()?);
    write(
        project.path(),
        &[
            ("repo/.git/HEAD", ""),
            ("repo/proj/Cargo.toml", "[package]\nname = \"x\"\n"),
            ("repo/proj/sub/.keep", ""),
        ],
    )?;
    write(
        home.path(),
        &[(
            "xdg-config/project-indicator/config.toml",
            "[detection]\nrequire_vcs_root = true\n\n[[indicators]]\nname = \"Rust\"\nfiles = [\"Cargo.toml\", \"*.rs\"]\ncolor = \"#dea584\"\nicon = \"R\"\n\n[[indicators.root_indicators]]\npattern = \"Cargo.toml\"\nweight = 0.95\n",
        )],
    )?;
    let start = project.path().join("repo/proj/sub");
    sleep(AGE);

    let before = run(home.path(), &start, &[])?;
    assert!(
        run_logged(home.path(), &start, &[])?.1,
        "precondition: the second run must be answered from the cache"
    );
    assert!(
        before.contains("Rust"),
        "precondition: Rust accepted: {before}"
    );
    assert!(
        common::cache_entry_count(home.path()) > 0,
        "precondition: entry stored"
    );

    fs::remove_dir_all(project.path().join("repo/.git"))?;

    let fresh = run(home.path(), &start, &["--no-cache"])?;
    assert_ne!(
        before, fresh,
        "precondition: removing the marker changes the result"
    );
    assert_eq!(
        run(home.path(), &start, &[])?,
        fresh,
        "cache served a stale result"
    );
    Ok(())
}

#[test]
fn test_git_info_exclude_created_after_the_entry_was_stored() -> TestResult {
    // `git init` makes `.git/info/` but not necessarily `exclude` inside it
    // (`.git` itself is never watched: every git command changes its mtime)
    assert_cached_run_matches_fresh(
        &[(".git/HEAD", ""), (".git/info/.keep", ""), ("app.js", "")],
        ".",
        &[(".git/info/exclude", "app.js\n")],
    )
}

#[test]
fn test_manifest_edit_when_the_root_marker_is_a_path() -> TestResult {
    assert_cached_run_matches_fresh(
        &[("pom.xml", "<project></project>"), ("src/main/java/.keep", "")],
        ".",
        &[(
            "pom.xml",
            "<project><dependencies><dependency><artifactId>spring-boot-starter</artifactId></dependency></dependencies></project>",
        )],
    )
}

#[test]
fn test_path_marker_created_in_an_existing_directory_below_an_ancestor_probe() -> TestResult {
    // Root discovery stats `sub/src/main/java` (a path marker) on the way up:
    // creating it changes `sub/src/main`, which no level directory reflects
    assert_cached_run_matches_fresh(
        &[
            ("composer.json", r#"{"require":{"laravel/framework":"1"}}"#),
            ("sub/src/main/.keep", ""),
        ],
        "sub",
        &[("sub/src/main/java/.keep", "")],
    )
}

/// A manifest that could not be read (permissions) leaves no trace in
/// the directory: fixing it by chmod changes no mtime, so a result computed
/// without it must not be cached at all.
#[cfg(unix)]
#[test]
fn test_manifest_made_readable_after_a_failed_read() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let (project, home) = (quiet_project()?, TempDir::new()?);
    write(
        project.path(),
        &[("package.json", r#"{"dependencies":{"react":"18"}}"#)],
    )?;
    let manifest = project.path().join("package.json");
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000))?;
    if fs::read(&manifest).is_ok() {
        return Ok(()); // running as root: permissions do not apply
    }
    sleep(AGE);

    let _ = run(home.path(), project.path(), &[])?;
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o644))?;

    let fresh = run(home.path(), project.path(), &["--no-cache"])?;
    assert!(
        fresh.contains("React"),
        "precondition: readable now: {fresh}"
    );
    assert_eq!(
        run(home.path(), project.path(), &[])?,
        fresh,
        "cache served a result computed without the manifest"
    );
    Ok(())
}

/// `$HOME` is a boundary directory for root discovery, so running with a
/// different `$HOME` (sudo, a container, `HOME=… cmd`) can change the result
/// for the very same directory.
#[test]
fn test_a_different_home_is_not_served_from_the_cached_result() -> TestResult {
    let (project, home) = (quiet_project()?, TempDir::new()?);
    write(
        project.path(),
        &[("Cargo.toml", "[package]\nname = \"x\"\n")],
    )?;
    sleep(AGE);

    // $HOME is the project itself: a boundary, so nothing is detected
    let boundary_home = project.path().to_path_buf();
    let with_boundary_home = |extra: &[&str]| -> Result<String, Box<dyn std::error::Error>> {
        let out = common::cli(home.path())
            // both: unix home_dir() reads HOME, Windows reads USERPROFILE
            .env("HOME", &boundary_home)
            .env("USERPROFILE", &boundary_home)
            .arg(project.path())
            .args(["--format", "json"])
            .args(extra)
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let none = with_boundary_home(&[])?;
    assert_eq!(with_boundary_home(&[])?, none, "precondition: stable");

    let normal_fresh = run(home.path(), project.path(), &["--no-cache"])?;
    assert_ne!(none, normal_fresh, "precondition: HOME changes the result");
    assert_eq!(
        run(home.path(), project.path(), &[])?,
        normal_fresh,
        "an entry computed under another $HOME was served"
    );
    Ok(())
}

#[test]
fn test_git_info_directory_and_exclude_created_in_a_git_dir_without_info() -> TestResult {
    // no `info/` yet, so there is nothing below `.git` to watch for `exclude`
    assert_cached_run_matches_fresh(
        &[(".git/HEAD", ""), ("a.ts", "")],
        ".",
        &[(".git/info/exclude", "a.ts\n")],
    )
}

/// Permission and symlink changes move no mtime, and some editors and tools
/// preserve mtimes: the cache must still notice (ctime moves on all of them).
#[cfg(unix)]
mod invisible_to_mtime {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &Path, mode: u32) -> TestResult {
        Ok(fs::set_permissions(path, fs::Permissions::from_mode(mode))?)
    }

    fn running_as_root() -> bool {
        // root ignores permission bits
        let probe = std::env::temp_dir().join(format!("probe-{}", std::process::id()));
        let denied = fs::write(&probe, "x")
            .and_then(|()| fs::set_permissions(&probe, fs::Permissions::from_mode(0o000)))
            .and_then(|()| fs::read(&probe).map(drop))
            .is_ok();
        let _ = fs::remove_file(&probe);
        denied
    }

    #[test]
    fn test_manifest_made_unreadable_after_caching() -> TestResult {
        if running_as_root() {
            return Ok(());
        }
        assert_cached_run_matches_fresh_after(
            &[("package.json", r#"{"dependencies":{"vue":"3"}}"#)],
            ".",
            |root| mode(&root.join("package.json"), 0o000),
        )
    }

    #[test]
    fn test_same_size_edit_with_the_mtime_restored() -> TestResult {
        assert_cached_run_matches_fresh_after(
            &[("package.json", r#"{"dependencies":{"vue":"3"}}"#)],
            ".",
            |root| {
                let path = root.join("package.json");
                let original = fs::metadata(&path)?.modified()?;
                fs::write(&path, r#"{"dependencies":{"zod":"3"}}"#)?;
                fs::File::options()
                    .write(true)
                    .open(&path)?
                    .set_modified(original)?;
                Ok(())
            },
        )
    }

    #[test]
    fn test_scan_root_made_readable_again() -> TestResult {
        // computed while the directory could not be listed: never cached
        let (project, home) = (quiet_project()?, TempDir::new()?);
        write(
            project.path(),
            &[("Cargo.toml", "[package]\nname = \"x\"\n")],
        )?;
        if running_as_root() {
            return Ok(());
        }
        mode(project.path(), 0o311)?;
        sleep(AGE);

        let _ = run(home.path(), project.path(), &[])?;
        let _ = run(home.path(), project.path(), &[])?;
        mode(project.path(), 0o755)?;

        let fresh = run(home.path(), project.path(), &["--no-cache"])?;
        assert!(
            fresh.contains("Rust"),
            "precondition: detectable now: {fresh}"
        );
        assert_eq!(
            run(home.path(), project.path(), &[])?,
            fresh,
            "cache served a result computed from an unlistable directory"
        );
        Ok(())
    }

    /// A directory beyond the ancestors the cache stamps, so creating
    /// something there moves nothing it watches
    fn unwatched_dir(project: &Path) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(project
            .ancestors()
            .nth(project_indicator::detection::engine::MAX_UPWARD_TRAVERSAL + 1)
            .ok_or("project is not deep enough")?
            .join("elsewhere"))
    }

    #[test]
    fn test_dangling_manifest_symlink_in_an_ancestor_whose_target_appears() -> TestResult {
        let (project, home) = (quiet_project()?, TempDir::new()?);
        let far = unwatched_dir(project.path())?;
        write(
            project.path(),
            &[
                ("Cargo.toml", "[package]\nname = \"x\"\n"),
                ("sub/deep/.keep", ""),
            ],
        )?;
        std::os::unix::fs::symlink(
            far.join("package.json"),
            project.path().join("sub/package.json"),
        )?;
        let start = project.path().join("sub/deep");
        sleep(AGE);

        let before = run(home.path(), &start, &[])?;
        let _ = run(home.path(), &start, &[])?;
        write(
            &far,
            &[("package.json", r#"{"dependencies":{"react":"18"}}"#)],
        )?;

        let fresh = run(home.path(), &start, &["--no-cache"])?;
        assert_ne!(before, fresh, "precondition: the target changes the result");
        assert_eq!(
            run(home.path(), &start, &[])?,
            fresh,
            "cache served a result that ignored a dangling symlink in an ancestor"
        );
        Ok(())
    }

    #[test]
    fn test_dangling_manifest_symlink_whose_target_appears() -> TestResult {
        let (project, home) = (quiet_project()?, TempDir::new()?);
        let far = unwatched_dir(project.path())?;
        write(project.path(), &[("README.md", "")])?;
        std::os::unix::fs::symlink(far.join("Cargo.toml"), project.path().join("Cargo.toml"))?;
        sleep(AGE);

        let before = run(home.path(), project.path(), &[])?;
        let _ = run(home.path(), project.path(), &[])?;
        write(&far, &[("Cargo.toml", "[package]\nname = \"x\"\n")])?;

        let fresh = run(home.path(), project.path(), &["--no-cache"])?;
        assert_ne!(before, fresh, "precondition: the target changes the result");
        assert_eq!(
            run(home.path(), project.path(), &[])?,
            fresh,
            "cache served a result that ignored a dangling manifest symlink"
        );
        Ok(())
    }
}

/// Like `assert_cached_run_matches_fresh_after`, for a VCS-tracked workspace
/// folder (the always-on subproject scan), so the subprojects' files are
/// cache inputs too.
fn assert_nested_cached_run_matches_fresh(
    initial: &[(&str, &str)],
    edit: impl FnOnce(&Path) -> TestResult,
) -> TestResult {
    let mut files = vec![(".git/HEAD", "")];
    files.extend_from_slice(initial);
    assert_cached_run_in_home(|_| Ok(()), "projects", &files, ".", edit)
}

#[test]
fn test_nested_subproject_manifest_edited_in_place() -> TestResult {
    assert_nested_cached_run_matches_fresh(
        &[("app/package.json", r#"{"dependencies":{"react":"18"}}"#)],
        |root| {
            write(
                root,
                &[("app/package.json", r#"{"dependencies":{"vue":"3"}}"#)],
            )
        },
    )
}

#[test]
fn test_nested_new_subproject_appears() -> TestResult {
    assert_nested_cached_run_matches_fresh(
        &[("app/package.json", r#"{"dependencies":{"react":"18"}}"#)],
        |root| write(root, &[("api/go.mod", "module x\n")]),
    )
}

#[test]
fn test_nested_manifest_created_in_an_examined_empty_directory() -> TestResult {
    assert_nested_cached_run_matches_fresh(
        &[
            ("app/package.json", r#"{"dependencies":{"react":"18"}}"#),
            ("tools/readme.md", ""),
        ],
        |root| write(root, &[("tools/Cargo.toml", "[package]\nname = \"d\"\n")]),
    )
}

#[test]
fn test_nested_subproject_removed() -> TestResult {
    assert_nested_cached_run_matches_fresh(
        &[("app/package.json", "{}"), ("api/go.mod", "module x\n")],
        |root| Ok(fs::remove_file(root.join("api/go.mod"))?),
    )
}

#[test]
fn test_nested_subproject_changes_language_when_a_source_directory_appears() -> TestResult {
    // the subproject's own probe watches `svc/src/main`; creating `java`
    // inside it moves no mtime the container has any other reason to watch
    assert_nested_cached_run_matches_fresh(
        &[
            ("svc/build.gradle.kts", "plugins { kotlin(\"jvm\") }\n"),
            ("svc/src/main/.keep", ""),
        ],
        |root| write(root, &[("svc/src/main/java/.keep", "")]),
    )
}

#[test]
fn test_nested_subproject_hidden_by_its_own_git_info_exclude() -> TestResult {
    // the child's walk honours ITS `.git/info/exclude`, which is not the
    // container's and sits in no directory the container lists
    assert_nested_cached_run_matches_fresh(
        &[
            ("c/m1.py", ""),
            ("c/m2.py", ""),
            ("c/.git/info/.keep", ""),
            ("other/package.json", "{}"),
        ],
        |root| write(root, &[("c/.git/info/exclude", "*.py\n")]),
    )
}
