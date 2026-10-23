use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Deno ecosystem dependencies via the `imports` map in
/// deno.json/deno.jsonc. deno.jsonc is parsed here (comments and trailing
/// commas stripped) rather than through the strict JSON cache, so a
/// commented file matches and no parse warning is emitted.
pub fn check_deno_ecosystem<P: AsRef<Path>>(
    path: P,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<(Vec<String>, Vec<String>)> {
    for config in ["deno.json", "deno.jsonc"] {
        let json = read_deno_config(path.as_ref(), config, parsed_cache)?;
        if let Some(imports) = json
            .as_ref()
            .and_then(|j| j.get("imports"))
            .and_then(|v| v.as_object())
        {
            let found: Vec<String> = dependencies
                .iter()
                .filter(|dep| imports_match(imports, dep))
                .cloned()
                .collect();
            if !found.is_empty() {
                return Ok((found, vec![config.to_string()]));
            }
        }
    }

    Ok((Vec::new(), Vec::new()))
}

/// Load deno.json (strict JSON, cached) or deno.jsonc (comments and trailing
/// commas tolerated, no parse warning). Shared with the npm matcher's
/// `npm:` import fallback.
pub(super) fn read_deno_config(
    dir: &Path,
    file_name: &str,
    parsed_cache: &ParsedFileCache,
) -> Result<Option<serde_json::Value>> {
    let path = dir.join(file_name);
    if file_name.ends_with(".jsonc") {
        Ok(parsed_cache
            .get_file_content(&path)?
            .and_then(|c| serde_json::from_str(&strip_jsonc(&c)).ok()))
    } else {
        parsed_cache.get_json_value(&path)
    }
}

/// Remove `//` and `/* */` comments and trailing commas, leaving string
/// contents untouched.
fn strip_jsonc(src: &str) -> String {
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
    strip_trailing_commas(&out)
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

/// The package name inside `rest` of a `jsr:@scope/name[@ver][/path]` or
/// `npm:name[@ver][/path]` specifier. `scoped` keeps `@scope/name` whole.
fn package_name(rest: &str) -> &str {
    let (scope_len, body) = match rest.strip_prefix('@') {
        Some(after) => match after.split_once('/') {
            Some((scope, tail)) => (scope.len() + 2, tail),
            None => return rest,
        },
        None => (0, rest),
    };
    let name_end = body.find(['@', '/']).unwrap_or(body.len());
    &rest[scope_len..scope_len + name_end]
}

/// A dependency matches an imports-map entry when its alias key names it
/// (`"oak"`, `"$fresh/"`) or its specifier value points at that exact
/// package: `jsr:@scope/<dep>`, `npm:<dep>`, or `deno.land/x/<dep>@`. The
/// package segment must equal the dependency — a sibling in the same scope
/// (`jsr:@hono/node-server`) or a look-alike (`npm:hono-jsx`) is not a match.
fn imports_match(imports: &serde_json::Map<String, serde_json::Value>, dep: &str) -> bool {
    imports.iter().any(|(key, value)| {
        let alias = key.strip_prefix('$').unwrap_or(key);
        let alias = alias.strip_suffix('/').unwrap_or(alias);
        if alias == dep {
            return true;
        }

        value.as_str().is_some_and(|v| {
            if let Some(rest) = v.strip_prefix("jsr:@") {
                // `@scope/name` dependency: scope and package must both match
                if let Some(scoped) = dep.strip_prefix('@') {
                    return rest.split_once('/').is_some_and(|(scope, tail)| {
                        scoped.split_once('/') == Some((scope, package_name(tail)))
                    });
                }
                return rest
                    .split_once('/')
                    .is_some_and(|(_, tail)| package_name(tail) == dep);
            }
            if let Some(rest) = v.strip_prefix("npm:") {
                return package_name(rest) == dep;
            }
            v.contains(&format!("/x/{dep}@"))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn check(deno_json: &str, deps: &[&str]) -> Vec<String> {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        if let Err(e) = fs::write(dir.path().join("deno.json"), deno_json) {
            panic!("write fixture: {e}");
        }
        let cache = ParsedFileCache::new();
        let deps: Vec<String> = deps.iter().map(|s| s.to_string()).collect();
        match check_deno_ecosystem(dir.path(), &deps, &cache) {
            Ok((found, _)) => found,
            Err(e) => panic!("matcher failed: {e}"),
        }
    }

    #[test]
    fn test_matches_alias_key_and_fresh_style_key() {
        let json = r#"{"imports": {"oak": "jsr:@oak/oak@^17", "$fresh/": "https://deno.land/x/fresh@1.7.3/"}}"#;
        assert_eq!(
            check(json, &["oak", "fresh"]),
            vec!["oak".to_string(), "fresh".to_string()]
        );
    }

    #[test]
    fn test_matches_specifier_value_without_alias() {
        let json = r#"{"imports": {"web": "npm:hono@4.6.0"}}"#;
        assert_eq!(check(json, &["hono"]), vec!["hono".to_string()]);
    }

    #[test]
    fn test_does_not_match_name_substrings() {
        // "cloak" must not satisfy "oak"; "preact" must not satisfy "react"
        let json =
            r#"{"imports": {"cloak": "jsr:@cloak/cloak@1", "preact": "https://esm.sh/preact@10"}}"#;
        assert!(check(json, &["oak", "react"]).is_empty());
    }

    #[test]
    fn test_jsr_scope_alone_is_not_a_package_match() {
        // @hono/node-server and @oak/commons are other packages in the scope
        let json = r#"{"imports": {"a": "jsr:@hono/node-server@1", "b": "jsr:@oak/commons@1"}}"#;
        assert!(check(json, &["hono", "oak"]).is_empty());
    }

    #[test]
    fn test_jsr_package_forms_match() {
        let json = r#"{"imports": {"a": "jsr:@hono/hono@^4", "b": "jsr:@oak/oak/router"}}"#;
        assert_eq!(check(json, &["hono", "oak"]), vec!["hono", "oak"]);
    }

    #[test]
    fn test_npm_subpath_and_lookalikes() {
        let json = r#"{"imports": {"a": "npm:hono/jsx", "b": "npm:hono-jsx@1", "c": "npm:@hono/node-server"}}"#;
        assert_eq!(check(json, &["hono"]), vec!["hono"]);
        let json = r#"{"imports": {"b": "npm:hono-jsx@1", "c": "npm:@hono/node-server"}}"#;
        assert!(check(json, &["hono"]).is_empty());
    }

    #[test]
    fn test_alias_key_lookalikes_do_not_match() {
        let json = r#"{"imports": {"hono-jsx": "https://x/y.ts", "$$oak//": "https://x/z.ts"}}"#;
        assert!(check(json, &["hono", "oak"]).is_empty());
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
    fn test_scoped_dependency_matches_jsr_scope_and_name() {
        // Aliasless: the specifier alone identifies @fresh/core
        let json = r#"{"imports": {"x": "jsr:@fresh/core@^2.0.0"}}"#;
        assert_eq!(check(json, &["@fresh/core"]), vec!["@fresh/core"]);
        let other = r#"{"imports": {"x": "jsr:@fresh/plugin-tailwind@1"}}"#;
        assert!(check(other, &["@fresh/core"]).is_empty());
    }
}
