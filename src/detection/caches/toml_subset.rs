//! A streaming pre-filter for TOML manifests.
//!
//! The `toml` crate builds a full tree of whatever it is given, at around a
//! hundred bytes per input byte of dense data, so a hostile 23 MB `Cargo.toml`
//! needs gigabytes. The matchers only read a few tables. This walks the
//! lexer's token stream once, tracking just the current table and bracket
//! depth, and copies only the statements under those tables into a small
//! document that the normal parser then handles. Everything else is skipped
//! in constant memory, and all value grammar (escapes, dates, inline tables)
//! stays with the real parser.

use toml_parser::lexer::{Token, TokenKind};
use toml_parser::Source;

/// Top-level tables some matcher reads. Add a table here when a matcher
/// starts reading a new one, or it will silently see nothing.
pub(crate) const KEPT_TABLES: [&str; 10] = [
    "package",
    "build-system",
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
    "workspace",
    "target",
    "project",
    "dependency-groups",
    "tool",
];

/// Kept text beyond this makes the file unparseable. Real manifests are tens
/// of kilobytes; this bounds what the full parser is ever handed.
pub(crate) const MAX_KEPT_BYTES: usize = 1_048_576;

/// The statements of `text` that sit under `KEPT_TABLES`, as TOML text, or
/// `None` when the structure is malformed or the kept text is too large.
/// Only structure is checked here: a bad value inside a skipped statement is
/// not noticed.
pub(crate) fn prune(text: &str) -> Option<String> {
    let mut tokens = Source::new(text)
        .lex()
        .filter(|t| !matches!(t.kind(), TokenKind::Whitespace | TokenKind::Comment));
    let mut out = String::new();
    // inside a [table]: whether it is kept; at the root: decided per key
    let mut table: Option<bool> = None;

    loop {
        let first = match tokens.next() {
            None => break,
            Some(t) => match t.kind() {
                TokenKind::Newline => continue,
                TokenKind::Eof => break,
                _ => t,
            },
        };
        let start = first.span().start();
        let (kept, end) = match first.kind() {
            TokenKind::LeftSquareBracket => {
                let (segment, end) = header(text, &mut tokens)?;
                let kept = segment_is_kept(&segment);
                table = Some(kept);
                (kept, end)
            }
            TokenKind::Atom | TokenKind::BasicString | TokenKind::LiteralString => {
                let segment = text.get(first.span().start()..first.span().end())?;
                key_rest(&mut tokens)?;
                let end = value_end(&mut tokens)?;
                (table.unwrap_or_else(|| segment_is_kept(segment)), end)
            }
            _ => return None,
        };
        if kept {
            let statement = text.get(start..end)?;
            // budget check before the append: refusing a hostile file must
            // not itself cost a transient multi-megabyte allocation
            if out.len() + statement.len() + 1 > MAX_KEPT_BYTES {
                return None;
            }
            out.push_str(statement);
            out.push('\n');
        }
    }
    Some(out)
}

/// After the opening `[` of a table header: the first key segment, and where
/// the closing bracket(s) end. Consumes the rest of the line.
fn header(text: &str, tokens: &mut impl Iterator<Item = Token>) -> Option<(String, usize)> {
    let mut first_segment = None;
    let mut opened = 1;
    let mut end;
    loop {
        let token = tokens.next()?;
        match token.kind() {
            TokenKind::LeftSquareBracket if first_segment.is_none() => opened += 1,
            TokenKind::Atom | TokenKind::BasicString | TokenKind::LiteralString => {
                if first_segment.is_none() {
                    first_segment = Some(
                        text.get(token.span().start()..token.span().end())?
                            .to_owned(),
                    );
                }
            }
            TokenKind::Dot => {}
            TokenKind::RightSquareBracket => {
                end = token.span().end();
                opened -= 1;
                if opened == 0 {
                    break;
                }
            }
            _ => return None,
        }
    }
    // nothing but a comment (already filtered out) may follow on the line
    match tokens.next().map(|t| t.kind()) {
        None | Some(TokenKind::Newline | TokenKind::Eof) => Some((first_segment?, end)),
        Some(_) => None,
    }
}

/// The rest of a (possibly dotted) key, up to and including the `=`
fn key_rest(tokens: &mut impl Iterator<Item = Token>) -> Option<()> {
    loop {
        match tokens.next()?.kind() {
            TokenKind::Dot
            | TokenKind::Atom
            | TokenKind::BasicString
            | TokenKind::LiteralString => {}
            TokenKind::Equals => return Some(()),
            _ => return None,
        }
    }
}

