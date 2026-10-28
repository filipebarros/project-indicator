//! Persistent detection cache.
//!
//! Caches one detection result per project directory across CLI invocations,
//! stored as JSON files under `<cache_base>/project-indicator/results/`.
//!
//! Invalidation is evidence-keyed (see CLAUDE.md for the full list): an entry
//! is served only when every recorded mtime is unchanged, the binary build
//! matches, and the stored directory path matches (which also makes
//! filename-hash collisions degrade to a miss, never a wrong result).
//!
//! Every failure path degrades silently to a cache miss: the cache must
//! never be able to break a shell prompt.

use crate::types::{DetectionConfig, DetectionEvidence, DetectionResult, DisplayConfig};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Opportunistic GC kicks in when the results directory exceeds this count.
const MAX_ENTRIES_BEFORE_GC: usize = 512;

/// A hit refreshes the entry's mtime only once it is older than this (well
/// under the GC horizon): a write on every hit is the priciest user-space
/// cost of the warm path.
const TOUCH_AFTER_SECS: u64 = 24 * 60 * 60;

/// File timestamps can come from a coarser clock than `SystemTime::now()`
/// (a few ms behind on Linux), so a write right after detection began may be
/// stamped at or before it. Anything modified this close to the start counts
/// as modified during detection.
const MTIME_SLACK: Duration = Duration::from_millis(20);

/// How recent a whole-second timestamp must be to be distrusted: twice the
/// coarsest granularity in use (FAT/exFAT keep 2 s)
const COARSE_CLOCK_SLACK: Duration = Duration::from_secs(4);

