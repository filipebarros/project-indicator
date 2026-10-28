use crate::detection::caches::ParsedFileCache;
use crate::detection::directory::DirectoryListing;
use crate::detection::engine::{merge_child_evidence, DetectionEngine};
use crate::detection::pattern_processor::normalized_relative;
use crate::detection::root_indicators::VCS_MARKERS;
use crate::detection::scanner::traverser::root_ignore_files_are_safe;
use crate::performance::FileSystemCache;
use crate::types::{DetectionEvidence, DetectionResult, DirectoryType, NestedProject};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

struct RepositoryScan<'a> {
    engine: &'a DetectionEngine,
    repository_root: &'a Path,
    started: Instant,
    result: DetectionResult,
    file_cache: FileSystemCache,
    parsed_cache: ParsedFileCache,
    files_matched: usize,
}

impl<'a> RepositoryScan<'a> {
    fn new(engine: &'a DetectionEngine, repository_root: &'a Path, started: Instant) -> Self {
        let evidence = DetectionEvidence {
            repository: true,
            scan_root: Some(repository_root.to_path_buf()),
            ..DetectionEvidence::default()
        };
        Self {
            engine,
            repository_root,
            started,
            result: DetectionResult::new_with_evidence(None, Vec::new(), 0.0, evidence),
            file_cache: FileSystemCache::default(),
            parsed_cache: ParsedFileCache::new(),
            files_matched: 0,
        }
    }

