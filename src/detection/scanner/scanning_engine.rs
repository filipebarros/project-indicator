use super::{traverser::build_walker, TimeoutManager};
use crate::detection::pattern_processor::{normalized_relative, PatternProcessor};
use crate::types::{DetectionEvidence, MatchedFile};
use std::collections::HashMap;
use std::path::Path;

/// What a scan found and which directories it depended on.
#[derive(Default)]
pub struct ScanOutcome {
    pub matches: Vec<MatchedFile>,
    /// The timeout cut the walk short (the matches are a partial sample)
    pub truncated: bool,
    /// Directories, relative to the scan root with `/` separators, whose
    /// contents shaped the matches (see `DetectionEvidence::watched_dirs`)
    pub watched_dirs: Vec<String>,
}

/// Maximum matches the walk collects before it stops. Exact-name root
/// manifests are seeded on top of this, and when the cap is hit a root-only
/// pass may add up to this many more root-level files, so a result holds at
/// most `2 * MAX_MATCHES` plus the exact manifests.
///
/// 15 leaves a wide margin over the <10 matches typical projects need, and
/// bounds memory and scan time in large monorepos.
const MAX_MATCHES: usize = 15;

/// Roots with more immediate entries than this are not walked (home and
/// system directories have 500-5000+, project roots 10-400): they only get
/// the root-level manifest check.
pub(crate) const EXTREME_SIZE_THRESHOLD: usize = 500;

pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
    max_depth: usize,
}

impl ScanningEngine {
    pub fn new(pattern_processor: PatternProcessor, max_depth: usize) -> Self {
        Self {
            pattern_processor,
            max_depth,
        }
    }