/// The `started` argument for [`PersistentCache::store`]: call it before
/// detection begins.
pub fn detection_started() -> SystemTime {
    SystemTime::now()
        .checked_sub(MTIME_SLACK)
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// A temp file older than this was left behind by a crashed writer.
const TMP_MAX_AGE_SECS: u64 = 10 * 60;

/// GC removes entries whose file mtime is older than this.
const GC_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct CacheEntry<T> {
    dir: PathBuf,
    binary_version: String,
    /// The user's home directory (root discovery's boundary) when this was
    /// computed: running as another user or with another `$HOME` can change
    /// the result for the same directory
    home: Option<PathBuf>,
    config: Option<(PathBuf, Stamp)>,
    dir_stamp: Stamp,
    /// Ancestor directories root discovery walked through, with mtimes: a
    /// new manifest in any of them can change the result
    ancestors: Vec<(PathBuf, Stamp)>,
    evidence: Vec<(PathBuf, Stamp)>,
    result: T,
    display: DisplayConfig,
}

pub struct PersistentCache {
    base: PathBuf,
}

impl PersistentCache {
    pub fn at_base(base: PathBuf) -> Self {
        Self { base }
    }

    /// `$XDG_CACHE_HOME` if set, else `~/.cache`.
    pub fn default_location() -> Option<Self> {
        let cache_home = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))?;

        Some(Self::at_base(
            cache_home.join("project-indicator").join("results"),
        ))
    }

    pub fn load(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
    ) -> Option<(DetectionResult, DisplayConfig)> {
        let dir = canonical(dir);
        let content = fs::read_to_string(self.entry_path(&dir)).ok()?;
        let entry: CacheEntry<DetectionResult> = serde_json::from_str(&content).ok()?;

        if entry.dir != dir || entry.binary_version != build_id() || entry.home != current_home() {
            return None;
        }

        let current_config = config_path.map(|p| (p.to_path_buf(), stamp(p)));
        let stored_config = entry.config.as_ref().map(|(p, t)| (p.clone(), Some(*t)));
        if current_config != stored_config {
            return None;
        }

        if stamp(&dir)? != entry.dir_stamp {
            return None;
        }

        for (path, stored) in entry.ancestors.iter().chain(&entry.evidence) {
            if stamp(path)? != *stored {
                return None;
            }
        }

        // Keep actively used entries fresh for the age-based GC; best-effort,
        // a hit must never fail
        let entry_path = self.entry_path(&dir);
        let stale =
            mtime(&entry_path).is_some_and(|t| age_secs(SystemTime::now(), t) > TOUCH_AFTER_SECS);
        if stale {
            let _ = fs::File::options()
                .write(true)
                .open(&entry_path)
                .and_then(|f| f.set_modified(SystemTime::now()));
        }

        Some((entry.result, entry.display))
    }

    /// Store a detection result for `dir`. `started` is when detection began:
    /// anything it depended on that changed after that may not be reflected
    /// in the result, so such results are not stored. Failures are logged at
    /// debug level and otherwise ignored.
    pub fn store(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
        result: &DetectionResult,
        display: &DisplayConfig,
        detection: &DetectionConfig,
        started: SystemTime,
    ) {
        match self.try_store(dir, config_path, result, display, detection, started) {
            Ok(true) => self.gc(),
            Ok(false) => {}
            Err(e) => log::debug!("persistent cache write skipped: {}", e),
        }
    }

    fn try_store(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
        result: &DetectionResult,
        display: &DisplayConfig,
        detection: &DetectionConfig,
        started: SystemTime,
    ) -> crate::Result<bool> {
        let dir = canonical(dir);

        // A timed-out scan or an unreadable file or directory depends on more
        // than we can watch, and listing too many directories or files would
        // cost more to check on every hit than the scan the cache saves
        if !result.evidence.is_cacheable()
            || (!result.evidence.repository
                && (result.evidence.watched_dirs.len() > DetectionEvidence::MAX_WATCHED_DIRS
                    || result.evidence.read_files.len() > DetectionEvidence::MAX_READ_FILES))
        {
            return Ok(false);
        }

        // A config the caller names but we cannot stat: don't cache
        let config = match config_path {
            Some(p) => {
                let Some(t) = stamp(p) else {
                    return Ok(false);
                };
                Some((p.to_path_buf(), t))
            }
            None => None,
        };
        let Some(dir_stamp) = stamp(&dir) else {
            return Ok(false);
        };

        let entry = CacheEntry {
            dir: dir.clone(),
            binary_version: build_id().to_string(),
            home: current_home(),
            config,
            dir_stamp,
            ancestors: stamped(ancestor_chain(
                &dir,
                result,
                crate::detection::engine::MAX_UPWARD_TRAVERSAL,
                detection.require_vcs_root,
            )),
            evidence: stamped(collect_evidence(&dir, result)),
            result,
            display: display.clone(),
        };

        let stamps = entry
            .config
            .iter()
            .map(|(_, t)| t)
            .chain(entry.ancestors.iter().map(|(_, t)| t))
            .chain(entry.evidence.iter().map(|(_, t)| t))
            .chain(std::iter::once(&entry.dir_stamp));
        if stamps.into_iter().any(|t| t.is_unsettled(started)) {
            return Ok(false);
        }

        fs::create_dir_all(&self.base)?;
        let final_path = self.entry_path(&dir);
        // Unique per writer (process + in-process counter) so concurrent
        // stores of the same directory can't interleave one temp file
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp_path = final_path.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let write = write_private(&tmp_path, &serde_json::to_vec(&entry)?)
            .and_then(|()| fs::rename(&tmp_path, &final_path));
        if write.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        write?;
        Ok(true)
    }

    pub fn clear(&self) -> crate::Result<()> {
        if self.base.exists() {
            fs::remove_dir_all(&self.base)?;
        }
        Ok(())
    }

    /// Returns (entry count, total size in bytes).
    pub fn stats(&self) -> crate::Result<(usize, u64)> {
        let mut count = 0;
        let mut bytes = 0;
        if self.base.exists() {
            for entry in fs::read_dir(&self.base)? {
                let entry = entry?;
                if entry.path().extension().is_some_and(|e| e == "json") {
                    count += 1;
                    bytes += entry.metadata().map_or(0, |m| m.len());
                }
            }
        }
        Ok((count, bytes))
    }

    fn entry_path(&self, dir: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        dir.hash(&mut hasher);
        self.base.join(format!("{:016x}.json", hasher.finish()))
    }

    /// Remove abandoned temp files, and (once the directory has grown large)
    /// entries older than the GC age or beyond the cap. Best-effort: all
    /// errors ignored. Below the cap only temp files are stat-ed.
    fn gc(&self) {
        let Ok(dir_entries) = fs::read_dir(&self.base) else {
            return;
        };
        let now = SystemTime::now();
        let mut entries = Vec::new();
        for path in dir_entries.filter_map(|e| e.ok().map(|e| e.path())) {
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => entries.push(path),
                // A writer's temp file lives for milliseconds: one that has
                // been around for minutes belongs to a crashed process
                Some("tmp")
                    if mtime(&path).is_some_and(|t| age_secs(now, t) > TMP_MAX_AGE_SECS) =>
                {
                    let _ = fs::remove_file(&path);
                }
                _ => {}
            }
        }
        if entries.len() <= MAX_ENTRIES_BEFORE_GC {
            return;
        }

        let mut entries: Vec<(PathBuf, SystemTime)> = entries
            .into_iter()
            .filter_map(|p| mtime(&p).map(|t| (p, t)))
            .collect();
        entries.retain(|(path, t)| {
            let expired = age_secs(now, *t) > GC_MAX_AGE_SECS;
            if expired {
                let _ = fs::remove_file(path);
            }
            !expired
        });

        // Age alone frees nothing when the cap is reached by entries used
        // within the horizon (hits refresh mtimes): evict oldest beyond cap
        if entries.len() > MAX_ENTRIES_BEFORE_GC {
            entries.sort_by_key(|(_, t)| *t);
            for (path, _) in &entries[..entries.len() - MAX_ENTRIES_BEFORE_GC] {
                let _ = fs::remove_file(path);
            }
        }
    }
}

