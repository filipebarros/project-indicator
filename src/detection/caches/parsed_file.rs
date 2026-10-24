//! Per-run cache for file contents and parsed JSON/TOML values.
//!
//! Manifests like `package.json` are touched by several components during one
//! detection run (root discovery, framework detector); this cache
//! ensures each file is read and parsed at most once per run. The cache lives
//! for a single CLI invocation, so there is no eviction or memory budget.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Files above this size are read but not cached (keeps the per-run cache small).
const MAX_FILE_SIZE: u64 = 1_048_576;

/// Read a text manifest tolerantly: UTF-16 (BOM) is decoded — PowerShell 5's
/// `>` writes it — and anything else is read lossily, so one stray Latin-1
/// byte can't blank a whole manifest.
pub(crate) fn read_text(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
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
    Ok(match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        _ => {
            let text = String::from_utf8_lossy(&bytes);
            text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned()
        }
    })
}

/// Remove `//` and `/* */` comments and trailing commas, leaving string
/// contents untouched.
fn strip_jsonc(src: &str) -> String {
    strip_trailing_commas(&strip_c_comments(src))
}

/// Remove `//` and `/* */` comments, leaving double-quoted string contents
/// (URLs, coordinates) untouched. Single quotes are not tracked, so a `//`
/// inside a single-quoted string drops the rest of its line.
pub(crate) fn strip_c_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let (mut in_str, mut escaped) = (false, false);
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_str = true;
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

