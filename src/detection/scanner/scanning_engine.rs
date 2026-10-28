use super::traverser::build_walker;
use crate::detection::directory::DirectoryListing;
use crate::detection::pattern_processor::PatternProcessor;
use crate::performance::smallest;
use crate::types::MatchedFile;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

/// What a scan found and which directories it depended on.
#[derive(Default)]
pub struct ScanOutcome {
    pub matches: Vec<MatchedFile>,
    /// The timeout cut the walk short (the matches are a partial sample)
    pub truncated: bool,
    /// Directories, relative to the scan root with `/` separators, whose
    /// contents shaped the matches: the nearest existing parent of each
    /// probed source path (see `DetectionEvidence::watched_dirs`)
    pub watched_dirs: Vec<String>,
    /// Part of the tree could not be seen: a directory that would not list,
    /// or a dangling symlink named like a manifest. The matches are
    /// incomplete, and the cause (chmod, a symlink target appearing) moves
    /// no mtime, so such a result must not be persisted.
    pub unreadable: bool,
}

/// Maximum matches the walk collects before it stops. Exact-name root
/// manifests are seeded on top of this, and when the cap is hit a root-only
/// pass may add up to this many more root-level files, so a result holds at
/// most `2 * MAX_MATCHES` plus the exact manifests.
///
/// 15 leaves a wide margin over the <10 matches typical projects need, and
/// bounds memory and scan time in large monorepos.
const MAX_MATCHES: usize = 15;

/// Root names kept besides the exact-name manifests: the wildcard top-up takes
/// the first `MAX_MATCHES` of them, so a hostile root with millions of files
/// need not be held in memory
const MAX_ROOT_NAMES: usize = 256;

/// A regular file, or a symlink that resolves to one (a symlinked manifest
/// counts; a broken link resolves to nothing and does not)
fn counts_as_file(file_type: Option<std::fs::FileType>, path: &Path) -> bool {
    file_type.is_some_and(|ft| ft.is_file() || (ft.is_symlink() && path.is_file()))
}

/// How long the walk may run, scaled with the size of the scan root (roots
/// over `EXTREME_SIZE_THRESHOLD` entries are never walked)
pub(crate) fn scan_timeout(entry_count: usize) -> Duration {
    match entry_count {
        0..=50 => Duration::from_millis(100),
        51..=200 => Duration::from_millis(300),
        _ => Duration::from_millis(500),
    }
}

/// Roots with more immediate entries than this are not walked (home and
/// system directories have 500-5000+, project roots 10-400): they only get
/// the root-level manifest check.
pub(crate) const EXTREME_SIZE_THRESHOLD: usize = 500;

pub struct ScanningEngine {
    pattern_processor: PatternProcessor,
}

/// A probe only matches the shape its pattern promises: a dotted last
/// component (`lib/main.dart`) must be a file; a bare one (`src/main/java`)
/// must be a directory with at least one entry — a 1-byte file or an empty
/// scaffold directory is no source tree.
pub(crate) fn probe_shape_matches(candidate: &Path, pattern: &str) -> std::io::Result<bool> {
    let expects_file = pattern.rsplit('/').next().is_some_and(|c| c.contains('.'));
    if expects_file {
        Ok(candidate.is_file())
    } else if !candidate.is_dir() {
        Ok(false)
    } else {
        std::fs::read_dir(candidate).map(|mut entries| entries.next().is_some())
    }
}

impl ScanningEngine {
    pub fn new(pattern_processor: PatternProcessor) -> Self {
        Self { pattern_processor }
    }