/// The canonical home directory root discovery treats as a boundary
fn current_home() -> Option<PathBuf> {
    std::env::home_dir().map(|home| canonical(&home))
}

/// Seconds from `t` to `now`; a future timestamp ages 0.
fn age_secs(now: SystemTime, t: SystemTime) -> u64 {
    now.duration_since(t).map_or(0, |age| age.as_secs())
}

/// Identifies the running binary: the package version plus the executable's
/// size and mtime, so rebuilding from source without a version bump doesn't
/// serve entries written by the previous build. Computed once per process.
fn build_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        let exe = std::env::current_exe()
            .ok()
            .and_then(|path| fs::metadata(path).ok());
        match exe {
            Some(meta) => {
                let nanos = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                format!("{version}+{}.{nanos}", meta.len())
            }
            None => version.to_string(),
        }
    })
}

fn canonical(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Write `bytes` to a new file only the owner can read: entries hold
/// absolute paths of the user's projects
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

/// What a watched path looked like. `changed` is the inode change time on
/// unix: it moves on chmod, chown and renames and cannot be set back, which
/// mtime can (`cp -p`, `rsync -t`, editors that preserve it). Elsewhere it is
/// the mtime again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    mtime: SystemTime,
    changed: SystemTime,
}

impl Stamp {
    /// Changed after `started`, or recent enough that a further change could
    /// still carry the same stamp (a coarse clock: whole-second times)
    fn is_unsettled(&self, started: SystemTime) -> bool {
        [self.mtime, self.changed].into_iter().any(|t| {
            let whole_second = t
                .duration_since(SystemTime::UNIX_EPOCH)
                .is_ok_and(|d| d.subsec_nanos() == 0);
            t > started || (whole_second && t + COARSE_CLOCK_SLACK > started)
        })
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        let since_epoch = Duration::new(
            u64::try_from(meta.ctime()).unwrap_or(0),
            u32::try_from(meta.ctime_nsec()).unwrap_or(0),
        );
        SystemTime::UNIX_EPOCH + since_epoch
    };
    #[cfg(not(unix))]
    let changed = mtime;
    Some(Stamp { mtime, changed })
}

/// Each path with its stamp; paths that cannot be stat-ed are dropped
fn stamped(paths: impl IntoIterator<Item = PathBuf>) -> Vec<(PathBuf, Stamp)> {
    paths
        .into_iter()
        .filter_map(|p| stamp(&p).map(|t| (p, t)))
        .collect()
}

/// Directories above `dir` that root discovery consulted: up to and including
/// the discovered root (the parent of the absolute root-discovery item), or
/// the `max_upward` ancestors discovery was allowed to visit when none was
/// found. A config change misses via the config mtime, so the limit is fixed
/// for the life of an entry.
fn ancestor_chain(
    dir: &Path,
    result: &DetectionResult,
    max_upward: usize,
    require_vcs_root: bool,
) -> Vec<PathBuf> {
    if result.evidence.repository {
        return Vec::new();
    }

    // Canonical like `dir` (/var vs /private/var on macOS), or the equality
    // checks below would never fire
    let root = result.evidence.scan_root.as_deref().map(canonical);
    let root = root.as_deref();

    // Root found in `dir` itself: nothing above it was consulted, unless
    // require_vcs_root sent discovery looking upward for a VCS marker
    if root == Some(dir) && !require_vcs_root {
        return Vec::new();
    }

    // Root discovery never enters boundary directories (home, filesystem
    // root), so their churn must not invalidate entries
    // Canonical like `dir`: a symlinked $HOME would never compare equal
    let home = current_home();
    let mut chain = Vec::new();
    for ancestor in dir.ancestors().skip(1) {
        if ancestor.parent().is_none() || home.as_deref() == Some(ancestor) {
            break;
        }
        chain.push(ancestor.to_path_buf());
        // Past the root only when a VCS marker is required: it may sit above
        if (root == Some(ancestor) && !require_vcs_root) || chain.len() >= max_upward {
            break;
        }
    }
    chain
}

/// The `exclude` file that shapes `dir`'s walk, and the directory it appears
/// in. Not `.git` itself (every git command touches its mtime), unless there
/// is no `info/` directory to watch instead.
fn watch_git_info(paths: &mut BTreeSet<PathBuf>, dir: &Path) {
    let git_info = dir.join(".git").join("info");
    if !git_info.exists() {
        paths.insert(dir.join(".git"));
    }
    paths.insert(git_info.join("exclude"));
    paths.insert(git_info);
}

