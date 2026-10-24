use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect Deno ecosystem dependencies via the `imports` map in
/// deno.json/deno.jsonc (the parse cache tolerates comments in `.jsonc`).
pub fn check_deno_ecosystem(
    path: &Path,
    dependencies: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    for config in ["deno.json", "deno.jsonc"] {
        if let Some(json) = parsed_cache.get_json_value(path.join(config))? {
            if imports_dependencies(&json, dependencies) {
                return Ok(Some(config.to_owned()));
            }
        }
    }

    Ok(None)
}

/// Whether a deno.json `imports` map names any of `dependencies` (see
/// [`imports_match`] for what counts). Shared with the npm matcher's `npm:`
/// import fallback.
pub(super) fn imports_dependencies(json: &serde_json::Value, dependencies: &[String]) -> bool {
    json.get("imports")
        .and_then(|v| v.as_object())
        .is_some_and(|imports| dependencies.iter().any(|dep| imports_match(imports, dep)))
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

    /// The dependencies of `deps` the imports map names (each checked alone)
    fn check(deno_json: &str, deps: &[&str]) -> Vec<String> {
        let dir = TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
        if let Err(e) = fs::write(dir.path().join("deno.json"), deno_json) {
            panic!("write fixture: {e}");
        }
        let cache = ParsedFileCache::new();
        deps.iter()
            .filter(
                |dep| match check_deno_ecosystem(dir.path(), &[dep.to_string()], &cache) {
                    Ok(found) => found.is_some(),
                    Err(e) => panic!("matcher failed: {e}"),
                },
            )
            .map(|dep| dep.to_string())
            .collect()
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
    fn test_scoped_dependency_matches_jsr_scope_and_name() {
        // Aliasless: the specifier alone identifies @fresh/core
        let json = r#"{"imports": {"x": "jsr:@fresh/core@^2.0.0"}}"#;
        assert_eq!(check(json, &["@fresh/core"]), vec!["@fresh/core"]);
        let other = r#"{"imports": {"x": "jsr:@fresh/plugin-tailwind@1"}}"#;
        assert!(check(other, &["@fresh/core"]).is_empty());
    }
}