/// Skip a value, tracking brackets, and return where its last token ends.
/// Consumes the newline that ends the statement.
fn value_end(tokens: &mut impl Iterator<Item = Token>) -> Option<usize> {
    let mut depth = 0usize;
    let mut end = None;
    loop {
        let Some(token) = tokens.next() else {
            return if depth == 0 { end } else { None };
        };
        match token.kind() {
            TokenKind::Newline | TokenKind::Eof if depth == 0 => return end,
            TokenKind::Eof => return None,
            TokenKind::Newline => continue,
            TokenKind::LeftSquareBracket | TokenKind::LeftCurlyBracket => depth += 1,
            TokenKind::RightSquareBracket | TokenKind::RightCurlyBracket => {
                depth = depth.checked_sub(1)?;
            }
            _ => {}
        }
        end = Some(token.span().end());
    }
}

/// Whether a key segment names a kept table. Quotes are stripped; a segment
/// with escapes is kept, since it could spell anything.
fn segment_is_kept(segment: &str) -> bool {
    let bare = segment.trim_matches(['"', '\'']);
    segment.contains('\\') || KEPT_TABLES.contains(&bare)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn pruned(text: &str) -> String {
        prune(text).unwrap_or_default()
    }

    #[test]
    fn test_tables_the_matchers_read_are_kept_and_others_dropped() {
        let out = pruned(
            "[package]\nname = \"x\"\n\n[profile.release]\nlto = true\n\n[dependencies]\nserde = \"1\"\n",
        );
        assert!(out.contains("[package]") && out.contains("name = \"x\""));
        assert!(out.contains("[dependencies]") && out.contains("serde = \"1\""));
        assert!(!out.contains("profile") && !out.contains("lto"), "{out}");
    }

    #[test]
    fn test_root_level_dotted_keys_follow_their_first_segment() {
        let out = pruned("tool.poetry.name = \"x\"\nother.key = 1\n");
        assert!(out.contains("tool.poetry.name"));
        assert!(!out.contains("other"));
    }

    #[test]
    fn test_inline_tables_and_multiline_arrays_are_kept_whole() {
        let out = pruned("dependencies = { a = 1, b = [\n  1,\n  2,\n] }\nskipme = [\n 1,\n]\n");
        assert!(out.contains("b = [\n  1,\n  2,\n]"), "{out}");
        assert!(!out.contains("skipme"));
    }

    #[test]
    fn test_a_bracket_inside_a_skipped_multiline_string_is_not_a_header() {
        let out =
            pruned("notes = \"\"\"\n[dependencies]\nevil = 1\n\"\"\"\n[package]\nname = \"x\"\n");
        assert!(!out.contains("evil"), "{out}");
        assert!(out.contains("name = \"x\""));
    }

    #[test]
    fn test_array_of_tables_headers_are_handled() {
        let out = pruned("[[tool.x]]\na = 1\n[[other]]\nb = 2\n");
        assert!(out.contains("[[tool.x]]") && out.contains("a = 1"));
        assert!(!out.contains("other"));
    }

    #[test]
    fn test_a_quoted_key_is_matched_by_its_text() {
        assert!(pruned("\"dependencies\" = { a = 1 }\n").contains("dependencies"));
    }

    #[test]
    fn test_comments_and_blank_lines_do_not_confuse_the_walk() {
        let out = pruned("# header\n\n[package] # trailing\nname = \"x\" # c\n");
        assert!(out.contains("[package]") && out.contains("name = \"x\""));
    }

    #[test]
    fn test_a_malformed_header_is_unparseable() {
        assert!(prune("[package\nname =").is_none());
    }

    #[test]
    fn test_a_key_without_a_value_is_unparseable() {
        assert!(prune("[package]\nname\n").is_none());
    }

    #[test]
    fn test_kept_text_beyond_the_budget_is_unparseable() {
        let filler = vec!["0"; MAX_KEPT_BYTES].join(",");
        assert!(prune(&format!("[dependencies]\njunk = [{filler}]\n")).is_none());
    }

    #[test]
    fn test_kept_text_exactly_at_the_budget_is_allowed_and_one_byte_over_is_not() {
        // kept output: "[dependencies]\n" (15) + the junk statement + '\n'
        let doc = |n: usize| format!("[dependencies]\njunk = \"{}\"\n", "x".repeat(n));
        let at_budget = MAX_KEPT_BYTES - 15 - "junk = \"\"".len() - 1;
        assert!(prune(&doc(at_budget)).is_some());
        assert!(prune(&doc(at_budget + 1)).is_none());
    }

    fn kept_projection(value: &toml::Value) -> toml::value::Table {
        let mut table = toml::value::Table::new();
        for key in KEPT_TABLES {
            if let Some(v) = value.get(key) {
                table.insert(key.to_string(), v.clone());
            }
        }
        table
    }

    fn toml_files(dir: &Path, into: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                toml_files(&path, into)?;
            } else if path.extension().is_some_and(|e| e == "toml") {
                into.push(path);
            }
        }
        Ok(())
    }

    fn assert_pruning_preserves_kept_tables(name: &str, text: &str) -> Result<(), String> {
        let Ok(full) = toml::from_str::<toml::Value>(text) else {
            return Ok(()); // deliberately broken documents are covered elsewhere
        };
        let pruned = prune(text).ok_or_else(|| format!("{name}: pruner refused"))?;
        let slim: toml::Value = toml::from_str(&pruned)
            .map_err(|e| format!("{name}: pruned text invalid: {e}\n{pruned}"))?;
        if kept_projection(&slim) == kept_projection(&full) {
            Ok(())
        } else {
            Err(format!("{name}: kept tables differ\npruned:\n{pruned}"))
        }
    }

    /// The pruned document must mean exactly what the full document means,
    /// for the tables that are kept: checked on every real manifest here.
    #[test]
    fn test_pruning_changes_nothing_for_the_kept_tables_in_real_manifests(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = vec![root.join("Cargo.toml")];
        toml_files(&root.join("tests/fixtures"), &mut files)?;
        assert!(
            files.len() >= 10,
            "expected real manifests, got {}",
            files.len()
        );

        for file in files {
            let text = std::fs::read_to_string(&file)?;
            assert_pruning_preserves_kept_tables(&file.display().to_string(), &text)?;
        }
        Ok(())
    }

    /// Valid but awkward documents a real-world manifest could contain
    #[test]
    fn test_pruning_changes_nothing_for_the_kept_tables_in_awkward_manifests() -> Result<(), String>
    {
        const CORPUS: [(&str, &str); 9] = [
            ("poetry", "[tool.poetry]\nname = \"x\"\n[tool.poetry.dependencies]\npython = \"^3.12\"\nDjango = { version = \"^5\", extras = [\"argon2\"] }\n[tool.poetry.group.dev.dependencies]\npytest = \"*\"\n[tool.black]\nline-length = 100\n"),
            ("pdm and uv", "[project]\nname = \"y\"\ndependencies = [\n  \"flask>=3\",  # web\n  \"requests\",\n]\n[tool.pdm.dev-dependencies]\ntest = [\"pytest\"]\n[tool.uv]\ndev-dependencies = [\"ruff\"]\n"),
            ("workspace", "[workspace]\nmembers = [\"a\", \"b\"]\n[workspace.dependencies]\naxum = \"0.7\"\ntokio = { version = \"1\", features = [\"full\"] }\n[profile.release]\nlto = true\n"),
            ("target tables", "[package]\nname = \"z\"\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n[target.\"cfg(windows)\".dev-dependencies]\nwinapi = \"0.3\"\n"),
            ("dotted keys", "package.name = \"d\"\ndependencies.serde = \"1\"\ndependencies.rand.version = \"0.8\"\nprofile.dev.opt-level = 1\n"),
            ("renamed dependency", "[dependencies]\nweb = { package = \"axum\", version = \"0.7\" }\n[features]\ndefault = [\"web\"]\n"),
            ("brackets in strings", "[package]\nname = \"[dependencies]\"\ndescription = \"\"\"\n[evil]\nx = 1\n\"\"\"\n[dependencies]\na = \"1\"\n"),
            ("array of tables", "[[bin]]\nname = \"one\"\n[[bin]]\nname = \"two\"\n[package]\nname = \"p\"\n[[tool.maturin.targets]]\nname = \"t\"\n"),
            ("crlf and comments", "# top\r\n[package] # p\r\nname = \"c\" # n\r\n\r\n[dependencies]\r\nserde = \"1\" # s\r\n"),
        ];
        for (name, text) in CORPUS {
            assert_pruning_preserves_kept_tables(name, text)?;
        }
        Ok(())
    }
}