/// Absolute paths of every file that influenced the result: language,
/// framework, and root-discovery evidence items plus per-framework evidence
/// file names. Relative paths are resolved against the project directory.
fn collect_evidence(dir: &Path, result: &DetectionResult) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();

    // No root discovery → the scan ran in `dir` itself
    let base = result.evidence.scan_root.as_deref().unwrap_or(dir);

    // Ignore files shape the walk, and editing one in place leaves every
    // directory mtime untouched (missing ones are dropped by the caller)
    let ignore_files = [".gitignore", ".ignore"];
    for name in ignore_files {
        paths.insert(base.join(name));
    }
    watch_git_info(&mut paths, base);
    for watched in &result.evidence.watched_dirs {
        let dir = base.join(watched);
        for name in ignore_files {
            paths.insert(dir.join(name));
        }
        // Absolute entries are the directories of nested projects, each
        // walked with its own `.git/info/exclude`
        if Path::new(watched).is_absolute() {
            watch_git_info(&mut paths, &dir);
        }
        paths.insert(dir);
    }

    paths.extend(
        result
            .evidence
            .read_files
            .iter()
            .map(|file| base.join(file)),
    );

    let items = result
        .evidence
        .indicator_evidence
        .iter()
        .chain(&result.evidence.framework_evidence)
        .chain(&result.evidence.root_discovery)
        .map(|item| item.file_path.as_str());

    for raw in items {
        // joining an absolute path replaces the base
        paths.insert(base.join(raw));
    }

    paths
}

#[cfg(test)]
mod tests {
    fn max_upward() -> usize {
        crate::detection::engine::MAX_UPWARD_TRAVERSAL
    }

    use super::*;
    use crate::types::Indicator;
    use std::sync::Arc;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    fn test_result(evidence_file: &str) -> DetectionResult {
        let lang = Indicator::new(
            "Rust".to_string(),
            vec!["Cargo.toml".to_string()],
            "#dea584".to_string(),
            "R".to_string(),
            vec![],
        );
        let mut result = DetectionResult::new(Some(Arc::new(lang)), vec![], 0.9);
        result
            .evidence
            .indicator_evidence
            .push(crate::types::EvidenceItem::manifest_file(
                evidence_file.to_string(),
                evidence_file.to_string(),
                0.8,
            ));
        result
    }

    /// A realistic result: root discovery found the project at `dir` itself
    fn rooted(mut result: DetectionResult, dir: &Path) -> DetectionResult {
        result.evidence.scan_root = Some(canonical(dir));
        result
            .evidence
            .root_discovery
            .push(crate::types::EvidenceItem::root_indicator(
                canonical(dir)
                    .join("Cargo.toml")
                    .to_string_lossy()
                    .to_string(),
                "Cargo.toml".to_string(),
                0.95,
            ));
        result
    }

    fn setup() -> Result<(TempDir, TempDir, PersistentCache), Box<dyn std::error::Error>> {
        let cache_dir = TempDir::new()?;
        let project = TempDir::new()?;
        fs::write(project.path().join("Cargo.toml"), "[package]")?;
        let cache = PersistentCache::at_base(cache_dir.path().join("results"));
        Ok((cache_dir, project, cache))
    }

    /// Store `result` as a run that has just finished would
    fn store_now(
        cache: &PersistentCache,
        dir: &Path,
        config_path: Option<&Path>,
        result: &DetectionResult,
    ) {
        cache.store(
            dir,
            config_path,
            result,
            &DisplayConfig::default(),
            &DetectionConfig::default(),
            SystemTime::now(),
        );
    }

    fn set_file_mtime(path: &Path, t: SystemTime) -> Result<(), Box<dyn std::error::Error>> {
        fs::File::options()
            .write(true)
            .open(path)?
            .set_modified(t)?;
        Ok(())
    }

    fn set_dir_mtime(path: &Path, t: SystemTime) -> Result<(), Box<dyn std::error::Error>> {
        let mut options = fs::File::options();
        // Unix cannot open a directory for writing; Windows needs write access
        // plus FILE_FLAG_BACKUP_SEMANTICS to open one at all
        #[cfg(windows)]
        std::os::windows::fs::OpenOptionsExt::custom_flags(options.write(true), 0x0200_0000);
        #[cfg(not(windows))]
        options.read(true);
        options.open(path)?.set_modified(t)?;
        Ok(())
    }

    fn write_tmp(
        base: &Path,
        name: &str,
        age: Duration,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        fs::create_dir_all(base)?;
        let path = base.join(name);
        fs::write(&path, "partial")?;
        set_file_mtime(&path, SystemTime::now() - age)?;
        Ok(path)
    }

    #[test]
    fn test_gc_removes_abandoned_temp_files_but_keeps_in_flight_ones(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);
        let abandoned = write_tmp(&cache.base, "a.json.1.1.tmp", Duration::from_secs(3600))?;
        let in_flight = write_tmp(&cache.base, "b.json.2.1.tmp", Duration::from_secs(1))?;

        cache.gc();

