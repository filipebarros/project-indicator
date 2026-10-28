//! Per-run cache for file contents and parsed JSON/TOML values.
//!
//! Manifests like `package.json` are touched by several components during one
//! detection run (root discovery, framework detector); this cache
//! ensures each file is read and parsed at most once per run. The cache lives
//! for a single CLI invocation, so there is no eviction or memory budget.

use super::json_subset::parse_manifest;
#[cfg(test)]
use super::json_subset::MAX_JSON_NODES;
use super::toml_subset::prune;
use anyhow::Result;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Largest line-oriented manifest (`requirements.txt`, `go.mod`, a pom, a
/// lockfile read as text …). Several matchers walk these line by line for
/// every dependency name, and a file of newlines is tiny in git but huge on
/// disk, so this is small: real manifests are kilobytes.
pub(crate) const MAX_TEXT_BYTES: u64 = 1024 * 1024;

/// Largest JSON or TOML manifest. Those go through the streaming
/// filters in `json_subset` and `toml_subset`, whose work and memory
/// do not grow with the file, so a big `package-lock.json` is fine.
const MAX_STREAMED_BYTES: u64 = 32 * 1024 * 1024;

/// All the files of one run together may not exceed this: a repository can
/// list many files of the per-file maximum.
const MAX_RUN_BYTES: u64 = 64 * 1024 * 1024;

/// Read a text manifest tolerantly: UTF-16 (BOM) is decoded — PowerShell 5's
/// `>` writes it — and anything else is read lossily, so one stray Latin-1
/// byte can't blank a whole manifest.
pub(crate) fn read_text(path: &Path, limit: u64) -> std::io::Result<String> {
    read_text_with_size(path, limit).map(|(text, _)| text)
}

fn read_text_with_size(path: &Path, limit: u64) -> std::io::Result<(String, u64)> {
    use std::io::{Error, ErrorKind, Read};

    // Regular files only: a repository can plant a FIFO (opening it blocks
    // forever) or a symlink to /dev/zero (reading it never ends). Unix opens
    // FIRST and inspects the handle — a stat-then-open gate can be raced by
    // swapping a FIFO in between the two. O_NONBLOCK makes opening a FIFO
    // return immediately instead of waiting for a writer, and is a no-op for
    // reads of the regular files that pass the gate.
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::File::options()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?
    };
    // Elsewhere there is no FIFO to block on: keep the stat-first gate
    #[cfg(not(unix))]
    let file = {
        if !fs::metadata(path)?.is_file() {
            return Err(Error::new(ErrorKind::InvalidInput, "not a regular file"));
        }
        fs::File::open(path)?
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(Error::new(ErrorKind::InvalidInput, "not a regular file"));
    }
    if meta.len() > limit {
        return Err(Error::new(ErrorKind::InvalidData, "file too large to read"));
    }
    let mut bytes = Vec::new();
    // `take` still bounds the read if the file grows after the size check
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::new(ErrorKind::InvalidData, "file too large to read"));
    }
    let utf16 = |rest: &[u8], from: fn([u8; 2]) -> u16| {
        let units: Vec<u16> = rest
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| from([c[0], c[1]]))
            .collect();
        char::decode_utf16(units)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    };
    let text = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => {
            // Valid UTF-8 (nearly always) reuses the buffer instead of copying
            let text = String::from_utf8(bytes)
                .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
            match text.strip_prefix('\u{feff}') {
                Some(rest) => rest.to_owned(),
                None => text,
            }
        }
    };
    Ok((text, meta.len()))
}