    /// One `read_dir` of the scan root: the total entry count (for the size
    /// guard and timeout) and the sorted names of the files in it (symlinks
    /// to files included).
    fn list_root(path: &Path) -> (usize, Vec<String>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return (0, Vec::new());
        };
        let (mut count, mut names) = (0, Vec::new());
        for entry in entries.filter_map(|e| e.ok()) {
            count += 1;
            let is_file = entry
                .file_type()
                .is_ok_and(|ft| ft.is_file() || (ft.is_symlink() && entry.path().is_file()));
            if let (true, Ok(name)) = (is_file, entry.file_name().into_string()) {
                names.push(name);
            }
        }
        names.sort();
        (count, names)
    }

    /// Exact-name pattern files present in the root, in sorted order
    fn root_manifest_matches(&self, names: &[String]) -> Vec<MatchedFile> {
        let exact = self.pattern_processor.get_exact_patterns();
        names
            .iter()
            .filter(|name| exact.contains(name.as_str()))
            .map(|name| MatchedFile::new(name.clone(), name.clone()))
            .collect()
    }

    /// Exact patterns that are relative paths (`src/main/java`) are probed,
    /// since the default depth never reaches them. Returns the matches and
    /// the directories to watch: the deepest existing parent of each probe,
    /// whose mtime changes when the next component is created. Existence is
    /// checked prefix by prefix and shared across probes, so a project
    /// without `src/` pays a single `stat`. Sorted for a stable result.
    fn probe_paths(&self, root: &Path) -> (Vec<MatchedFile>, Vec<String>) {
        let mut patterns: Vec<&String> = self
            .pattern_processor
            .get_exact_patterns()
            .iter()
            .filter(|pattern| pattern.contains('/'))
            .collect();
        patterns.sort();

        let mut exists: HashMap<&str, bool> = HashMap::new();
        let (mut matches, mut watched) = (Vec::new(), Vec::new());
        for pattern in patterns {
            let mut parent: Option<&str> = None;
            let prefixes = pattern
                .match_indices('/')
                .map(|(i, _)| &pattern[..i])
                .chain(std::iter::once(pattern.as_str()));
            let mut all_exist = true;
            for prefix in prefixes {
                let found = *exists
                    .entry(prefix)
                    .or_insert_with(|| root.join(prefix).exists());
                if !found {
                    all_exist = false;
                    break;
                }
                if prefix.len() < pattern.len() {
                    parent = Some(prefix);
                }
            }
            if all_exist {
                matches.push(MatchedFile::new(pattern.clone(), pattern.clone()));
            }
            if let Some(parent) = parent {
                watched.push(parent.to_string());
            }
        }
        watched.sort();
        watched.dedup();
        (matches, watched)
    }

    /// Root-level files matching a wildcard pattern (`*.csproj`, `*.gemspec`),
    /// in name order and capped, skipping names already in `already`.
    fn root_wildcard_matches(&self, names: &[String], already: &[MatchedFile]) -> Vec<MatchedFile> {
        names
            .iter()
            .filter(|name| {
                self.pattern_processor.should_scan_file(name)
                    && !already.iter().any(|m| &m.filename == *name)
            })
            .take(MAX_MATCHES)
            .map(|name| MatchedFile::new(name.clone(), name.clone()))
            .collect()
    }

    /// Full directory scan: a sorted, deterministic walk bounded by the match
    /// cap and an adaptive timeout. Oversized directories (likely a home or
    /// system dir) only get a root-level check, and `max_depth == 0` scans
    /// nothing. Also reports whether the timeout cut the walk short (the
    /// matches are then a partial sample) and the directories it depended on.
    pub fn scan_with_status(&self, path: &Path) -> ScanOutcome {
        log::trace!("Starting scan for path: {}", path.display());
        if self.max_depth == 0 {
            return ScanOutcome::default();
        }
        let (entry_count, root_names) = Self::list_root(path);

        if entry_count > EXTREME_SIZE_THRESHOLD {
            log::debug!(
                "Directory {} has {} immediate entries (>{} threshold), likely not a project root. Skipping scan.",
                path.display(),
                entry_count,
                EXTREME_SIZE_THRESHOLD
            );
            // Root-level matches are still cheap, so an oversized project
            // root (e.g. a big monorepo) is not reported as empty
            let mut matches = self.root_manifest_matches(&root_names);
            matches.extend(self.root_wildcard_matches(&root_names, &matches));
            let (probed, watched_dirs) = self.probe_paths(path);
            matches.extend(probed);
            return ScanOutcome {
                matches,
                truncated: false,
                watched_dirs,
            };
        }

        let (probed, probe_dirs) = self.probe_paths(path);
        let mut seeded = self.root_manifest_matches(&root_names);
        seeded.extend(probed);
        let mut outcome = self.scan_sequential(path, TimeoutManager::new(entry_count), seeded);
        outcome.watched_dirs.extend(probe_dirs);
        outcome.watched_dirs.sort();
        outcome.watched_dirs.dedup();
        outcome
    }

    fn prefilter_entry(pattern_processor: &PatternProcessor, entry: &ignore::DirEntry) -> bool {
        // Symlinks are resolved to their targets so a symlinked manifest counts as a file
        // (matching the extreme-size root check, which stats through
        // symlinks); broken links resolve to nothing and are skipped.
        let is_file = entry
            .file_type()
            .is_some_and(|ft| ft.is_file() || (ft.is_symlink() && entry.path().is_file()));
        if !is_file {
            return false;
        }

        let Some(filename) = entry.file_name().to_str() else {
            return false;
        };

        pattern_processor.should_scan_file(filename)
    }

    fn collect_match(&self, entry: &ignore::DirEntry, base: &Path, matched: &mut Vec<MatchedFile>) {
        if !Self::prefilter_entry(&self.pattern_processor, entry) {
            return;
        }
        if let Some(file_match) = self
            .pattern_processor
            .match_file_against_patterns(entry.path(), base)
        {
            if !matched
                .iter()
                .any(|m| m.relative_path == file_match.relative_path)
            {
                matched.push(file_match);
            }
        }
    }

    /// The one scan path: a sequential, name-sorted walk
    fn scan_sequential(
        &self,
        path: &Path,
        timeout_mgr: TimeoutManager,
        root_manifests: Vec<MatchedFile>,
    ) -> ScanOutcome {
        let scan_start = std::time::Instant::now();
        let mut timed_out = false;
        let mut watched_dirs = Vec::new();
        // Seeded exact-name manifests don't count against the cap on the walk
        let mut matched_files = root_manifests;
        let root_count = matched_files.len();
        let mut capped = false;

        for result in build_walker(path, self.max_depth).build() {
            if matched_files.len() - root_count >= MAX_MATCHES {
                log::debug!("Sequential scan: max matches ({}) reached", MAX_MATCHES);
                capped = true;
                break;
            }

            if timeout_mgr.should_stop() {
                timed_out = true;
                break;
            }

            let entry = match result {
                Ok(entry) => entry,
                Err(e) => {
                    log::debug!("Scan: skipped an unreadable entry: {e}");
                    continue;
                }
            };
            // A listed subdirectory (depth 1..max_depth) shapes the result
            if entry.depth() >= 1
                && entry.depth() < self.max_depth
                && watched_dirs.len() <= DetectionEvidence::MAX_WATCHED_DIRS
                && entry.file_type().is_some_and(|ft| ft.is_dir())
            {
                if let Some(relative) = normalized_relative(entry.path(), path) {
                    watched_dirs.push(relative);
                }
            }
            self.collect_match(&entry, path, &mut matched_files);
        }

        // The walk is name-sorted, so a busy subdirectory can use up the cap
        // before a root-level marker (`App.csproj` after `Controllers/`) is
        // reached. Top up from a root-only pass, which still honours
        // .gitignore and the depth limit.
        if capped {
            let before = matched_files.len();
            for entry in build_walker(path, self.max_depth.min(1)).build().flatten() {
                if matched_files.len() - before >= MAX_MATCHES {
                    break;
                }
                self.collect_match(&entry, path, &mut matched_files);
            }
        }

        let elapsed = scan_start.elapsed().as_millis();
        log::debug!(
            "Scan complete: {} matches in {}ms",
            matched_files.len(),
            elapsed
        );

        ScanOutcome {
            matches: matched_files,
            truncated: timed_out,
            watched_dirs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_rust_project;
    use crate::Result;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn scan(engine: &ScanningEngine, path: &Path) -> Vec<MatchedFile> {
        engine.scan_with_status(path).matches
    }

    fn create_test_engine() -> ScanningEngine {
        let patterns = Arc::from(vec![
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
            "*.js".to_string(),
        ]);

        let pattern_processor = PatternProcessor::new(patterns);

        ScanningEngine::new(pattern_processor, 3)
    }

    #[test]
    fn test_root_wildcard_marker_survives_match_cap() -> Result<()> {
        let processor =
            PatternProcessor::new(Arc::from(vec!["*.cs".to_string(), "*.csproj".to_string()]));
        let engine = ScanningEngine::new(processor, 3);
        let temp_dir = TempDir::new()?;
        let controllers = temp_dir.path().join("Controllers");
        fs::create_dir(&controllers)?;
        for i in 0..(MAX_MATCHES + 5) {
            fs::write(controllers.join(format!("C{i:02}.cs")), "")?;
        }
        fs::write(temp_dir.path().join("MyApp.csproj"), "")?;

        let matches = scan(&engine, temp_dir.path());
        assert!(
            matches.iter().any(|m| m.relative_path == "MyApp.csproj"),
            "root *.csproj must not be crowded out by a busy subdirectory"
        );
        Ok(())
    }

    fn js_engine(max_depth: usize) -> ScanningEngine {
        let patterns = Arc::from(vec!["*.js".to_string(), "package.json".to_string()]);
        ScanningEngine::new(PatternProcessor::new(patterns), max_depth)
    }

    #[test]
    fn test_root_manifest_seeded_and_walked_is_reported_once() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("package.json"), "{}")?;

        let names: Vec<String> = scan(&js_engine(2), temp_dir.path())
            .into_iter()
            .map(|m| m.relative_path)
            .collect();
        assert_eq!(names, vec!["package.json".to_string()]);
        Ok(())
    }

    #[test]
    fn test_gitignored_root_file_is_not_a_match() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join(".gitignore"), "bundle.js\n")?;
        fs::write(temp_dir.path().join("bundle.js"), "")?;
        fs::write(temp_dir.path().join("app.js"), "")?;

        let names: Vec<String> = scan(&js_engine(2), temp_dir.path())
            .into_iter()
            .map(|m| m.filename)
            .collect();
        assert_eq!(names, vec!["app.js".to_string()]);
        Ok(())
    }

    #[test]
    fn test_depth_zero_scans_nothing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("package.json"), "{}")?;
        fs::write(temp_dir.path().join("app.js"), "")?;

        assert!(scan(&js_engine(0), temp_dir.path()).is_empty());
        Ok(())
    }

    #[test]
    fn test_match_count_stays_bounded_when_cap_is_hit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("package.json"), "{}")?;
        for i in 0..100 {
            fs::write(temp_dir.path().join(format!("f{i:03}.js")), "")?;
        }
        fs::create_dir(temp_dir.path().join("src"))?;
        for i in 0..31 {
            fs::write(temp_dir.path().join("src").join(format!("m{i:02}.js")), "")?;
        }

        let matches = scan(&js_engine(2), temp_dir.path());
        // exact manifest + walk cap + root-level top-up
        assert_eq!(matches.len(), 1 + 2 * MAX_MATCHES);
        let unique: std::collections::HashSet<_> =
            matches.iter().map(|m| m.relative_path.as_str()).collect();
        assert_eq!(unique.len(), matches.len(), "top-up must not duplicate");
        // the walk's matches come first, the top-up's root files after them
        assert_eq!(matches[0].relative_path, "package.json");
        assert_eq!(matches[1].relative_path, "f000.js");
        assert_eq!(matches[MAX_MATCHES + 1].relative_path, "f015.js");
        Ok(())
    }

    #[test]
    fn test_ancestor_gitignore_does_not_hide_nested_files() -> Result<()> {
        let parent = TempDir::new()?;
        fs::write(parent.path().join(".gitignore"), "*.js\n")?;
        let project = parent.path().join("proj");
        fs::create_dir_all(project.join("src"))?;
        fs::write(project.join("src").join("m.js"), "")?;

        let matches = scan(&js_engine(2), &project);
        assert!(matches.iter().any(|m| m.relative_path == "src/m.js"));
        Ok(())
    }

    fn probe_engine(max_depth: usize) -> ScanningEngine {
        let patterns = Arc::from(vec![
            "src/main/kotlin".to_string(),
            "src/main/java".to_string(),
        ]);
        ScanningEngine::new(PatternProcessor::new(patterns), max_depth)
    }

    #[test]
    fn test_source_directory_probes_are_seeded_in_sorted_order() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("src/main/kotlin"))?;
        fs::create_dir_all(temp_dir.path().join("src/main/java"))?;

        let paths: Vec<String> = scan(&probe_engine(1), temp_dir.path())
            .into_iter()
            .map(|m| m.relative_path)
            .collect();
        assert_eq!(paths, vec!["src/main/java", "src/main/kotlin"]);
        Ok(())
    }

    #[test]
    fn test_source_directory_probes_respect_depth_zero() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("src/main/java"))?;

        assert!(scan(&probe_engine(0), temp_dir.path()).is_empty());
        Ok(())
    }

    #[test]
    fn test_listed_subdirectories_are_reported_as_watched() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("a/b/c"))?;

        // depth 3 lists a (1) and a/b (2); a/b/c is yielded but not listed
        let outcome = js_engine(3).scan_with_status(temp_dir.path());
        assert_eq!(outcome.watched_dirs, vec!["a", "a/b"]);

        // depth 1 lists only the root, which the cache always watches
        let outcome = js_engine(1).scan_with_status(temp_dir.path());
        assert!(outcome.watched_dirs.is_empty());
        Ok(())
    }

    /// `.git` changes on every git operation: watching it would invalidate the
    /// persistent cache constantly, and its contents never hold project files.
    #[test]
    fn test_git_directory_is_neither_walked_nor_watched() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join(".git/objects/ab"))?;
        fs::write(temp_dir.path().join(".git/objects/ab/package.json"), "")?;
        fs::create_dir_all(temp_dir.path().join("src"))?;

        let outcome = js_engine(4).scan_with_status(temp_dir.path());

        assert_eq!(outcome.watched_dirs, vec!["src"]);
        assert!(outcome.matches.is_empty(), "{:?}", outcome.matches);
        Ok(())
    }

    #[test]
    fn test_probe_watches_nearest_existing_ancestor() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("src"))?;

        // src exists, src/main does not: creating it changes src's mtime
        let outcome = probe_engine(1).scan_with_status(temp_dir.path());
        assert_eq!(outcome.watched_dirs, vec!["src"]);

        fs::create_dir_all(temp_dir.path().join("src/main"))?;
        let outcome = probe_engine(1).scan_with_status(temp_dir.path());
        assert_eq!(outcome.watched_dirs, vec!["src/main"]);
        Ok(())
    }

    #[test]
    fn test_scan_with_no_matches() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;

        fs::write(temp_dir.path().join("readme.txt"), "text")?;

        let matches = scan(&engine, temp_dir.path());
        assert!(matches.is_empty());

        Ok(())
    }

    #[test]
    fn test_list_root_counts_everything_and_lists_sorted_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        fs::create_dir(root.join("a_dir"))?;
        fs::write(root.join("b.txt"), "x")?;
        fs::write(root.join("a.txt"), "x")?;

        let (count, names) = ScanningEngine::list_root(root);
        assert_eq!(count, 3, "directories count towards the size guard");
        assert_eq!(names, vec!["a.txt".to_string(), "b.txt".to_string()]);

        assert_eq!(
            ScanningEngine::list_root(&root.join("missing")),
            (0, vec![])
        );
        assert_eq!(
            ScanningEngine::list_root(TempDir::new()?.path()),
            (0, vec![])
        );
        Ok(())
    }

    #[test]
    fn test_oversized_directory_still_detects_root_manifests() -> Result<()> {
        let patterns = Arc::from(vec!["special.config".to_string(), "*.xyz".to_string()]);
        // Detection must survive the extreme-size bailout via the root
        // exact-pattern check
        let pattern_processor = PatternProcessor::new(patterns);
        let engine = ScanningEngine::new(pattern_processor, 3);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..510 {
            fs::write(root.join(format!("file_{}.txt", i)), "x")?;
        }
        fs::write(root.join("special.config"), "cfg")?;

        let matches = scan(&engine, root);
        assert!(matches.iter().any(|m| m.filename == "special.config"));

        Ok(())
    }

    #[test]
    fn test_scan_adaptive_timeout() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        let src = root.join("src");
        fs::create_dir(&src)?;
        fs::write(src.join("lib.rs"), "// lib")?;

        let matches = scan(&engine, root);
        assert!(
            matches.iter().any(|m| m.relative_path == "src/lib.rs"),
            "nested file within max_depth must be found: {matches:?}"
        );

        Ok(())
    }

    #[test]
    fn test_repeated_scans_are_identical() -> Result<()> {
        let engine = create_test_engine();
        let temp_dir = create_test_rust_project().map_err(|e| anyhow::anyhow!("{}", e))?;

        let paths =
            |v: Vec<MatchedFile>| v.into_iter().map(|m| m.relative_path).collect::<Vec<_>>();
        let first = paths(scan(&engine, temp_dir.path()));
        assert!(!first.is_empty());
        for _ in 0..3 {
            assert_eq!(first, paths(scan(&engine, temp_dir.path())));
        }

        Ok(())
    }

    #[test]
    fn test_scan_ignores_files_beyond_max_depth() -> Result<()> {
        let patterns = Arc::from(vec!["*.rs".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let engine = ScanningEngine::new(pattern_processor, 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many nested directories to potentially trigger timeout
        for i in 0..50 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            for j in 0..10 {
                let subdir = dir.join(format!("subdir_{}", j));
                fs::create_dir(&subdir)?;
                fs::write(subdir.join("lib.rs"), "// lib")?;
            }
        }

        // lib.rs files sit three levels down, beyond max_depth = 1
        let matches = scan(&engine, root);
        assert!(matches.is_empty(), "files beyond max_depth: {matches:?}");

        Ok(())
    }

    #[test]
    fn test_busy_directory_scan_is_deterministic_and_keeps_root_manifest() -> Result<()> {
        // 60 matching sources + a root manifest: the match cap must never
        // crowd the manifest out, and repeated scans must agree
        let patterns = Arc::from(vec![
            "*.js".to_string(),
            "*.rs".to_string(),
            "Cargo.toml".to_string(),
        ]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..60 {
            fs::write(root.join(format!("file_{i:02}.js")), "x")?;
        }
        for i in 0..3 {
            fs::write(root.join(format!("m{i}.rs")), "x")?;
        }
        fs::write(root.join("Cargo.toml"), "[package]")?;

        let first = scan(&engine, root);
        assert!(
            first.iter().any(|m| m.filename == "Cargo.toml"),
            "root manifest must survive the match cap"
        );
        for _ in 0..20 {
            let again = scan(&engine, root);
            let names = |v: &[MatchedFile]| {
                v.iter()
                    .map(|m| m.relative_path.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&first), names(&again), "scan results must be stable");
        }
        Ok(())
    }

    #[test]
    fn test_oversized_directory_keeps_wildcard_root_markers() -> Result<()> {
        let patterns = Arc::from(vec!["*.csproj".to_string(), "go.mod".to_string()]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..510 {
            fs::write(root.join(format!("file_{i}.txt")), "x")?;
        }
        fs::write(root.join("App.csproj"), "<Project/>")?;
        fs::write(root.join("go.mod"), "module x")?;

        let names: Vec<String> = scan(&engine, root)
            .into_iter()
            .map(|m| m.filename)
            .collect();
        assert!(names.contains(&"go.mod".to_string()));
        assert!(
            names.contains(&"App.csproj".to_string()),
            "wildcard-only marker lost in an oversized directory: {names:?}"
        );
        Ok(())
    }

    #[test]
    fn test_timed_out_scan_is_reported_truncated() -> Result<()> {
        let patterns = Arc::from(vec!["*.rs".to_string()]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns), 1);
        let temp_dir = TempDir::new()?;
        for i in 0..20 {
            fs::write(temp_dir.path().join(format!("f{i}.rs")), "x")?;
        }

        let timed = engine.scan_sequential(
            temp_dir.path(),
            TimeoutManager::with_timeout(std::time::Duration::ZERO),
            Vec::new(),
        );
        assert!(timed.truncated, "an expired timeout must be reported");

        let ScanOutcome {
            matches,
            truncated: timed_out,
            ..
        } = engine.scan_with_status(temp_dir.path());
        assert!(!timed_out && !matches.is_empty());
        Ok(())
    }
}
