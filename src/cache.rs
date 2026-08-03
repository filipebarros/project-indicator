//! Persistent detection cache.
//!
//! Caches one detection result per project directory across CLI invocations,
//! stored as JSON files under `<cache_base>/project-indicator/results/`.
//!
//! Invalidation is evidence-keyed: an entry records the mtimes of the
//! directory, the active config file, and every file that influenced the
//! result. An entry is served only when all of them are unchanged, the
//! binary version matches, and the stored directory path matches (which also
//! makes filename-hash collisions degrade to a miss, never a wrong result).
//!
//! Every failure path degrades silently to a cache miss: the cache must
//! never be able to break a shell prompt.

use crate::types::{DetectionResult, DisplayConfig};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Opportunistic GC kicks in when the results directory exceeds this count.
const MAX_ENTRIES_BEFORE_GC: usize = 512;

/// GC removes entries whose file mtime is older than this.
const GC_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    dir: PathBuf,
    binary_version: String,
    config: Option<(PathBuf, SystemTime)>,
    dir_mtime: SystemTime,
    evidence: Vec<(PathBuf, SystemTime)>,
    result: DetectionResult,
    display: DisplayConfig,
}

pub struct PersistentCache {
    base: PathBuf,
}

impl PersistentCache {
    /// Cache rooted at an explicit base directory (used by tests).
    pub fn at_base(base: PathBuf) -> Self {
        Self { base }
    }

    /// Cache at the platform default location:
    /// `$XDG_CACHE_HOME` if set, else `~/.cache`, plus `project-indicator/results`.
    pub fn default_location() -> Option<Self> {
        let cache_home = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| dirs::home_dir().map(|home| home.join(".cache")))?;