    /// The total entry count (for the size guard and timeout) and the sorted
    /// names of files that a pattern could match. Every exact-name manifest
    /// is kept; of the rest only the `MAX_ROOT_NAMES` smallest.
    fn list_root(&self, listing: &DirectoryListing) -> (usize, Vec<String>, bool) {
        let exact = self.pattern_processor.get_exact_patterns();
        let mut manifests = Vec::new();
        let others = listing.entries().filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !counts_as_file(entry.file_type().ok(), &entry.path()) {
                return None;
            }
            if exact.contains(&name) {
                manifests.push(name);
                return None;
            }
            self.pattern_processor
                .should_scan_file(&name)
                .then_some(name)
        });
        let mut names = smallest(others, MAX_ROOT_NAMES);
        names.append(&mut manifests);
        names.sort();
        (listing.len(), names, !listing.unreadable())
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
    /// since the walk never leaves the root. Returns the matches, directories
    /// to watch, and whether a probe could not be read. Existence is checked
    /// prefix by prefix and shared across probes, so a project without `src/`
    /// pays a single `stat`. Sorted for a stable result.
    fn probe_paths(&self, root: &Path) -> (Vec<MatchedFile>, Vec<String>, bool) {
        let mut patterns: Vec<&String> = self
            .pattern_processor
            .get_exact_patterns()
            .iter()
            .filter(|pattern| pattern.contains('/'))
            .collect();
        patterns.sort();

        let mut exists: HashMap<&str, bool> = HashMap::new();
        let (mut matches, mut watched) = (Vec::new(), Vec::new());
        let mut unreadable = false;
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
                match probe_shape_matches(&root.join(pattern.as_str()), pattern) {
                    Ok(true) => matches.push(MatchedFile::new(pattern.clone(), pattern.clone())),
                    Ok(false) => {}
                    Err(_) => unreadable = true,
                }
            }
            if let Some(parent) = parent {
                watched.push(parent.to_string());
            }
        }
        watched.sort();
        watched.dedup();
        (matches, watched, unreadable)
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

    /// Root-level scan: a sorted, deterministic walk bounded by the match
    /// cap and an adaptive timeout. Oversized directories (likely a home or
    /// system dir) only get a root-level check. Also reports whether the
    /// timeout cut the walk short (the matches are then a partial sample)
    /// and the directories it depended on.
    pub fn scan_with_status(&self, path: &Path) -> ScanOutcome {
        self.scan_with_listing(path, &DirectoryListing::read(path))
    }

    pub(crate) fn scan_with_listing(&self, path: &Path, listing: &DirectoryListing) -> ScanOutcome {
        log::trace!("Starting scan for path: {}", path.display());
        let (entry_count, root_names, listed) = self.list_root(listing);

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
            let (probed, watched_dirs, unreadable) = self.probe_paths(path);
            matches.extend(probed);
            return ScanOutcome {
                matches,
                truncated: false,
                watched_dirs,
                unreadable: !listed || unreadable,
            };
        }

        let (probed, probe_dirs, probe_unreadable) = self.probe_paths(path);
        let mut seeded = self.root_manifest_matches(&root_names);
        seeded.extend(probed);
        let has_ignore_files = listing.entries().any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| matches!(name, ".gitignore" | ".ignore"))
        }) || std::fs::symlink_metadata(path.join(".git/info/exclude"))
            .is_ok();
        let mut outcome = if has_ignore_files {
            self.scan_sequential(path, scan_timeout(entry_count), seeded)
        } else {
            self.scan_listing(path, listing, scan_timeout(entry_count), seeded)
        };
        outcome.unreadable |= !listed || probe_unreadable;
        outcome.watched_dirs.extend(probe_dirs);
        outcome.watched_dirs.sort();
        outcome.watched_dirs.dedup();
        outcome
    }

    fn prefilter_entry(
        pattern_processor: &PatternProcessor,
        file_type: Option<std::fs::FileType>,
        path: &Path,
        file_name: &std::ffi::OsStr,
    ) -> bool {
        // Symlinks are resolved to their targets so a symlinked manifest counts as a file
        // (matching the extreme-size root check, which stats through
        // symlinks); broken links resolve to nothing and are skipped.
        if !counts_as_file(file_type, path) {
            return false;
        }

        let Some(filename) = file_name.to_str() else {
            return false;
        };

        pattern_processor.should_scan_file(filename)
    }

    fn collect_match(
        &self,
        file_type: Option<std::fs::FileType>,
        path: &Path,
        file_name: &std::ffi::OsStr,
        base: &Path,
        matched: &mut Vec<MatchedFile>,
        unreadable: &mut bool,
    ) {
        // A broken link named like a manifest is a manifest we cannot see;
        // creating its target moves no mtime we watch
        let dangling = file_type.is_some_and(|ft| ft.is_symlink()) && !path.exists();
        if dangling
            && file_name
                .to_str()
                .is_some_and(|name| self.pattern_processor.should_scan_file(name))
        {
            *unreadable = true;
            return;
        }
        if !Self::prefilter_entry(&self.pattern_processor, file_type, path, file_name) {
            return;
        }
        if let Some(file_match) = self
            .pattern_processor
            .match_file_against_patterns(path, base)
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
        timeout: Duration,
        root_manifests: Vec<MatchedFile>,
    ) -> ScanOutcome {
        let scan_start = Instant::now();
        let deadline = scan_start + timeout;
        let mut timed_out = false;
        let mut unreadable = false;
        // Seeded exact-name manifests don't count against the cap on the walk
        let mut matched_files = root_manifests;
        let root_count = matched_files.len();
        let mut capped = false;

        for result in build_walker(path).build() {
            if matched_files.len() - root_count >= MAX_MATCHES {
                log::debug!("Sequential scan: max matches ({}) reached", MAX_MATCHES);
                capped = true;
                break;
            }

            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }

            let entry = match result {
                Ok(entry) => entry,
                Err(e) => {
                    log::debug!("Scan: skipped an unreadable entry: {e}");
                    unreadable = true;
                    continue;
                }
            };
            self.collect_match(
                entry.file_type(),
                entry.path(),
                entry.file_name(),
                path,
                &mut matched_files,
                &mut unreadable,
            );
        }

        // The walk is name-sorted, so earlier-sorted files can use up the cap
        // before a root marker (`App.csproj` after many `C*.cs`) is reached.
        // Top up from a second pass, which still honours .gitignore.
        if capped {
            let before = matched_files.len();
            for entry in build_walker(path).build().flatten() {
                if matched_files.len() - before >= MAX_MATCHES {
                    break;
                }
                self.collect_match(
                    entry.file_type(),
                    entry.path(),
                    entry.file_name(),
                    path,
                    &mut matched_files,
                    &mut unreadable,
                );
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
            watched_dirs: Vec::new(),
            unreadable,
        }
    }

    fn scan_listing(
        &self,
        path: &Path,
        listing: &DirectoryListing,
        timeout: Duration,
        root_manifests: Vec<MatchedFile>,
    ) -> ScanOutcome {
        let scan_start = Instant::now();
        let deadline = scan_start + timeout;
        let mut timed_out = false;
        let mut unreadable = false;
        let mut matched_files = root_manifests;
        let root_count = matched_files.len();
        let mut capped = false;
        let mut entries: Vec<_> = listing
            .entries()
            .filter(|entry| entry.file_name() != ".git")
            .collect();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in &entries {
            if matched_files.len() - root_count >= MAX_MATCHES {
                log::debug!("Sequential scan: max matches ({}) reached", MAX_MATCHES);
                capped = true;
                break;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }
            self.collect_match(
                entry.file_type().ok(),
                &entry.path(),
                &entry.file_name(),
                path,
                &mut matched_files,
                &mut unreadable,
            );
        }

        if capped {
            let before = matched_files.len();
            for entry in &entries {
                if matched_files.len() - before >= MAX_MATCHES {
                    break;
                }
                self.collect_match(
                    entry.file_type().ok(),
                    &entry.path(),
                    &entry.file_name(),
                    path,
                    &mut matched_files,
                    &mut unreadable,
                );
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
            watched_dirs: Vec::new(),
            unreadable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        ScanningEngine::new(pattern_processor)
    }

    fn assert_same_scan(walker: &ScanOutcome, listing: &ScanOutcome) {
        assert_eq!(walker.matches, listing.matches);
        assert_eq!(walker.truncated, listing.truncated);
        assert_eq!(walker.watched_dirs, listing.watched_dirs);
        assert_eq!(walker.unreadable, listing.unreadable);
    }

    #[test]
    fn test_listing_scan_matches_walker_without_ignore_files() -> Result<()> {
        use crate::detection::directory::DirectoryListing;

        let patterns = Arc::from(vec![
            "Cargo.toml".to_string(),
            "*.csproj".to_string(),
            "*.rs".to_string(),
            "src/main/java".to_string(),
        ]);
        let engine = ScanningEngine::new(PatternProcessor::new(patterns));
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        fs::write(root.join("Cargo.toml"), "[package]")?;
        fs::write(root.join("MyApp.csproj"), "<Project/>")?;
        fs::write(root.join("main.rs"), "")?;
        fs::create_dir_all(root.join("src/main/java"))?;
        fs::write(root.join("src/main/java/Main.java"), "")?;
        fs::create_dir(root.join("nested"))?;
        fs::write(root.join("nested/ignored.rs"), "")?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("main.rs"), root.join("linked.rs"))?;
            std::os::unix::fs::symlink(root.join("nested"), root.join("linked_dir"))?;
        }

        let listing = DirectoryListing::read(root);
        let from_listing = engine.scan_with_listing(root, &listing);
        let (entry_count, root_names, listed) = engine.list_root(&listing);
        let (probed, probe_dirs, probe_unreadable) = engine.probe_paths(root);
        let mut seeded = engine.root_manifest_matches(&root_names);
        seeded.extend(probed);
        let mut from_walker = engine.scan_sequential(root, scan_timeout(entry_count), seeded);
        from_walker.unreadable |= !listed || probe_unreadable;
        from_walker.watched_dirs.extend(probe_dirs);
        from_walker.watched_dirs.sort();
        from_walker.watched_dirs.dedup();

        assert_same_scan(&from_walker, &from_listing);
        Ok(())
    }

    #[test]
    fn test_listing_scan_preserves_match_cap_and_wildcard_top_up() -> Result<()> {
        use crate::detection::directory::DirectoryListing;

        let processor =
            PatternProcessor::new(Arc::from(vec!["*.cs".to_string(), "*.csproj".to_string()]));
        let engine = ScanningEngine::new(processor);
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..(MAX_MATCHES + 5) {
            fs::write(root.join(format!("C{i:02}.cs")), "")?;
        }
        fs::write(root.join("MyApp.csproj"), "")?;

        let listing = DirectoryListing::read(root);
        let from_listing = engine.scan_with_listing(root, &listing);
        let from_walker = engine.scan_sequential(root, scan_timeout(listing.len()), Vec::new());
        let listing_paths: Vec<&str> = from_listing
            .matches
            .iter()
            .map(|m| m.relative_path.as_str())
            .collect();
        let walker_paths: Vec<&str> = from_walker
            .matches
            .iter()
            .map(|m| m.relative_path.as_str())
            .collect();

        assert_eq!(listing_paths, walker_paths);
        assert_eq!(from_listing.matches.len(), from_walker.matches.len());
        assert!(listing_paths.contains(&"MyApp.csproj"));
        Ok(())
    }

    #[test]
    fn test_local_ignore_files_use_walker_semantics() -> Result<()> {
        use crate::detection::directory::DirectoryListing;

        let engine = ScanningEngine::new(PatternProcessor::new(Arc::from(vec![
            "*.go".to_string(),
            "go.mod".to_string(),
        ])));
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        fs::write(root.join(".gitignore"), "*.go\ngo.mod\n")?;
        fs::write(root.join("main.go"), "")?;
        fs::write(root.join("go.mod"), "module example")?;

        let listing = DirectoryListing::read(root);
        let from_listing = engine.scan_with_listing(root, &listing);
        let from_walker = engine.scan_with_status(root);

        assert_same_scan(&from_walker, &from_listing);
        assert!(!from_listing
            .matches
            .iter()
            .any(|m| m.relative_path == "main.go"));
        assert!(from_listing
            .matches
            .iter()
            .any(|m| m.relative_path == "go.mod"));
        Ok(())
    }

    #[test]
    fn test_listing_scan_has_no_second_listing() -> Result<()> {
        use crate::detection::directory::DirectoryListing;

        let engine = create_test_engine();
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        let source = root.join("main.rs");
        fs::write(&source, "")?;
        let listing = DirectoryListing::read(root);
        fs::remove_file(source)?;

        let outcome = engine.scan_with_listing(root, &listing);

        assert!(outcome.matches.iter().any(|m| m.relative_path == "main.rs"));
        Ok(())
    }

    #[test]
    fn test_root_wildcard_marker_survives_match_cap() -> Result<()> {
        let processor =
            PatternProcessor::new(Arc::from(vec!["*.cs".to_string(), "*.csproj".to_string()]));
        let engine = ScanningEngine::new(processor);
        let temp_dir = TempDir::new()?;
        for i in 0..(MAX_MATCHES + 5) {
            fs::write(temp_dir.path().join(format!("C{i:02}.cs")), "")?;
        }
        fs::write(temp_dir.path().join("MyApp.csproj"), "")?;

        let matches = scan(&engine, temp_dir.path());
        assert!(
            matches.iter().any(|m| m.relative_path == "MyApp.csproj"),
            "root *.csproj must not be crowded out by earlier-sorted matches"
        );
        Ok(())
    }

    fn txt_engine() -> ScanningEngine {
        ScanningEngine::new(PatternProcessor::new(Arc::from(vec!["*.txt".to_string()])))
    }

    fn js_engine() -> ScanningEngine {
        let patterns = Arc::from(vec!["*.js".to_string(), "package.json".to_string()]);
        ScanningEngine::new(PatternProcessor::new(patterns))
    }

    #[test]
    fn test_root_manifest_seeded_and_walked_is_reported_once() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("package.json"), "{}")?;

        let names: Vec<String> = scan(&js_engine(), temp_dir.path())
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

        let names: Vec<String> = scan(&js_engine(), temp_dir.path())
            .into_iter()
            .map(|m| m.filename)
            .collect();
        assert_eq!(names, vec!["app.js".to_string()]);
        Ok(())
    }

    #[test]
    fn test_match_count_stays_bounded_when_cap_is_hit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join("package.json"), "{}")?;
        for i in 0..100 {
            fs::write(temp_dir.path().join(format!("f{i:03}.js")), "")?;
        }
        // Subdirectory files are never walked and must not count
        fs::create_dir(temp_dir.path().join("src"))?;
        for i in 0..31 {
            fs::write(temp_dir.path().join("src").join(format!("m{i:02}.js")), "")?;
        }

        let matches = scan(&js_engine(), temp_dir.path());
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
    fn test_ancestor_gitignore_does_not_hide_project_files() -> Result<()> {
        let parent = TempDir::new()?;
        fs::write(parent.path().join(".gitignore"), "*.js\n")?;
        let project = parent.path().join("proj");
        fs::create_dir_all(&project)?;
        fs::write(project.join("m.js"), "")?;

        let matches = scan(&js_engine(), &project);
        assert!(matches.iter().any(|m| m.relative_path == "m.js"));
        Ok(())
    }

    fn probe_engine() -> ScanningEngine {
        let patterns = Arc::from(vec![
            "src/main/kotlin".to_string(),
            "src/main/java".to_string(),
        ]);
        ScanningEngine::new(PatternProcessor::new(patterns))
    }

    #[test]
    fn test_source_directory_probes_are_seeded_in_sorted_order() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("src/main/kotlin"))?;
        fs::create_dir_all(temp_dir.path().join("src/main/java"))?;
        // a probed directory must hold something: empty scaffolds don't count
        fs::write(temp_dir.path().join("src/main/kotlin/Main.kt"), "")?;
        fs::write(temp_dir.path().join("src/main/java/Main.java"), "")?;

        let paths: Vec<String> = scan(&probe_engine(), temp_dir.path())
            .into_iter()
            .map(|m| m.relative_path)
            .collect();
        assert_eq!(paths, vec!["src/main/java", "src/main/kotlin"]);
        Ok(())
    }

    /// The walk contributes no watched directories — only probed source
    /// paths do (the root itself is always watched by the cache).
    #[test]
    fn test_walked_subdirectories_are_not_watched() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("a/b"))?;

        let outcome = js_engine().scan_with_status(temp_dir.path());
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

        let outcome = js_engine().scan_with_status(temp_dir.path());

        assert!(outcome.watched_dirs.is_empty());
        assert!(outcome.matches.is_empty(), "{:?}", outcome.matches);
        Ok(())
    }

    #[test]
    fn test_probe_watches_nearest_existing_ancestor() -> Result<()> {
        let temp_dir = TempDir::new()?;
        fs::create_dir_all(temp_dir.path().join("src"))?;

        // src exists, src/main does not: creating it changes src's mtime
        let outcome = probe_engine().scan_with_status(temp_dir.path());
        assert_eq!(outcome.watched_dirs, vec!["src"]);

        fs::create_dir_all(temp_dir.path().join("src/main"))?;
        let outcome = probe_engine().scan_with_status(temp_dir.path());
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

        let engine = txt_engine();
        let (count, names, listed) = engine.list_root(&DirectoryListing::read(root));
        assert_eq!(count, 3, "directories count towards the size guard");
        assert_eq!(names, vec!["a.txt".to_string(), "b.txt".to_string()]);
        assert!(listed);

        // a directory that cannot be listed says so; an empty one does not
        assert_eq!(
            engine.list_root(&DirectoryListing::read(&root.join("missing"))),
            (0, vec![], false)
        );
        assert_eq!(
            engine.list_root(&DirectoryListing::read(TempDir::new()?.path())),
            (0, vec![], true)
        );
        Ok(())
    }

    #[test]
    fn test_list_root_keeps_a_bounded_sample_of_names() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();
        for i in 0..MAX_ROOT_NAMES * 2 {
            fs::write(root.join(format!("a{i:05}.js")), "")?;
        }
        fs::write(root.join("package.json"), "{}")?;
        fs::write(root.join("notes.txt"), "")?;

        let (count, names, _) = js_engine().list_root(&DirectoryListing::read(root));

        assert_eq!(
            count,
            MAX_ROOT_NAMES * 2 + 2,
            "every entry is still counted"
        );
        assert!(names.len() <= MAX_ROOT_NAMES + 1, "{} names", names.len());
        assert!(
            names.contains(&"package.json".to_string()),
            "an exact-name manifest is never sampled away"
        );
        assert!(!names.contains(&"notes.txt".to_string()));
        assert_eq!(names[0], "a00000.js", "the sample is the smallest names");
        Ok(())
    }

    #[test]
    fn test_oversized_directory_still_detects_root_manifests() -> Result<()> {
        let patterns = Arc::from(vec!["special.config".to_string(), "*.xyz".to_string()]);
        // Detection must survive the extreme-size bailout via the root
        // exact-pattern check
        let pattern_processor = PatternProcessor::new(patterns);
        let engine = ScanningEngine::new(pattern_processor);

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
    fn test_scan_never_descends_into_subdirectories() -> Result<()> {
        let patterns = Arc::from(vec!["*.rs".to_string()]);
        let pattern_processor = PatternProcessor::new(patterns);

        let engine = ScanningEngine::new(pattern_processor);

        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        for i in 0..50 {
            let dir = root.join(format!("dir_{}", i));
            fs::create_dir(&dir)?;
            for j in 0..10 {
                let subdir = dir.join(format!("subdir_{}", j));
                fs::create_dir(&subdir)?;
                fs::write(subdir.join("lib.rs"), "// lib")?;
            }
        }

        // lib.rs files sit below the root, which the walk never leaves
        let matches = scan(&engine, root);
        assert!(matches.is_empty(), "files below the root: {matches:?}");

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
        let engine = ScanningEngine::new(PatternProcessor::new(patterns));

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
        let engine = ScanningEngine::new(PatternProcessor::new(patterns));

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
        let engine = ScanningEngine::new(PatternProcessor::new(patterns));
        let temp_dir = TempDir::new()?;
        for i in 0..20 {
            fs::write(temp_dir.path().join(format!("f{i}.rs")), "x")?;
        }

        let timed = engine.scan_sequential(temp_dir.path(), Duration::ZERO, Vec::new());
        assert!(timed.truncated, "an expired timeout must be reported");

        let ScanOutcome {
            matches,
            truncated: timed_out,
            ..
        } = engine.scan_with_status(temp_dir.path());
        assert!(!timed_out && !matches.is_empty());
        Ok(())
    }

    #[test]
    fn test_scan_timeout_scales_with_directory_size() {
        for (entries, expected_ms) in [
            (25, 100),
            (50, 100),
            (51, 300),
            (100, 300),
            (200, 300),
            (201, 500),
            (500, 500),
        ] {
            assert_eq!(
                scan_timeout(entries),
                Duration::from_millis(expected_ms),
                "entries: {entries}"
            );
        }
    }

    /// A scan that could not see part of the tree reports it, so the result
    /// is never cached: fixing the cause (chmod) changes no mtime.
    #[cfg(unix)]
    mod unreadable {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn running_as_root() -> bool {
            std::fs::read_dir("/root").is_ok()
                && std::env::var_os("USER").is_some_and(|u| u == "root")
        }

        #[test]
        fn test_an_unlistable_root_is_reported() -> Result<()> {
            if running_as_root() {
                return Ok(());
            }
            let temp_dir = TempDir::new()?;
            fs::write(temp_dir.path().join("lib.rs"), "")?;
            fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o311))?;

            let outcome = js_engine().scan_with_status(temp_dir.path());

            fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o755))?;
            assert!(outcome.unreadable);
            Ok(())
        }

        #[test]
        fn test_an_unlistable_probed_source_directory_is_reported() -> Result<()> {
            if running_as_root() {
                return Ok(());
            }
            let temp_dir = TempDir::new()?;
            let source = temp_dir.path().join("src/main/java");
            fs::create_dir_all(&source)?;
            fs::set_permissions(&source, fs::Permissions::from_mode(0o000))?;

            let outcome = probe_engine().scan_with_status(temp_dir.path());

            fs::set_permissions(&source, fs::Permissions::from_mode(0o755))?;
            assert!(outcome.unreadable);
            assert!(outcome.matches.is_empty());
            Ok(())
        }

        #[test]
        fn test_a_dangling_symlink_with_a_matching_name_is_reported() -> Result<()> {
            let temp_dir = TempDir::new()?;
            std::os::unix::fs::symlink(
                temp_dir.path().join("missing/app.js"),
                temp_dir.path().join("app.js"),
            )?;

            let outcome = js_engine().scan_with_status(temp_dir.path());

            assert!(outcome.unreadable);
            assert!(outcome.matches.is_empty());
            Ok(())
        }

        #[test]
        fn test_a_dangling_symlink_with_an_unrelated_name_is_not() -> Result<()> {
            let temp_dir = TempDir::new()?;
            std::os::unix::fs::symlink(
                temp_dir.path().join("missing/x"),
                temp_dir.path().join("notes.txt"),
            )?;

            assert!(!js_engine().scan_with_status(temp_dir.path()).unreadable);
            Ok(())
        }

        #[test]
        fn test_a_fully_readable_tree_is_not() -> Result<()> {
            let temp_dir = TempDir::new()?;
            fs::write(temp_dir.path().join("app.js"), "")?;

            assert!(!js_engine().scan_with_status(temp_dir.path()).unreadable);
            Ok(())
        }
    }
}