fn strip_trailing_commas(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let (mut in_str, mut escaped) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        if in_str {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_str = false,
                _ => {}
            }
        } else if c == '"' {
            in_str = true;
        } else if c == ',' {
            let next = chars[i + 1..].iter().find(|n| !n.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Locks are only held for a lookup or an insert, never across file I/O or
/// parsing; a poisoned lock still holds valid data, so recover it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Default)]
pub struct ParsedFileCache {
    /// Raw file contents; `None` records a missing file so it isn't re-stat'd
    contents: Mutex<HashMap<PathBuf, Option<Arc<String>>>>,
    /// Parsed JSON values; `None` records a missing file or parse failure
    json_values: Mutex<HashMap<PathBuf, Option<Arc<serde_json::Value>>>>,
    /// Parsed TOML values; `None` records a missing file or parse failure
    toml_values: Mutex<HashMap<PathBuf, Option<Arc<toml::Value>>>>,
    /// Files read but too large to keep in `contents`
    oversized: Mutex<Vec<PathBuf>>,
}

impl ParsedFileCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every file that was found and read this run, sorted: the inputs a
    /// result depends on, which the persistent cache has to watch
    pub fn read_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = lock(&self.contents)
            .iter()
            .filter(|(_, content)| content.is_some())
            .map(|(path, _)| path.clone())
            .collect();
        paths.extend(lock(&self.oversized).iter().cloned());
        paths.sort();
        paths.dedup();
        paths
    }

    pub fn get_file_content<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<String>>> {
        let file_path = file_path.as_ref();

        // HashMap<PathBuf, _> accepts &Path lookups via Borrow, avoiding a
        // PathBuf allocation on cache hits
        if let Some(cached) = lock(&self.contents).get(file_path) {
            return Ok(cached.clone());
        }

        if !file_path.exists() {
            lock(&self.contents).insert(file_path.to_path_buf(), None);
            return Ok(None);
        }

        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

        if metadata.len() > MAX_FILE_SIZE {
            // Read but don't cache oversized files
            let content = read_text(file_path)
                .with_context(|| format!("Failed to read large file: {}", file_path.display()))?;
            lock(&self.oversized).push(file_path.to_path_buf());
            return Ok(Some(Arc::new(content)));
        }

        let content = read_text(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let arc_content = Arc::new(content);
        lock(&self.contents).insert(file_path.to_path_buf(), Some(arc_content.clone()));

        Ok(Some(arc_content))
    }

    pub fn get_json_value<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Option<Arc<serde_json::Value>>> {
        let file_path = file_path.as_ref();

        if let Some(cached) = lock(&self.json_values).get(file_path) {
            return Ok(cached.clone());
        }

        // Deno accepts comments in deno.json as well as deno.jsonc
        let jsonc = file_path.extension().is_some_and(|e| e == "jsonc")
            || file_path.file_name().is_some_and(|n| n == "deno.json");
        let parsed = match self.get_file_content(file_path)? {
            Some(content) => match if jsonc {
                // JSONC: comments and trailing commas are legal
                serde_json::from_str::<serde_json::Value>(&strip_jsonc(&content))
            } else {
                serde_json::from_str::<serde_json::Value>(&content)
            } {
                Ok(value) => Some(Arc::new(value)),
                Err(e) => {
                    // Memoize the failure so the file isn't re-parsed this run
                    log::debug!("Failed to parse JSON from {}: {}", file_path.display(), e);
                    None
                }
            },
            None => None,
        };

        lock(&self.json_values).insert(file_path.to_path_buf(), parsed.clone());

        Ok(parsed)
    }

    pub fn get_toml_value<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Arc<toml::Value>>> {
        let file_path = file_path.as_ref();

        if let Some(cached) = lock(&self.toml_values).get(file_path) {
            return Ok(cached.clone());
        }

        let parsed = match self.get_file_content(file_path)? {
            Some(content) => match toml::from_str::<toml::Value>(&content) {
                Ok(value) => Some(Arc::new(value)),
                Err(e) => {
                    // Memoize the failure so the file isn't re-parsed this run
                    log::debug!("Failed to parse TOML from {}: {}", file_path.display(), e);
                    None
                }
            },
            None => None,
        };

        lock(&self.toml_values).insert(file_path.to_path_buf(), parsed.clone());

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
        fs::write(&file_path, r#"{"name": "test", "version": "1.0.0"}"#)?;

        let value = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert_eq!(
            value.get("name").and_then(|n| n.as_str()),
            Some("test"),
            "parsed JSON should expose fields"
        );

        // Second lookup hits the parsed-value memo
        let value = cache.get_json_value(&file_path)?.ok_or("expected JSON")?;
        assert_eq!(value.get("version").and_then(|v| v.as_str()), Some("1.0.0"));
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
    fn test_strip_jsonc_comments_and_trailing_commas() {
        let src = "{\n // c\n \"u\": \"http://x//y\", /* b */\n \"a\": [1,],\n}";
        let v: serde_json::Value = match serde_json::from_str(&strip_jsonc(src)) {
            Ok(v) => v,
            Err(e) => panic!("stripped output must parse: {e}"),
        };
        assert_eq!(v["u"], "http://x//y");
        assert_eq!(v["a"][0], 1);
    }

    #[test]
    fn test_strip_jsonc_unterminated_block_comment_and_edges() {
        // An unterminated block comment swallows the rest — parse fails soft
        assert!(
            serde_json::from_str::<serde_json::Value>(&strip_jsonc("{} /* never closed")).is_ok()
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(&strip_jsonc(r#"{"a": 1 /*/ x */}"#)).is_ok()
        );
        // Comment markers inside strings survive
        let v: serde_json::Value =
            serde_json::from_str(&strip_jsonc(r#"{"u": "a/*b*/c//d"}"#)).unwrap_or_default();
        assert_eq!(v["u"], "a/*b*/c//d");
    }

    #[test]
    fn test_jsonc_with_comments_is_parsed_and_cached() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let path = dir.path().join("deno.jsonc");
        fs::write(
            &path,
            "{\n  // a comment\n  \"imports\": {\"oak\": \"jsr:@oak/oak\",},\n}\n",
        )?;

        let cache = ParsedFileCache::new();
        let first = cache.get_json_value(&path)?.ok_or("jsonc must parse")?;
        assert_eq!(first["imports"]["oak"], "jsr:@oak/oak");
        // memoized: the same Arc comes back
        let second = cache.get_json_value(&path)?.ok_or("cached")?;
        assert!(std::sync::Arc::ptr_eq(&first, &second));

        // a plain .json with comments is still invalid
        let strict = dir.path().join("x.json");
        fs::write(&strict, "{ // no\n}")?;
        assert!(cache.get_json_value(&strict)?.is_none());
        Ok(())
    }

    #[test]
    fn test_deno_json_accepts_comments_like_deno_does() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let path = dir.path().join("deno.json");
        fs::write(
            &path,
            "{\n  // comment\n  \"imports\": {\"oak\": \"jsr:@oak/oak\"},\n}\n",
        )?;

        let value = ParsedFileCache::new()
            .get_json_value(&path)?
            .ok_or("deno.json with comments must parse")?;
        assert_eq!(value["imports"]["oak"], "jsr:@oak/oak");
        Ok(())
    }
}
