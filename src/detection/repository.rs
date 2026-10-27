use crate::detection::caches::ParsedFileCache;
use crate::detection::engine::{merge_child_evidence, DetectionEngine};
use crate::detection::pattern_processor::normalized_relative;
use crate::detection::root_indicators::VCS_MARKERS;
use crate::performance::FileSystemCache;
use crate::types::{DetectionEvidence, DetectionResult, DirectoryType, NestedProject};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Windows system directories, matched case-insensitively with any verbatim
/// `\\?\` prefix (which `canonicalize` adds) stripped
pub(crate) fn is_windows_system_dir(path: &str) -> bool {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    [
        r"C:\Windows",
        r"C:\Program Files",
        r"C:\Program Files (x86)",
        r"C:\Users",
    ]
    .iter()
    .any(|dir| path.eq_ignore_ascii_case(dir))
}

pub(crate) fn is_boundary_directory(path: &Path, home: Option<&Path>) -> bool {
    if home == Some(path) || path.parent().is_none() {
        return true;
    }

    let path_str = path.to_string_lossy();
    if is_windows_system_dir(&path_str) {
        return true;
    }

    matches!(
        path_str.as_ref(),
        "/Users"
            | "/home"
            | "/root"
            | "/System"
            | "/Library"
            | "/Applications"
            | "/tmp"
            | "/private/tmp"
            | "/var"
            | "/private/var"
            | "/opt"
            | "/usr"
            | "/etc"
            | "/srv"
            | "/mnt"
            | "/media"
            | "/Volumes"
    )
}

pub fn find_vcs_root(start: &Path) -> Option<PathBuf> {
    let home = std::env::home_dir().map(|home| home.canonicalize().unwrap_or(home));
    find_vcs_root_with_home(start, home.as_deref())
}

pub(crate) fn find_vcs_root_with_home(start: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let home = home.map(|home| home.canonicalize().unwrap_or_else(|_| home.to_path_buf()));
    start
        .ancestors()
        .take_while(|dir| !is_boundary_directory(dir, home.as_deref()))
        .find(|dir| VCS_MARKERS.iter().any(|marker| dir.join(marker).exists()))
        .map(Path::to_path_buf)
}

const REPOSITORY_SCAN_TIMEOUT: Duration = Duration::from_secs(5);

pub fn detect_repository(engine: &DetectionEngine, repository_root: &Path) -> DetectionResult {
    let repository_root = repository_root
        .canonicalize()
        .unwrap_or_else(|_| repository_root.to_path_buf());
    detect_repository_with_timeout(engine, &repository_root, REPOSITORY_SCAN_TIMEOUT)
}

pub(crate) fn detect_repository_with_timeout(
    engine: &DetectionEngine,
    repository_root: &Path,
    timeout: Duration,
) -> DetectionResult {
    let repository_root = repository_root
        .canonicalize()
        .unwrap_or_else(|_| repository_root.to_path_buf());
    let started = Instant::now();
    let deadline = started + timeout;
    let evidence = DetectionEvidence {
        repository: true,
        scan_root: Some(repository_root.clone()),
        ..DetectionEvidence::default()
    };
    let mut result = DetectionResult::new_with_evidence(None, Vec::new(), 0.0, evidence);
    let mut queue = VecDeque::from([(repository_root.clone(), false)]);
    let file_cache = FileSystemCache::default();
    let parsed_cache = ParsedFileCache::new();

    let mut files_matched = 0;
    while let Some((directory, has_project_ancestor)) = queue.pop_front() {
        if Instant::now() >= deadline {
            result.evidence.scan_truncated = true;
            break;
        }
        result
            .evidence
            .watched_dirs
            .push(directory.to_string_lossy().into_owned());

        let scan = engine.scanning_engine.scan_with_status(&directory);
        files_matched += scan.matches.len();
        result.evidence.scan_truncated |= scan.truncated;
        result.evidence.read_failed |= scan.unreadable;
        result.evidence.watched_dirs.extend(
            scan.watched_dirs
                .iter()
                .map(|watched| directory.join(watched).to_string_lossy().into_owned()),
        );

        let mut has_project = has_project_ancestor;
        if !scan.matches.is_empty() {
            let has_markers = has_project
                && !engine
                    .root_indicator_engine
                    .qualifying_markers_at(&directory, &file_cache, &parsed_cache)
                    .is_empty();
            if !has_project || has_markers {
                let found = engine.detect_one(&directory, 0, true);
                merge_child_evidence(&mut result.evidence, &directory, &found.evidence);
                if let Some(indicator) = found.indicator {
                    let name = normalized_relative(&directory, &repository_root)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| ".".to_string());
                    result.nested.push(NestedProject {
                        name,
                        indicator,
                        frameworks: found.frameworks,
                    });
                    has_project = true;
                }
            }
        }

        match repository_candidate_dirs(&directory) {
            Ok(children) => {
                for child in children {
                    queue.push_back((child, has_project));
                }
            }
            Err(_) => result.evidence.read_failed = true,
        }
    }

    result.evidence.read_failed |= parsed_cache.read_failed() || file_cache.read_failed();
    result.evidence.read_files.extend(
        parsed_cache
            .read_paths()
            .iter()
            .map(|path| path.to_string_lossy().into_owned()),
    );
    result.evidence.read_files.sort();
    result.evidence.read_files.dedup();
    result.evidence.watched_dirs.sort();
    result.evidence.watched_dirs.dedup();
    result
        .evidence
        .set_scan_metrics(files_matched, started.elapsed().as_millis() as u64);
    result
}