        assert!(!abandoned.exists(), "a crashed writer's temp file lingers");
        assert!(
            in_flight.exists(),
            "another process's live temp file removed"
        );
        Ok(())
    }

    #[test]
    fn test_temp_files_do_not_count_toward_the_entry_cap() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_c, _project, cache) = setup()?;
        let temps: Vec<PathBuf> = (0..MAX_ENTRIES_BEFORE_GC + 5)
            .map(|i| {
                write_tmp(
                    &cache.base,
                    &format!("{i}.json.9.1.tmp"),
                    Duration::from_secs(1),
                )
            })
            .collect::<Result<_, _>>()?;

        cache.gc();

        assert!(
            temps.iter().all(|t| t.exists()),
            "GC evicted live temp files"
        );
        Ok(())
    }

    #[test]
    fn test_gc_evicts_oldest_beyond_cap() -> Result<(), Box<dyn std::error::Error>> {
        let (cache_dir, project, cache) = setup()?;
        let base = cache_dir.path().join("results");
        fs::create_dir_all(&base)?;

        // Fresh entries beyond the cap: age-based GC alone frees nothing
        let old = SystemTime::now() - Duration::from_secs(3600);
        for i in 0..MAX_ENTRIES_BEFORE_GC + 10 {
            let p = base.join(format!("junk{i}.json"));
            fs::write(&p, "x")?;
            if i < 10 {
                set_file_mtime(&p, old)?;
            }
        }

        store_now(
            &cache,
            project.path(),
            None,
            &rooted(test_result("Cargo.toml"), project.path()),
        );

        let count = fs::read_dir(&base)?.count();
        assert!(
            count <= MAX_ENTRIES_BEFORE_GC,
            "cache must not grow beyond the cap, got {count} entries"
        );
        assert!(
            !base.join("junk0.json").exists(),
            "the oldest entries must be evicted first"
        );
        assert!(
            cache.load(project.path(), None).is_some(),
            "the just-stored entry must survive eviction"
        );
        Ok(())
    }

    /// Entries unused for longer than the GC horizon go even when the cap
    /// alone would not have evicted them (expiry removes MORE than the
    /// overflow here, so cap eviction cannot be what passes this test).
    #[test]
    fn test_gc_expires_entries_older_than_the_horizon() -> Result<(), Box<dyn std::error::Error>> {
        let (cache_dir, project, cache) = setup()?;
        let base = cache_dir.path().join("results");
        fs::create_dir_all(&base)?;

        let expired = SystemTime::now() - Duration::from_secs(GC_MAX_AGE_SECS + 3600);
        for i in 0..50 {
            let p = base.join(format!("expired{i}.json"));
            fs::write(&p, "x")?;
            set_file_mtime(&p, expired)?;
        }
        let fresh = MAX_ENTRIES_BEFORE_GC - 10;
        for i in 0..fresh {
            fs::write(base.join(format!("fresh{i}.json")), "x")?;
        }

        store_now(
            &cache,
            project.path(),
            None,
            &rooted(test_result("Cargo.toml"), project.path()),
        );

        let name_starts = |prefix: &str| -> usize {
            fs::read_dir(&base).map_or(0, |entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
                    .count()
            })
        };
        assert_eq!(name_starts("expired"), 0, "every expired entry must go");
        assert_eq!(name_starts("fresh"), fresh, "fresh entries must stay");
        Ok(())
    }

    #[test]
    fn test_hit_refreshes_entry_mtime() -> Result<(), Box<dyn std::error::Error>> {
        let (cache_dir, project, cache) = setup()?;
        store_now(
            &cache,
            project.path(),
            None,
            &rooted(test_result("Cargo.toml"), project.path()),
        );

        // Age the entry past the GC horizon, then hit it: a daily-used
        // project's entry must not be evicted by the age sweep
        let base = cache_dir.path().join("results");
        let entry = fs::read_dir(&base)?
            .filter_map(|e| e.ok())
            .next()
            .ok_or("entry file missing")?
            .path();
        set_file_mtime(
            &entry,
            SystemTime::now() - Duration::from_secs(GC_MAX_AGE_SECS + 60),
        )?;

        assert!(cache.load(project.path(), None).is_some());
        let age = SystemTime::now()
            .duration_since(mtime(&entry).ok_or("no mtime")?)?
            .as_secs();
        assert!(
            age < 3600,
            "a cache hit must refresh the entry mtime, age is {age}s"
        );
        Ok(())
    }

    #[test]
    fn test_store_then_load_hits() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);

        let (loaded, display) = cache
            .load(project.path(), None)
            .ok_or("expected cache hit")?;
        assert_eq!(loaded, result);
        assert_eq!(display, DisplayConfig::default());
        Ok(())
    }

    #[test]
    fn test_config_mtime_change_invalidates() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let config_dir = TempDir::new()?;
        let config_path = config_dir.path().join("config.toml");
        fs::write(&config_path, "[meta]\nversion = \"2.0\"")?;

        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), Some(&config_path), &result);
        assert!(cache.load(project.path(), Some(&config_path)).is_some());

        sleep(Duration::from_millis(20));
        fs::write(&config_path, "[meta]\nversion = \"2.1\"")?;

        assert!(cache.load(project.path(), Some(&config_path)).is_none());
        Ok(())
    }

    #[test]
    fn test_config_appearing_or_disappearing_invalidates() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_c, project, cache) = setup()?;
        let config_dir = TempDir::new()?;
        let config_path = config_dir.path().join("config.toml");
        fs::write(&config_path, "x")?;

        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);
        assert!(cache.load(project.path(), Some(&config_path)).is_none());

        store_now(&cache, project.path(), Some(&config_path), &result);
        assert!(cache.load(project.path(), Some(&config_path)).is_some());
        fs::remove_file(&config_path)?;
        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_corrupt_entry_is_a_miss() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);

        let entry_path = cache.entry_path(&canonical(project.path()));
        fs::write(&entry_path, "not json at all {{{")?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_version_mismatch_is_a_miss() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);

        let entry_path = cache.entry_path(&canonical(project.path()));
        let content = fs::read_to_string(&entry_path)?;
        let stale = content.replace(env!("CARGO_PKG_VERSION"), "0.0.0-other");
        fs::write(&entry_path, stale)?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_clear_and_stats() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);

        let (count, bytes) = cache.stats()?;
        assert_eq!(count, 1);
        assert!(bytes > 0);

        cache.clear()?;
        let (count, _) = cache.stats()?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[test]
    fn test_new_manifest_in_ancestor_invalidates_no_result(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let sub = project.path().join("a").join("b");
        fs::create_dir_all(&sub)?;
        // Cached against `sub`; evidence file doesn't matter here
        store_now(&cache, &sub, None, &test_result("Cargo.toml"));
        // (No is_some() precondition: with no root found the chain reaches the
        // shared system temp dir, whose mtime parallel tests churn.)

        // A change in an ANCESTOR (root discovery walks up) must invalidate
        std::thread::sleep(Duration::from_millis(20));
        fs::write(project.path().join("a").join("package.json"), "{}")?;
        assert!(
            cache.load(&sub, None).is_none(),
            "ancestor change must miss"
        );
        Ok(())
    }

    #[test]
    fn test_ancestor_chain_stops_at_discovered_root() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let root = project.path().join("root");
        let sub = root.join("x").join("y");
        fs::create_dir_all(&sub)?;
        fs::write(root.join("Cargo.toml"), "[package]")?;
        let mut result = test_result("Cargo.toml");
        result.evidence.scan_root = Some(canonical(&root));
        result
            .evidence
            .root_discovery
            .push(crate::types::EvidenceItem::root_indicator(
                canonical(&root)
                    .join("Cargo.toml")
                    .to_string_lossy()
                    .to_string(),
                "Cargo.toml".to_string(),
                0.95,
            ));
        store_now(&cache, &sub, None, &result);

        // Above the root: irrelevant to the result, must NOT invalidate
        std::thread::sleep(Duration::from_millis(20));
        fs::write(project.path().join("unrelated.txt"), "x")?;
        assert!(cache.load(&sub, None).is_some());

        // At the root (a new go.mod could change the winner): must invalidate
        fs::write(root.join("go.mod"), "module x")?;
        assert!(cache.load(&sub, None).is_none());
        Ok(())
    }

    /// With `require_vcs_root` the VCS marker may sit ABOVE the discovered
    /// root (and root discovery only accepted the root because of it), so
    /// the chain must keep going past the root to the traversal bound.
    #[test]
    fn test_ancestor_chain_passes_the_root_when_a_vcs_marker_is_required() {
        let sub = Path::new("/a/b/root/x/y");
        let mut result = test_result("Cargo.toml");
        result.evidence.scan_root = Some(PathBuf::from("/a/b/root"));
        result
            .evidence
            .root_discovery
            .push(crate::types::EvidenceItem::root_indicator(
                "/a/b/root/Cargo.toml".to_string(),
                "Cargo.toml".to_string(),
                0.95,
            ));

        let without = ancestor_chain(sub, &result, max_upward(), false);
        let with = ancestor_chain(sub, &result, max_upward(), true);

        assert_eq!(
            without.last().map(|p| p.as_path()),
            Some(Path::new("/a/b/root"))
        );
        assert!(
            with.contains(&PathBuf::from("/a/b")) && with.contains(&PathBuf::from("/a")),
            "directories above the root hold the VCS marker: {with:?}"
        );
    }

    #[test]
    fn test_repository_result_has_no_ancestor_watches() {
        let mut result = test_result("Cargo.toml");
        result.evidence.scan_root = Some(PathBuf::from("/a/b/root"));
        result.evidence.repository = true;

        let chain = ancestor_chain(Path::new("/a/b/root/x/y"), &result, max_upward(), true);

        assert!(chain.is_empty());
    }

    #[test]
    fn test_old_evidence_json_defaults_to_non_repository() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut json = serde_json::to_value(DetectionEvidence::default())?;
        json.as_object_mut()
            .ok_or("evidence must be an object")?
            .remove("repository");
        let evidence: DetectionEvidence = serde_json::from_value(json)?;
        assert!(!evidence.repository);
        Ok(())
    }

    #[test]
    fn test_ancestor_chain_is_capped_without_a_root() {
        let deep = Path::new("/a/b/c/d/e/f/g/h/i/j/k/l/m/n");
        let result = test_result("Cargo.toml");
        let chain = ancestor_chain(deep, &result, max_upward(), false);
        assert_eq!(chain.len(), max_upward());
        assert_eq!(
            chain.first().map(|p| p.as_path()),
            Some(Path::new("/a/b/c/d/e/f/g/h/i/j/k/l/m"))
        );
        // never includes the filesystem root
        assert!(!chain.iter().any(|p| p.parent().is_none()));
    }

    #[test]
    fn test_entry_written_by_another_build_of_the_same_version_is_a_miss(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);
        assert!(cache.load(project.path(), None).is_some());

        // What a build without a binary id (or another binary) would have
        // written: the bare package version
        let entry_path = cache.entry_path(&canonical(project.path()));
        let mut entry: serde_json::Value = serde_json::from_str(&fs::read_to_string(&entry_path)?)?;
        entry["binary_version"] = env!("CARGO_PKG_VERSION").into();
        fs::write(&entry_path, serde_json::to_vec(&entry)?)?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_entry_recorded_for_another_directory_is_a_miss(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // A filename-hash collision must degrade to a miss, never a wrong result
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        store_now(&cache, project.path(), None, &result);
        let entry_path = cache.entry_path(&canonical(project.path()));
        let mut entry: serde_json::Value = serde_json::from_str(&fs::read_to_string(&entry_path)?)?;
        entry["dir"] = "/some/other/project".into();
        fs::write(&entry_path, serde_json::to_vec(&entry)?)?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_file_modified_within_the_clock_slack_of_the_start_is_not_stored(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Coarse filesystem clocks can stamp a write just after detection
        // began with a time at or before `started`
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        fs::write(project.path().join("Cargo.toml"), "[package]\nname = \"y\"")?;

        cache.store(
            project.path(),
            None,
            &result,
            &DisplayConfig::default(),
            &DetectionConfig::default(),
            detection_started(),
        );
        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_file_modified_long_before_the_start_is_stored() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        let long_ago = SystemTime::now() - MTIME_SLACK * 10;
        set_file_mtime(&project.path().join("Cargo.toml"), long_ago)?;
        set_dir_mtime(project.path(), long_ago)?;
        // ctime cannot be set back: let real time pass so the files are old
        // in every sense, as a file untouched for a while is
        sleep(MTIME_SLACK * 3);

        cache.store(
            project.path(),
            None,
            &result,
            &DisplayConfig::default(),
            &DetectionConfig::default(),
            detection_started(),
        );
        assert!(cache.load(project.path(), None).is_some());
        Ok(())
    }

    /// A whole-second timestamp gives the filesystem away as coarse (FAT and
    /// exFAT keep 2 s, ext3, HFS+ and some NFS 1 s): an edit in the same
    /// second would keep the stamp unchanged, so a recent one is not trusted
    fn store_with_whole_second_mtimes(age: Duration) -> Result<bool, Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let whole = SystemTime::UNIX_EPOCH + Duration::from_secs(now.as_secs()) - age;
        set_file_mtime(&project.path().join("Cargo.toml"), whole)?;
        set_dir_mtime(project.path(), whole)?;
        // ctime cannot be set back: let real time pass so it is old too
        sleep(MTIME_SLACK * 3);

        cache.store(
            project.path(),
            None,
            &result,
            &DisplayConfig::default(),
            &DetectionConfig::default(),
            detection_started(),
        );
        Ok(cache.load(project.path(), None).is_some())
    }

    #[test]
    fn test_a_recent_whole_second_timestamp_is_not_stored() -> Result<(), Box<dyn std::error::Error>>
    {
        assert!(!store_with_whole_second_mtimes(Duration::from_secs(1))?);
        Ok(())
    }

    #[test]
    fn test_an_old_whole_second_timestamp_is_stored() -> Result<(), Box<dyn std::error::Error>> {
        assert!(store_with_whole_second_mtimes(Duration::from_secs(10))?);
        Ok(())
    }

    /// `mutate` makes a result depend on something the cache cannot watch
    fn assert_not_stored(
        mutate: impl FnOnce(&mut DetectionEvidence),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let mut result = rooted(test_result("Cargo.toml"), project.path());
        mutate(&mut result.evidence);
        store_now(&cache, project.path(), None, &result);
        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_a_result_that_read_too_many_files_is_not_stored(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_not_stored(|evidence| {
            evidence.read_files = (0..=DetectionEvidence::MAX_READ_FILES)
                .map(|i| format!("p{i}.csproj"))
                .collect();
        })
    }

    #[test]
    fn test_a_truncated_scan_is_not_stored() -> Result<(), Box<dyn std::error::Error>> {
        assert_not_stored(|evidence| evidence.scan_truncated = true)
    }

    #[test]
    fn test_a_result_computed_without_an_unreadable_file_is_not_stored(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_not_stored(|evidence| evidence.read_failed = true)
    }

    #[test]
    fn test_scan_with_too_many_watched_dirs_is_not_stored() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_not_stored(|evidence| {
            evidence.watched_dirs = (0..=DetectionEvidence::MAX_WATCHED_DIRS)
                .map(|i| format!("d{i}"))
                .collect();
        })
    }

    /// Entries hold absolute project paths: keep them to the owner
    #[cfg(unix)]
    #[test]
    fn test_entries_are_private_to_the_owner() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let (cache_dir, project, cache) = setup()?;
        store_now(
            &cache,
            project.path(),
            None,
            &rooted(test_result("Cargo.toml"), project.path()),
        );

        let entry = fs::read_dir(cache_dir.path().join("results"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .ok_or("no entry written")?;

        let mode = fs::metadata(&entry)?.permissions().mode();
        assert_eq!(mode & 0o077, 0, "group/other can read the entry: {mode:o}");
        Ok(())
    }

    /// A chmod during detection moves ctime but not mtime: the result may have
    /// been computed from the file's previous permissions
    #[cfg(unix)]
    #[test]
    fn test_a_permission_change_during_detection_is_not_stored(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let (_c, project, cache) = setup()?;
        let result = rooted(test_result("Cargo.toml"), project.path());
        let manifest = project.path().join("Cargo.toml");
        set_file_mtime(&manifest, SystemTime::now() - Duration::from_secs(3600))?;
        set_dir_mtime(
            project.path(),
            SystemTime::now() - Duration::from_secs(3600),
        )?;
        sleep(MTIME_SLACK * 3); // let the ctime of the set-up settle in the past
        let started = detection_started();

        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))?;

        cache.store(
            project.path(),
            None,
            &result,
            &DisplayConfig::default(),
            &DetectionConfig::default(),
            started,
        );
        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_ancestor_chain_cap_follows_configured_traversal() {
        let deep = Path::new("/a/b/c/d/e/f/g/h/i/j/k/l/m/n");
        let result = test_result("Cargo.toml");
        assert_eq!(ancestor_chain(deep, &result, 3, false).len(), 3);
        assert_eq!(ancestor_chain(deep, &result, 12, false).len(), 12);
    }

    #[test]
    fn test_ancestor_chain_stops_before_home() {
        if let Some(home) = std::env::home_dir().map(|h| canonical(&h)) {
            let dir = home.join("proj").join("sub");
            let result = test_result("Cargo.toml");
            let chain = ancestor_chain(&dir, &result, max_upward(), false);
            assert!(chain.contains(&home.join("proj")));
            assert!(
                !chain.contains(&home),
                "home is a boundary and must not be tracked"
            );
        }
    }

    #[test]
    fn test_concurrent_stores_leave_a_valid_entry_and_no_temp_files(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (cache_dir, project, cache) = setup()?;
        let cache = Arc::new(cache);
        let result = rooted(test_result("Cargo.toml"), project.path());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (cache, result, dir) =
                    (cache.clone(), result.clone(), project.path().to_path_buf());
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        store_now(&cache, &dir, None, &result);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().map_err(|_| "writer thread panicked")?;
        }

        assert!(cache.load(project.path(), None).is_some());
        let stray: Vec<_> = fs::read_dir(cache_dir.path().join("results"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "stray temp files: {stray:?}");
        Ok(())
    }

    #[test]
    fn test_hit_on_recent_entry_does_not_rewrite_mtime() -> Result<(), Box<dyn std::error::Error>> {
        let (cache_dir, project, cache) = setup()?;
        store_now(
            &cache,
            project.path(),
            None,
            &rooted(test_result("Cargo.toml"), project.path()),
        );
        let entry = fs::read_dir(cache_dir.path().join("results"))?
            .filter_map(|e| e.ok())
            .next()
            .ok_or("entry file missing")?
            .path();

        // An hour old is well inside the GC horizon: a hit must stay read-only
        let an_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        set_file_mtime(&entry, an_hour_ago)?;
        let before = mtime(&entry).ok_or("no mtime")?;

        assert!(cache.load(project.path(), None).is_some());
        assert_eq!(mtime(&entry).ok_or("no mtime")?, before);
        Ok(())
    }
}