/// Remove `//` and `/* */` comments, leaving single- and double-quoted string
/// contents (URLs, coordinates) untouched.
pub(crate) fn strip_c_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let (mut in_str, mut escaped) = (None, false);
    while let Some(c) = chars.next() {
        if let Some(quote) = in_str {
            out.push(c);
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, _) if c == quote => in_str = None,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"' | '\'', _) => {
                in_str = Some(c);
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Default)]
pub struct ParsedFileCache {
    /// Raw file contents; `None` records a missing file so it isn't re-stat'd
    contents: RefCell<HashMap<PathBuf, Option<Arc<String>>>>,
    /// Parsed JSON values; `None` records a missing file or parse failure
    json_values: RefCell<HashMap<PathBuf, Option<Arc<serde_json::Value>>>>,
    /// Parsed TOML values; `None` records a missing file or parse failure
    toml_values: RefCell<HashMap<PathBuf, Option<Arc<toml::Value>>>>,
    /// A file that exists could not be read (permissions, I/O): the run's
    /// result was computed without it
    read_failed: Cell<bool>,
    /// Files denied this run, with the reason, so each is tried once: refused
    /// for size or the byte budget (deterministic, so the result stays
    /// cacheable and the file stays an input of it), or failed to read
    /// (`read_failed` keeps that result out of the cache)
    /// (a size refusal holds only for reads up to the limit it was made at)
    denied: RefCell<HashMap<PathBuf, (u64, String)>>,
    bytes_read: Cell<u64>,
    /// On-disk size of each file read: the limits are about the file, and
    /// decoding can make the text longer
    sizes: RefCell<HashMap<PathBuf, u64>>,
}