        Some(Self::at_base(
            cache_home.join("project-indicator").join("results"),
        ))
    }

    /// Load a cached result for `dir` if present and still valid.
    ///
    /// Any error (missing file, corrupt JSON, mtime mismatch, version or
    /// config change) is a cache miss.
    pub fn load(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
    ) -> Option<(DetectionResult, DisplayConfig)> {
        let dir = canonical(dir);
        let content = fs::read_to_string(self.entry_path(&dir)).ok()?;
        let entry: CacheEntry = serde_json::from_str(&content).ok()?;

        if entry.dir != dir || entry.binary_version != env!("CARGO_PKG_VERSION") {
            return None;
        }

        let current_config = config_path.map(|p| (p.to_path_buf(), mtime(p)));
        let stored_config = entry.config.as_ref().map(|(p, t)| (p.clone(), Some(*t)));
        if current_config != stored_config {
            return None;
        }

        if mtime(&dir)? != entry.dir_mtime {
            return None;
        }

        for (path, stored) in &entry.evidence {
            if mtime(path)? != *stored {
                return None;
            }
        }

        Some((entry.result, entry.display))
    }

    /// Store a detection result for `dir`. Failures are logged at debug
    /// level and otherwise ignored.
    pub fn store(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
        result: &DetectionResult,
        display: &DisplayConfig,
    ) {
        if let Err(e) = self.try_store(dir, config_path, result, display) {
            log::debug!("persistent cache write skipped: {}", e);
        }
        self.gc();
    }

    fn try_store(
        &self,
        dir: &Path,
        config_path: Option<&Path>,
        result: &DetectionResult,
        display: &DisplayConfig,
    ) -> crate::Result<()> {
        let dir = canonical(dir);

        let config = match config_path {
            Some(p) => match mtime(p) {
                Some(t) => Some((p.to_path_buf(), t)),
                // Config exists per caller but is not stat-able: don't cache
                None => return Ok(()),
            },
            None => None,
        };

        let dir_mtime = match mtime(&dir) {
            Some(t) => t,
            None => return Ok(()),
        };

        let evidence = collect_evidence(&dir, result)
            .into_iter()
            .filter_map(|p| mtime(&p).map(|t| (p, t)))
            .collect();

        let entry = CacheEntry {
            dir: dir.clone(),
            binary_version: env!("CARGO_PKG_VERSION").to_string(),
            config,
            dir_mtime,
            evidence,
            result: result.clone(),
            display: display.clone(),
        };

        fs::create_dir_all(&self.base)?;
        let final_path = self.entry_path(&dir);
        let tmp_path = final_path.with_extension("json.tmp");
        fs::write(&tmp_path, serde_json::to_vec(&entry)?)?;
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    /// Delete all cached entries.
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
                    bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
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

    /// Remove entries older than the GC age when the dir has grown large.
    /// Best-effort: all errors ignored.
    fn gc(&self) {
        let Ok(entries) = fs::read_dir(&self.base) else {
            return;
        };
        let paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        if paths.len() <= MAX_ENTRIES_BEFORE_GC {
            return;
        }
        let now = SystemTime::now();
        for path in paths {
            let expired = mtime(&path)
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age.as_secs() > GC_MAX_AGE_SECS);
            if expired {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

fn canonical(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Absolute paths of every file that influenced the result: language,
/// framework, and root-discovery evidence items plus per-framework evidence
/// file names. Relative paths are resolved against the project directory.
fn collect_evidence(dir: &Path, result: &DetectionResult) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();

    let items = result
        .evidence
        .indicator_evidence
        .iter()
        .chain(&result.evidence.framework_evidence)
        .chain(&result.evidence.root_discovery)
        .map(|item| item.file_path.as_str());

    let framework_files = result
        .frameworks
        .iter()
        .flat_map(|f| f.evidence.iter().map(String::as_str));

    for raw in items.chain(framework_files) {
        let p = Path::new(raw);
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            dir.join(p)
        };
        paths.insert(abs);
    }

    paths
}

#[cfg(test)]
mod tests {
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
            1,
            vec![],
        );
        let mut result = DetectionResult::new(Some(Arc::new(lang)), vec![], 0.9);
        result
            .evidence
            .indicator_evidence
            .push(crate::types::EvidenceItem::new(
                crate::types::EvidenceType::ManifestFile,
                evidence_file.to_string(),
                evidence_file.to_string(),
                0.8,
                "test evidence".to_string(),
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

    #[test]
    fn test_store_then_load_hits() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        let (loaded, display) = cache
            .load(project.path(), None)
            .ok_or("expected cache hit")?;
        assert_eq!(loaded, result);
        assert_eq!(display, DisplayConfig::default());
        Ok(())
    }

    #[test]
    fn test_touched_evidence_file_invalidates() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        sleep(Duration::from_millis(20));
        fs::write(project.path().join("Cargo.toml"), "[package]\nname=\"x\"")?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_new_root_file_invalidates_via_dir_mtime() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        sleep(Duration::from_millis(20));
        fs::write(project.path().join("package.json"), "{}")?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_config_mtime_change_invalidates() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let config_dir = TempDir::new()?;
        let config_path = config_dir.path().join("config.toml");
        fs::write(&config_path, "[meta]\nversion = \"2.0\"")?;

        let result = test_result("Cargo.toml");
        cache.store(
            project.path(),
            Some(&config_path),
            &result,
            &DisplayConfig::default(),
        );
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

        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        // Cached without a config; now one exists → miss
        assert!(cache.load(project.path(), Some(&config_path)).is_none());
        Ok(())
    }

    #[test]
    fn test_corrupt_entry_is_a_miss() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        let entry_path = cache.entry_path(&canonical(project.path()));
        fs::write(&entry_path, "not json at all {{{")?;

        assert!(cache.load(project.path(), None).is_none());
        Ok(())
    }

    #[test]
    fn test_version_mismatch_is_a_miss() -> Result<(), Box<dyn std::error::Error>> {
        let (_c, project, cache) = setup()?;
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

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
        let result = test_result("Cargo.toml");
        cache.store(project.path(), None, &result, &DisplayConfig::default());

        let (count, bytes) = cache.stats()?;
        assert_eq!(count, 1);
        assert!(bytes > 0);

        cache.clear()?;
        let (count, _) = cache.stats()?;
        assert_eq!(count, 0);
        Ok(())
    }
}