    fn scan_directory(&mut self, directory: &Path, has_project_ancestor: bool) -> bool {
        self.result
            .evidence
            .watched_dirs
            .push(directory.to_string_lossy().into_owned());
        let listing = DirectoryListing::read(directory);
        let scan = self
            .engine
            .scanning_engine
            .scan_with_listing(directory, &listing);
        self.files_matched += scan.matches.len();
        self.result.evidence.scan_truncated |= scan.truncated;
        self.result.evidence.read_failed |= scan.unreadable;
        self.result.evidence.watched_dirs.extend(
            scan.watched_dirs
                .iter()
                .map(|watched| directory.join(watched).to_string_lossy().into_owned()),
        );

        let mut has_project = has_project_ancestor;
        if !scan.matches.is_empty() {
            let markers = self.engine.root_indicator_engine.qualifying_markers_in(
                directory,
                &listing,
                &self.file_cache,
                &self.parsed_cache,
            );
            if !has_project || !markers.is_empty() {
                let found = self.engine.detect_one_with_scan(directory, markers, scan);
                merge_child_evidence(&mut self.result.evidence, directory, &found.evidence);
                if let Some(indicator) = found.indicator {
                    let name = normalized_relative(directory, self.repository_root)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| ".".to_string());
                    self.result.nested.push(NestedProject {
                        name,
                        indicator,
                        frameworks: found.frameworks,
                    });
                    has_project = true;
                }
            }
        }
        has_project
    }

    fn finish(mut self) -> DetectionResult {
        self.result.evidence.read_failed |=
            self.parsed_cache.read_failed() || self.file_cache.read_failed();
        self.result.evidence.watched_dirs.extend(
            self.file_cache
                .watched_dirs()
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        self.result.evidence.read_files.extend(
            self.parsed_cache
                .read_paths()
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        self.result.evidence.read_files.sort();
        self.result.evidence.read_files.dedup();
        self.result.evidence.watched_dirs.sort();
        self.result.evidence.watched_dirs.dedup();
        self.result.nested.sort_by(|a, b| {
            let depth = |name: &str| {
                if name == "." {
                    0
                } else {
                    name.split('/').count()
                }
            };
            depth(&a.name)
                .cmp(&depth(&b.name))
                .then_with(|| a.name.cmp(&b.name))
        });
        self.result.evidence.set_scan_metrics(
            self.files_matched,
            self.started.elapsed().as_millis() as u64,
        );
        self.result
    }
}

pub fn detect_repository(engine: &DetectionEngine, repository_root: &Path) -> DetectionResult {
    detect_repository_with_timeout(engine, repository_root, REPOSITORY_SCAN_TIMEOUT)
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
    if root_ignore_files_are_safe(&repository_root) {
        if let Some(result) =
            detect_repository_walk(engine, &repository_root, started, deadline, true)
        {
            return result;
        }
    }
    match detect_repository_walk(engine, &repository_root, started, deadline, false) {
        Some(result) => result,
        None => unreachable!("an unfiltered walk cannot encounter unsafe ignore files"),
    }
}

fn detect_repository_walk(
    engine: &DetectionEngine,
    repository_root: &Path,
    started: Instant,
    deadline: Instant,
    respect_ignores: bool,
) -> Option<DetectionResult> {
    let unsafe_ignore = Arc::new(AtomicBool::new(false));
    let unsafe_in_filter = unsafe_ignore.clone();
    let mut builder = WalkBuilder::new(repository_root);
    builder
        .standard_filters(true)
        .git_ignore(respect_ignores)
        .ignore(respect_ignores)
        .git_exclude(respect_ignores)
        .parents(false)
        .git_global(false)
        .require_git(false)
        .hidden(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                return false;
            }
            let Some(name) = entry.file_name().to_str() else {
                return false;
            };
            if name.starts_with('.') || DirectoryType::classify(name) == DirectoryType::Noise {
                return false;
            }
            if respect_ignores && !root_ignore_files_are_safe(entry.path()) {
                unsafe_in_filter.store(true, Ordering::Relaxed);
                return false;
            }
            true
        });

    let mut scan = RepositoryScan::new(engine, repository_root, started);
    let mut project_roots = HashSet::new();
    for entry in builder.build() {
        if unsafe_ignore.load(Ordering::Relaxed) {
            return None;
        }
        if Instant::now() >= deadline {
            scan.result.evidence.scan_truncated = true;
            break;
        }
        match entry {
            Ok(entry) => {
                let directory = entry.into_path();
                let has_project_ancestor = directory
                    .ancestors()
                    .skip(1)
                    .take_while(|ancestor| ancestor.starts_with(repository_root))
                    .any(|ancestor| project_roots.contains(ancestor));
                let has_project = scan.scan_directory(&directory, has_project_ancestor);
                if has_project && !has_project_ancestor {
                    project_roots.insert(directory);
                }
            }
            Err(error) => {
                log::debug!("Repository walk skipped an unreadable entry: {error}");
                scan.result.evidence.read_failed = true;
            }
        }
    }
    if unsafe_ignore.load(Ordering::Relaxed) {
        return None;
    }
    Some(scan.finish())
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
        fn test_repository_prunes_ignored_directories_and_honours_negations() -> TestResult {
            let dir = TempDir::new()?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    (".git/info/exclude", "excluded/\n"),
                    (".gitignore", "ignored/\n"),
                    (".ignore", "also-ignored/\n"),
                    ("actual/package.json", "{}"),
                    ("ignored/package.json", "{}"),
                    ("also-ignored/package.json", "{}"),
                    ("excluded/package.json", "{}"),
                    ("config/.gitignore", "generated/*\n!generated/keep/\n"),
                    ("config/generated/drop/package.json", "{}"),
                    ("config/generated/keep/package.json", "{}"),
                ],
            )?;

            let result = detect_repository(&engine(), dir.path());

            let names: Vec<&str> = result
                .nested
                .iter()
                .map(|project| project.name.as_str())
                .collect();
            assert_eq!(names, ["actual", "config/generated/keep"]);
            assert!(result
                .evidence
                .watched_dirs
                .iter()
                .all(|path| !path.contains("generated/drop")));
            assert!(!result.evidence.read_failed);
            Ok(())
        }

        #[test]
        fn test_oversized_ignore_files_fail_open_without_being_read() -> TestResult {
            for (ignore_file, project_file, expected) in [
                (".gitignore", "ignored/package.json", "ignored"),
                (
                    "config/.gitignore",
                    "config/ignored/package.json",
                    "config/ignored",
                ),
            ] {
                let dir = TempDir::new()?;
                write(
                    dir.path(),
                    &[
                        (".git/HEAD", ""),
                        (ignore_file, &format!("ignored/\n{}", " ".repeat(1_048_576))),
                        (project_file, "{}"),
                    ],
                )?;

                let result = detect_repository(&engine(), dir.path());

                assert_eq!(result.nested[0].name, expected);
                assert!(!result.evidence.read_failed);
            }
            Ok(())
        }

        #[test]
        fn test_ignore_type_precedence_uses_walker_semantics() -> TestResult {
            let dir = TempDir::new()?;
            write(
                dir.path(),
                &[
                    (".git/HEAD", ""),
                    (".ignore", "generated/\n"),
                    ("config/.gitignore", "!generated/\n"),
                    ("config/generated/package.json", "{}"),
                ],
            )?;

            let result = detect_repository(&engine(), dir.path());

            assert!(result.nested.is_empty());
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