impl ParsedFileCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every file that was found and read this run, sorted: the inputs a
    /// result depends on, which the persistent cache has to watch
    pub fn read_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .contents
            .borrow()
            .iter()
            .filter(|(_, content)| content.is_some())
            .map(|(path, _)| path.clone())
            .collect();
        paths.extend(self.denied.borrow().keys().cloned());
        paths.sort();
        paths.dedup();
        paths
    }

    /// Whether the file exists, whether or not it parses. Looks, does not
    /// read: it neither depends on the file's size nor counts against the
    /// run's byte budget.
    pub fn is_present<P: AsRef<Path>>(&self, file_path: P) -> Result<bool> {
        let file_path = file_path.as_ref();
        if let Some(cached) = self.contents.borrow().get(file_path) {
            return Ok(cached.is_some());
        }
        Ok(fs::metadata(file_path).is_ok_and(|meta| meta.is_file()))
    }

    /// Bytes read from disk this run
    #[cfg(test)]
    fn bytes_read(&self) -> u64 {
        self.bytes_read.get()
    }

    /// Whether any file that exists failed to read this run
    pub fn read_failed(&self) -> bool {
        self.read_failed.get()
    }

    /// A line-oriented manifest, up to `MAX_TEXT_BYTES`
    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        self.read_up_to(file_path.as_ref(), MAX_TEXT_BYTES)
    }

    /// The file's text, read at most once per run, and only if it is within
    /// `limit` and the run's byte budget. `Ok(None)` means it does not exist.
    fn read_up_to(&self, file_path: &Path, limit: u64) -> Result<Option<Arc<String>>> {
        // HashMap<PathBuf, _> accepts &Path lookups via Borrow, avoiding a
        // PathBuf allocation on cache hits
        if let Some(cached) = self.contents.borrow().get(file_path) {
            // read for a larger class before: not for this one
            if self
                .sizes
                .borrow()
                .get(file_path)
                .is_some_and(|&size| size > limit)
            {
                anyhow::bail!("{} is too large to read as text", file_path.display());
            }
            return Ok(cached.clone());
        }
        if let Some((denied_at, reason)) = self.denied.borrow().get(file_path) {
            if limit <= *denied_at {
                anyhow::bail!("{reason}");
            }
        }

        let refuse = |denied_at: u64, reason: String| -> Result<Option<Arc<String>>> {
            self.denied
                .borrow_mut()
                .insert(file_path.to_path_buf(), (denied_at, reason.clone()));
            anyhow::bail!("{reason}")
        };
        let read_limit = limit.min(MAX_RUN_BYTES.saturating_sub(self.bytes_read.get()));
        let (content, size) = match read_text_with_size(file_path, read_limit) {
            Ok((content, size)) => (Arc::new(content), size),
            Err(e)
                if e.kind() == ErrorKind::InvalidData
                    && e.to_string() == "file too large to read" =>
            {
                let size = fs::metadata(file_path).map_or(0, |meta| meta.len());
                if size > limit {
                    return refuse(
                        limit,
                        format!(
                            "{} is {size} bytes, over the {limit} byte limit",
                            file_path.display()
                        ),
                    );
                }
                return refuse(
                    u64::MAX,
                    format!(
                        "{} would exceed the {MAX_RUN_BYTES} byte read budget of one run",
                        file_path.display()
                    ),
                );
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // A dangling symlink is not "absent": its target appearing
                // later moves no watched timestamp, so the result must stay
                // out of the persistent cache like any other failed read
                if fs::symlink_metadata(file_path).is_ok() {
                    self.read_failed.set(true);
                    let reason = format!("{} is a dangling symlink", file_path.display());
                    self.denied
                        .borrow_mut()
                        .insert(file_path.to_path_buf(), (u64::MAX, reason.clone()));
                    anyhow::bail!("{reason}");
                }
                self.contents
                    .borrow_mut()
                    .insert(file_path.to_path_buf(), None);
                return Ok(None);
            }
            Err(e) => {
                self.read_failed.set(true);
                let reason = format!("Failed to read file {}: {e}", file_path.display());
                self.denied
                    .borrow_mut()
                    .insert(file_path.to_path_buf(), (u64::MAX, reason.clone()));
                anyhow::bail!("{reason}");
            }
        };

        // Charge the on-disk size, like the budget check above: decoding can
        // make the text longer (lossy UTF-8) or shorter (UTF-16)
        self.bytes_read.set(self.bytes_read.get() + size);
        self.sizes
            .borrow_mut()
            .insert(file_path.to_path_buf(), size);
        self.contents
            .borrow_mut()
            .insert(file_path.to_path_buf(), Some(content.clone()));
        Ok(Some(content))
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<Arc<serde_json::Value>>> {
        let file_path = file_path.as_ref();
        self.parsed(&self.json_values, "JSON", file_path, |content| {
            parse_manifest(content).map_err(|e| e.to_string())
        })
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<toml::Value>>> {
        self.parsed(&self.toml_values, "TOML", file_path.as_ref(), |content| {
            // Only the tables the matchers read reach the full parser
            prune(content)
                .ok_or_else(|| "malformed, or too large to keep".to_owned())
                .and_then(|kept| toml::from_str::<toml::Value>(&kept).map_err(|e| e.to_string()))
        })
    }

    /// Read `file_path` as a streamed manifest and parse it once per run. A
    /// file that does not parse is memoized as `None`, so it is not
    /// re-parsed.
    fn parsed<T>(
        &self,
        memo: &RefCell<HashMap<PathBuf, Option<Arc<T>>>>,
        kind: &str,
        file_path: &Path,
        parse: impl FnOnce(&str) -> std::result::Result<T, String>,
    ) -> Result<Option<Arc<T>>> {
        if let Some(cached) = memo.borrow().get(file_path) {
            return Ok(cached.clone());
        }
        let parsed = match self.read_up_to(file_path, MAX_STREAMED_BYTES)? {
            Some(content) => match parse(&content) {
                Ok(value) => Some(Arc::new(value)),
                Err(e) => {
                    log::debug!("Failed to parse {kind} from {}: {e}", file_path.display());
                    None
                }
            },
            None => None,
        };
        memo.borrow_mut()
            .insert(file_path.to_path_buf(), parsed.clone());
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_content_is_cached() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "hello")?;

        let first = cache
            .get_file_content(&file_path)?
            .ok_or("expected content")?;
        assert_eq!(*first, "hello");

        let second = cache
            .get_file_content(&file_path)?
            .ok_or("expected content")?;
        assert_eq!(*second, "hello");
        Ok(())
    }

    const ONE_MIB: usize = 1 << 20;

    #[test]
    fn test_missing_file_is_cached_as_none() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("missing.txt");

        assert!(cache.get_file_content(&file_path)?.is_none());
        assert!(cache.get_file_content(&file_path)?.is_none());
        Ok(())
    }

    #[test]
    fn test_json_parsing_and_memoization() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("package.json");
        fs::write(&file_path, r#"{"dependencies": {"react": "18"}}"#)?;

        let value = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert_eq!(
            value["dependencies"]["react"], "18",
            "parsed JSON should expose the keys matchers read"
        );

        let again = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert!(std::sync::Arc::ptr_eq(&value, &again), "memoized");
        Ok(())
    }

    #[test]
    fn test_toml_parsing() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("Cargo.toml");
        fs::write(&file_path, "[package]\nname = \"test\"\n")?;

        let value = cache.get_toml_value(&file_path)?.ok_or("expected TOML")?;
        assert_eq!(
            value
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str()),
            Some("test")
        );
        Ok(())
    }

    #[test]
    fn test_parse_failure_returns_none_and_is_memoized() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("broken.json");
        fs::write(&file_path, "not json at all")?;

        assert!(cache.get_json_value(&file_path)?.is_none());

        // The failure is memoized for the rest of the run: fixing the file
        // does not change the cached result
        fs::write(&file_path, "{}")?;
        assert!(cache.get_json_value(&file_path)?.is_none());
        Ok(())
    }

    #[test]
    fn test_missing_file_json_lookup() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParsedFileCache::new();
        let temp_dir = TempDir::new()?;

        assert!(cache
            .get_json_value(temp_dir.path().join("missing.json"))?
            .is_none());
        Ok(())
    }

    #[test]
    fn test_utf16_and_invalid_utf8_manifests_are_readable() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        // PowerShell 5 `>` writes UTF-16LE with a BOM
        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "flask==2.0\n".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let p16 = dir.path().join("requirements.txt");
        fs::write(&p16, utf16)?;
        let text = cache.get_file_content(&p16)?.ok_or("missing")?;
        assert_eq!(text.as_str(), "flask==2.0\n");

        // A stray Latin-1 byte must not blank the whole file
        let plat = dir.path().join("Cargo.toml");
        fs::write(&plat, b"# caf\xE9\n[dependencies]\naxum = \"0.7\"\n")?;
        let text = cache.get_file_content(&plat)?.ok_or("missing")?;
        assert!(text.contains("axum = \"0.7\""));
        Ok(())
    }

    #[test]
    fn test_budget_charges_disk_bytes_not_decoded_length_for_invalid_utf8(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        // 64 invalid bytes lossy-decode to 64 three-byte replacement chars;
        // the budget is about the file, not the (longer) decoded text
        let path = dir.path().join("requirements.txt");
        fs::write(&path, [0xFF_u8; 64])?;
        let text = cache.get_file_content(&path)?.ok_or("missing")?;
        assert_eq!(text.chars().count(), 64);
        assert_eq!(cache.bytes_read(), 64);
        Ok(())
    }

    #[test]
    fn test_budget_charges_disk_bytes_not_decoded_length_for_utf16(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        // UTF-16 decodes to roughly half its disk size; charging the decoded
        // text would let a run read ~2x the budget from disk
        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "flask==2.0\n".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let disk_len = utf16.len() as u64;
        let path = dir.path().join("requirements.txt");
        fs::write(&path, utf16)?;
        cache.get_file_content(&path)?;
        assert_eq!(cache.bytes_read(), disk_len);
        Ok(())
    }

    fn write_json(name: &str, text: &str) -> Result<(TempDir, PathBuf), std::io::Error> {
        let dir = TempDir::new()?;
        let path = dir.path().join(name);
        fs::write(&path, text)?;
        Ok((dir, path))
    }

    #[test]
    fn test_json_keeps_only_the_keys_the_matchers_read() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, path) = write_json(
            "package.json",
            r#"{"name":"x","scripts":{"a":"b"},"junk":[0,0],"dependencies":{"react":"18"}}"#,
        )?;

        let value = ParsedFileCache::new()
            .get_json_value(&path)?
            .ok_or("expected JSON")?;

        assert_eq!(value["dependencies"]["react"], "18");
        assert!(["name", "scripts", "junk"]
            .iter()
            .all(|key| value.get(key).is_none()));
        Ok(())
    }

    #[test]
    fn test_too_many_nodes_under_a_read_key_make_the_file_unparseable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let filler = vec!["0"; MAX_JSON_NODES + 1].join(",");
        let (_dir, path) =
            write_json("package.json", &format!(r#"{{"dependencies":[{filler}]}}"#))?;

        assert!(ParsedFileCache::new().get_json_value(&path)?.is_none());
        Ok(())
    }

    #[test]
    fn test_a_huge_value_in_an_unread_toml_table_is_skipped(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let filler = vec!["0"; ONE_MIB].join(",");
        let (_dir, path) = write_json(
            "Cargo.toml",
            &format!("[package]\nname = \"x\"\n[profile.dev]\njunk = [{filler}]\n[dependencies]\nserde = \"1\"\n"),
        )?;

        let value = ParsedFileCache::new()
            .get_toml_value(&path)?
            .ok_or("a big file with junk only in unread tables must parse")?;

        assert_eq!(value["package"]["name"].as_str(), Some("x"));
        assert_eq!(value["dependencies"]["serde"].as_str(), Some("1"));
        assert!(value.get("profile").is_none());
        Ok(())
    }

    #[test]
    fn test_a_huge_value_in_a_read_toml_table_makes_it_unparseable_but_watched(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let filler = vec!["0"; ONE_MIB].join(",");
        let (_dir, path) = write_json(
            "Cargo.toml",
            &format!("[dependencies]\njunk = [{filler}]\n"),
        )?;
        let cache = ParsedFileCache::new();

        assert!(cache.get_toml_value(&path)?.is_none());
        assert_eq!(cache.read_paths(), vec![path]);
        Ok(())
    }

    #[test]
    fn test_read_text_enforces_the_limit_it_is_given() -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, path) = write_json("f.txt", "12345678")?;

        assert!(read_text(&path, 7).is_err());
        assert_eq!(read_text(&path, 8)?, "12345678");
        Ok(())
    }

    /// Line-oriented manifests get the small cap: a file of newlines is tiny
    /// in git, and several matchers walk it line by line per dependency name.
    /// A refusal is the same every run for the same file, so the file stays
    /// watched and the result stays cacheable.
    #[test]
    fn test_a_text_manifest_beyond_the_text_cap_is_refused_remembered_and_watched(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_dir, path) = write_json(
            "requirements.txt",
            &"a\n".repeat(MAX_TEXT_BYTES as usize / 2 + 1),
        )?;
        let cache = ParsedFileCache::new();

        assert!(cache.get_file_content(&path).is_err());
        assert!(!cache.read_failed(), "a size refusal is not an I/O failure");
        assert_eq!(cache.read_paths(), vec![path.clone()], "still an input");

        fs::remove_file(&path)?;
        assert!(
            cache.get_file_content(&path).is_err(),
            "the refusal is remembered, not turned into 'missing'"
        );
        Ok(())
    }

    /// JSON and TOML are streamed through the pruning filters, so they
    /// may be much larger than a text manifest
    #[test]
    fn test_a_text_refusal_does_not_block_the_streamed_read_of_the_same_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // root discovery reads Cargo.toml as text first (1 MiB cap); the
        // framework matchers then stream it (32 MiB cap)
        let padding = "#\n".repeat(MAX_TEXT_BYTES as usize / 2 + 1);
        let (_dir, path) = write_json(
            "Cargo.toml",
            &format!("[package]\nname = \"x\"\n{padding}[dependencies]\nserde = \"1\"\n"),
        )?;
        let cache = ParsedFileCache::new();

        assert!(cache.get_file_content(&path).is_err());
        let value = cache.get_toml_value(&path)?.ok_or("expected TOML")?;

        assert_eq!(value["dependencies"]["serde"].as_str(), Some("1"));
        assert!(!cache.read_failed());
        Ok(())
    }

    #[test]
    fn test_a_text_file_that_decodes_larger_is_readable_twice(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // invalid UTF-8 decodes to 3-byte replacement characters: 600 KB on
        // disk is 1.8 MB of text, but the limit is about the file
        let dir = TempDir::new()?;
        let path = dir.path().join("requirements.txt");
        fs::write(&path, vec![0xFF; 600_000])?;
        let cache = ParsedFileCache::new();

        assert!(cache.get_file_content(&path)?.is_some());
        assert!(cache.get_file_content(&path)?.is_some());
        Ok(())
    }

    #[test]
    fn test_a_file_read_in_two_classes_is_listed_once() -> Result<(), Box<dyn std::error::Error>> {
        let padding = "#\n".repeat(MAX_TEXT_BYTES as usize / 2 + 1);
        let (_dir, path) = write_json("Cargo.toml", &format!("[package]\n{padding}"))?;
        let cache = ParsedFileCache::new();

        assert!(cache.get_file_content(&path).is_err());
        assert!(cache.get_toml_value(&path)?.is_some());

        assert_eq!(cache.read_paths(), vec![path]);
        Ok(())
    }

    #[test]
    fn test_a_json_manifest_beyond_the_text_cap_is_still_streamed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let padding = " ".repeat(2 * MAX_TEXT_BYTES as usize);
        let (_dir, path) = write_json(
            "package.json",
            &format!(r#"{{"dependencies":{{"react":"18"}}}}{padding}"#),
        )?;
        let cache = ParsedFileCache::new();

        let value = cache.get_json_value(&path)?.ok_or("must parse")?;
        assert_eq!(value["dependencies"]["react"], "18");

        fs::remove_file(&path)?;
        assert!(cache.get_json_value(&path)?.is_some(), "read once per run");
        Ok(())
    }

    /// Asking whether a file exists neither reads it nor depends on its size
    #[test]
    fn test_presence_does_not_read_or_size_check_the_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_dir, path) = write_json("big.lock", &"x".repeat(2 * MAX_TEXT_BYTES as usize))?;
        let cache = ParsedFileCache::new();

        assert!(cache.is_present(&path)?);

        assert!(!cache.read_failed());
        assert_eq!(cache.bytes_read(), 0);
        Ok(())
    }

    /// A tiny repository must not be able to make one prompt read gigabytes
    /// by listing many big files: reads stop at a per-run byte budget
    #[test]
    fn test_reads_stop_at_the_per_run_budget() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let cache = ParsedFileCache::new();
        let big = MAX_STREAMED_BYTES - 1024;
        let files_in_budget = (MAX_RUN_BYTES / big) as usize;
        let mut paths = Vec::new();
        for i in 0..=files_in_budget {
            let path = dir.path().join(format!("f{i}.json"));
            fs::File::create(&path)?.set_len(big)?; // sparse: no disk cost
            paths.push(path);
        }

        for path in &paths {
            let _ = cache.get_json_value(path);
        }

        assert!(
            cache.bytes_read() <= MAX_RUN_BYTES,
            "read {} bytes",
            cache.bytes_read()
        );
        assert!(
            !cache.read_failed(),
            "running out of budget is deterministic, not an I/O failure"
        );
        assert_eq!(
            cache.read_paths().len(),
            paths.len(),
            "refused files stay watched"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_a_dangling_symlink_manifest_is_a_failed_read() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = TempDir::new()?;
        let link = dir.path().join("package.json");
        std::os::unix::fs::symlink(dir.path().join("missing.json"), &link)?;

        let cache = ParsedFileCache::new();
        assert!(cache.get_json_value(&link).is_err());
        assert!(
            cache.read_failed(),
            "the target appearing later moves no watched timestamp, so the \
             result must stay out of the persistent cache"
        );
        Ok(())
    }

    /// The gate is the open handle itself: a FIFO (even one swapped in after
    /// any earlier stat) opens without blocking and is refused, instead of
    /// hanging the prompt until a writer shows up.
    #[cfg(unix)]
    #[test]
    fn test_a_fifo_manifest_is_refused_without_blocking() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = TempDir::new()?;
        let path = dir.path().join("package.json");
        let status = std::process::Command::new("mkfifo").arg(&path).status()?;
        assert!(status.success(), "mkfifo failed");
        let cache = ParsedFileCache::new();

        assert!(cache.get_json_value(&path).is_err());
        assert!(
            cache.read_failed(),
            "a refused special file keeps the result out of the persistent cache"
        );
        assert_eq!(cache.read_paths(), vec![path], "still an input of the run");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_a_failed_read_is_remembered_for_the_run() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, path) = write_json("package.json", "{}")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000))?;
        if fs::read(&path).is_ok() {
            return Ok(()); // running as root
        }
        let cache = ParsedFileCache::new();

        assert!(cache.get_file_content(&path).is_err());
        assert!(cache.read_failed());

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert!(
            cache.get_file_content(&path).is_err(),
            "one failure per file per run, not a re-read on every request"
        );
        Ok(())
    }
}