fn repository_candidate_dirs(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut directories = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.starts_with('.') && DirectoryType::classify(&name) != DirectoryType::Noise {
            directories.push(entry.path());
        }
    }
    directories.sort();
    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_nearest_vcs_root_wins_beyond_the_old_depth_cap() -> TestResult {
        let dir = TempDir::new()?;
        fs::create_dir(dir.path().join(".git"))?;
        let start = dir.path().join("a/b/c/d/e/f");
        fs::create_dir_all(&start)?;

        assert_eq!(find_vcs_root(&start), Some(dir.path().canonicalize()?));
        Ok(())
    }

    #[test]
    fn test_git_file_hg_and_svn_markers_are_repository_roots() -> TestResult {
        for (marker, is_file) in [(".git", true), (".hg", false), (".svn", false)] {
            let dir = TempDir::new()?;
            if is_file {
                fs::write(dir.path().join(marker), "gitdir: /elsewhere\n")?;
            } else {
                fs::create_dir(dir.path().join(marker))?;
            }
            let start = dir.path().join("nested/project");
            fs::create_dir_all(&start)?;

            assert_eq!(
                find_vcs_root_with_home(&start, None),
                Some(dir.path().canonicalize()?),
                "{marker}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_nested_repository_uses_its_own_root() -> TestResult {
        let dir = TempDir::new()?;
        let inner = dir.path().join("inner");
        fs::create_dir_all(dir.path().join(".git"))?;
        fs::create_dir_all(inner.join(".git"))?;
        let start = inner.join("deep/path");
        fs::create_dir_all(&start)?;

        assert_eq!(
            find_vcs_root_with_home(&start, None),
            Some(inner.canonicalize()?)
        );
        Ok(())
    }

    #[test]
    fn test_home_and_system_boundaries_stop_discovery() -> TestResult {
        let home = TempDir::new()?;
        let start = home.path().join("deep/project");
        fs::create_dir_all(&start)?;

        assert_eq!(find_vcs_root_with_home(&start, Some(home.path())), None);
        assert!(is_boundary_directory(home.path(), Some(home.path())));
        assert!(is_boundary_directory(Path::new("/"), None));
        assert!(is_boundary_directory(Path::new("/tmp"), None));
        assert_eq!(find_vcs_root_with_home(Path::new("/"), None), None);
        Ok(())
    }

    mod aggregation {
        use super::*;
        use crate::detection::DetectionEngine;
        use crate::output::{render, OutputFormat};
        use crate::types::DetectionResult;
        use std::time::Duration;

        fn engine() -> DetectionEngine {
            let config = crate::config::built_in_config();
            DetectionEngine::with_config(config.indicators, config.frameworks, config.detection)
        }

        fn write(root: &Path, files: &[(&str, &str)]) -> TestResult {
            for (relative, content) in files {
                let path = root.join(relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(path, content)?;
            }
            Ok(())
        }

        fn has_java_spring(result: &DetectionResult) -> bool {
            result.nested.iter().any(|project| {
                project.indicator.name == "Java"
                    && project
                        .frameworks
                        .iter()
                        .any(|framework| framework.framework.name == "Spring Boot")
            })
        }

        #[test]
        fn test_deep_java_directories_share_repository_result() -> TestResult {
            let dir = TempDir::new()?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    (
                        "pom.xml",
                        "<project><dependencies><dependency><artifactId>spring-boot-starter</artifactId></dependency></dependencies></project>",
                    ),
                    ("src/main/java/com/acme/foo/Hello.java", "class Hello {}"),
                    ("src/main/java/com/acme/foo/internal/.keep", ""),
                ],
            )?;
            let engine = engine();

            for relative in [
                "src/main/java/com/acme",
                "src/main/java/com/acme/foo",
                "src/main/java/com/acme/foo/internal",
                ".",
            ] {
                let result = engine.detect(&dir.path().join(relative))?;
                assert!(result.indicator.is_none(), "{relative}");
                assert!(result.evidence.repository, "{relative}");
                assert!(has_java_spring(&result), "{relative}: {result:?}");
            }
            Ok(())
        }

        #[test]
        fn test_repository_aggregates_all_projects_beyond_old_limits() -> TestResult {
            let dir = TempDir::new()?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    ("Cargo.toml", "[package]\nname = \"root\"\n"),
                    ("apps/web/package.json", r#"{"dependencies":{"react":"18"}}"#),
                    ("services/api/go.mod", "module x\n"),
                    ("tools/python/pyproject.toml", "[project]\nname = \"x\"\ndependencies = [\"django\"]\n"),
                    ("deep/jvm/pom.xml", "<project><dependencies><dependency><artifactId>spring-boot-starter</artifactId></dependency></dependencies></project>"),
                    ("extra/csharp/App.csproj", "<Project><ItemGroup><PackageReference Include=\"Microsoft.AspNetCore.App\" /></ItemGroup></Project>"),
                ],
            )?;

            let result = detect_repository(&engine(), dir.path());

            assert!(result.indicator.is_none());
            assert!(result.evidence.repository);
            let names: Vec<&str> = result
                .nested
                .iter()
                .map(|project| project.name.as_str())
                .collect();
            assert_eq!(
                names,
                [
                    ".",
                    "apps/web",
                    "deep/jvm",
                    "extra/csharp",
                    "services/api",
                    "tools/python"
                ]
            );
            Ok(())
        }

        #[test]
        fn test_repository_output_deduplicates_repeated_indicators() -> TestResult {
            let dir = TempDir::new()?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    ("backend-a/Cargo.toml", "[package]\nname = \"a\"\n"),
                    ("backend-b/Cargo.toml", "[package]\nname = \"b\"\n"),
                    (
                        "frontend/package.json",
                        r#"{"dependencies":{"react":"18"}}"#,
                    ),
                ],
            )?;
            let result = detect_repository(&engine(), dir.path());
            let rust = result.nested[0].clone();
            let react = result.nested[2].clone();
            let rust_icon = rust.display_icon().ok_or("Rust icon")?;
            let rust_color = rust.display_color().ok_or("Rust color")?;
            let react_icon = react.display_icon().ok_or("React icon")?;

            let simple = render(&result, OutputFormat::Simple, &Default::default());
            assert_eq!(simple.matches(rust_icon).count(), 1);
            assert_eq!(simple.matches(react_icon).count(), 1);
            let full = render(&result, OutputFormat::Full, &Default::default());
            let component = format!("{rust_icon}|{rust_color}");
            assert_eq!(full.matches(&component).count(), 1);
            let json: serde_json::Value =
                serde_json::from_str(&render(&result, OutputFormat::Json, &Default::default()))?;
            assert_eq!(json["projects"].as_array().map(Vec::len), Some(3));
            Ok(())
        }

        #[test]
        fn test_repository_detects_source_only_project_and_skips_noise() -> TestResult {
            let dir = TempDir::new()?;
            let external = TempDir::new()?;
            write(
                external.path(),
                &[("Cargo.toml", "[package]\nname = \"x\"\n")],
            )?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    ("src/main/java/com/acme/Main.java", "class Main {}"),
                    ("node_modules/pkg/package.json", "{}"),
                    (".hidden/Cargo.toml", "[package]\nname = \"hidden\"\n"),
                ],
            )?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(external.path(), dir.path().join("linked"))?;

            let result = detect_repository(&engine(), dir.path());

            assert_eq!(result.nested.len(), 1);
            assert_eq!(result.nested[0].name, ".");
            assert_eq!(result.nested[0].indicator.name, "Java");
            Ok(())
        }

        #[test]
        fn test_repository_timeout_is_not_cacheable() -> TestResult {
            let dir = TempDir::new()?;
            write(dir.path(), &[(".git/HEAD", "")])?;

            let result = detect_repository_with_timeout(&engine(), dir.path(), Duration::ZERO);

            assert!(result.evidence.scan_truncated);
            assert!(!result.evidence.is_cacheable());
            Ok(())
        }
    }
}
