use crate::detection::caches::parsed_file::strip_c_comments;
use crate::detection::caches::ParsedFileCache;
use std::collections::HashSet;
use std::path::Path;

/// Remove `<!-- ... -->` comments (pom.xml, .csproj)
fn strip_xml_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Strip `#` comments outside single- and double-quoted strings.
fn strip_hash_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let mut quote = None;
        let mut escaped = false;
        let mut end = line.len();
        for (index, c) in line.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' && quote.is_some() {
                escaped = true;
                continue;
            }
            match (quote, c) {
                (Some(delimiter), current) if delimiter == current => quote = None,
                (None, '\'' | '"') => quote = Some(c),
                (None, '#') => {
                    end = index;
                    break;
                }
                _ => {}
            }
        }
        out.push_str(&line[..end]);
        out.push('\n');
    }
    out
}

fn lua_long_bracket_level(src: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut level = 0;
    let mut index = 1;
    while bytes.get(index) == Some(&b'=') {
        level += 1;
        index += 1;
    }
    (bytes.get(index) == Some(&b'[')).then_some(level)
}

fn lua_long_bracket_end(src: &str, start: usize, level: usize) -> usize {
    let content_start = start + level + 2;
    let closing = format!("]{}]", "=".repeat(level));
    src[content_start..]
        .find(&closing)
        .map_or(src.len(), |end| content_start + end + closing.len())
}

fn strip_lua_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    while index < src.len() {
        let Some(c) = src[index..].chars().next() else {
            break;
        };
        if let Some(delimiter) = quote {
            out.push(c);
            index += c.len_utf8();
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == delimiter {
                quote = None;
            }
            continue;
        }
        if matches!(c, '\'' | '"') {
            quote = Some(c);
            out.push(c);
            index += c.len_utf8();
            continue;
        }
        if src[index..].starts_with("--") {
            let after_open = index + 2;
            if let Some(level) = lua_long_bracket_level(&src[after_open..]) {
                index = lua_long_bracket_end(src, after_open, level);
            } else {
                index = after_open;
                while index < src.len() {
                    let Some(c) = src[index..].chars().next() else {
                        break;
                    };
                    if c == '\n' {
                        break;
                    }
                    index += c.len_utf8();
                }
            }
            continue;
        }
        if c == '[' {
            if let Some(level) = lua_long_bracket_level(&src[index..]) {
                let end = lua_long_bracket_end(src, index, level);
                out.push_str(&src[index..end]);
                index = end;
                continue;
            }
        }
        out.push(c);
        index += c.len_utf8();
    }
    out
}

pub fn check_toml_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> bool {
    // The manifest itself, Cargo workspaces ([workspace.dependencies]) and
    // platform tables ([target.'cfg(unix)'.dependencies])
    let platforms = toml_value
        .get("target")
        .and_then(|v| v.as_table())
        .into_iter()
        .flat_map(|targets| targets.values());
    [Some(toml_value), toml_value.get("workspace")]
        .into_iter()
        .flatten()
        .chain(platforms)
        .any(|parent| {
            ["dependencies", "dev-dependencies", "build-dependencies"]
                .iter()
                .filter_map(|section| parent.get(section).and_then(|v| v.as_table()))
                .any(|table| table_names_dependency(table, dep_names))
        })
}

/// A dependency table lists one of `dep_names`, directly or through
/// `web = { package = "axum" }`, which renames the real package
fn table_names_dependency(table: &toml::Table, dep_names: &[String]) -> bool {
    dep_names.iter().any(|dep| table.contains_key(dep))
        || table.values().any(|spec| {
            matches!(spec.get("package"), Some(toml::Value::String(real)) if dep_names.contains(real))
        })
}

pub fn check_json_dependencies(json_value: &serde_json::Value, dep_names: &[String]) -> bool {
    const SECTIONS: [&str; 6] = [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
        "require",
        "require-dev",
    ];
    SECTIONS
        .iter()
        .filter_map(|section| json_value.get(section).and_then(|v| v.as_object()))
        .any(|deps| dep_names.iter().any(|dep| deps.contains_key(dep)))
}

pub fn check_pyproject_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> bool {
    dep_names
        .iter()
        .any(|dep| has_pyproject_dependency(toml_value, dep))
}

/// PEP 503 name normalisation: case-insensitive, `_` `.` `-` equivalent.
fn normalize_py_name(name: &str) -> String {
    name.to_ascii_lowercase().replace(['_', '.'], "-")
}

/// The requirement's package name when `line` (a PEP 508 requirement or a
/// Pipfile `name = ...` entry) names `dep_name`. The name is the leading run
/// of name characters and must END at a requirement boundary — end of line,
/// extras, a version specifier, a marker, or whitespace — so look-alikes
/// (`django-extensions`) never match.
fn python_requirement_names(line: &str, dep_name: &str) -> bool {
    // A UTF-8 BOM can prefix the first line of a file
    let line = line.trim_start_matches('\u{feff}').trim();
    let end = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || "._-".contains(c)))
        .unwrap_or(line.len());
    let (name, rest) = line.split_at(end);
    let boundary = rest
        .trim_start()
        .chars()
        .next()
        .is_none_or(|c| "[=<>!~;@(#".contains(c));
    !name.is_empty() && boundary && normalize_py_name(name) == normalize_py_name(dep_name)
}

/// Checks a single `project.dependencies`/`optional-dependencies`/
/// `dependency-groups` array for a requirement naming `dep_name`.
fn deps_array_matches(deps: &toml::Value, dep_name: &str) -> bool {
    let Some(deps_array) = deps.as_array() else {
        return false;
    };

    deps_array.iter().any(|dep| {
        dep.as_str()
            .is_some_and(|d| python_requirement_names(d, dep_name))
    })
}

/// requirements.txt / Pipfile matching with Python name semantics
pub fn check_python_text_dependencies(content: &str, dep_names: &[String]) -> bool {
    dep_names.iter().any(|dep| {
        content.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && !line.starts_with('-') && python_requirement_names(line, dep)
        })
    })
}

/// The values of a TOML table, or nothing when `value` is absent or not a table
fn table_values(value: Option<&toml::Value>) -> impl Iterator<Item = &toml::Value> {
    value
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|table| table.values())
}

fn has_pyproject_dependency(toml_value: &toml::Value, dep_name: &str) -> bool {
    let is_dep = |k: &String| normalize_py_name(k) == normalize_py_name(dep_name);
    let lists_it = |deps: &toml::Value| deps_array_matches(deps, dep_name);
    let names_it = |table: Option<&toml::Value>| {
        table
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.keys().any(is_dep))
    };

    let project = toml_value.get("project");
    let tool = toml_value.get("tool");
    // PEP 621: [project].dependencies and [project.optional-dependencies.<extra>]
    project.and_then(|p| p.get("dependencies")).is_some_and(lists_it)
        || table_values(project.and_then(|p| p.get("optional-dependencies"))).any(lists_it)
        // PEP 735: [dependency-groups]
        || table_values(toml_value.get("dependency-groups")).any(lists_it)
        || ["poetry", "pdm", "uv"]
            .iter()
            .filter_map(|name| tool?.get(name))
            .any(|section| {
            // [tool.poetry.dependencies]
            names_it(section.get("dependencies"))
                // [tool.poetry.group.<g>.dependencies]
                || table_values(section.get("group")).any(|g| names_it(g.get("dependencies")))
                // dev-dependencies: poetry's legacy table of names, pdm's
                // table of requirement arrays, uv's plain array
                || section.get("dev-dependencies").is_some_and(|dev| {
                    lists_it(dev) || names_it(Some(dev)) || table_values(Some(dev)).any(lists_it)
                })
        })
}

/// Check go.mod dependencies. The single-line form puts the `require`
/// keyword before the module path; strip it so module paths anchor at line
/// start like the indented block form, then reuse the text matcher.
pub fn check_go_mod_dependencies(content: &str, dep_names: &[String]) -> bool {
    let mut normalized = String::with_capacity(content.len());
    // `// indirect` requirements are transitive, not declared dependencies
    for line in content.lines().filter(|line| !line.contains("// indirect")) {
        normalized.push_str(
            line.trim_start()
                .strip_prefix("require ")
                .map_or(line, str::trim_start),
        );
        normalized.push('\n');
    }
    check_text_dependencies(&normalized, dep_names)
}

/// Only real `gem 'name'` / `gem("name")` declarations count
pub fn check_gemfile_dependencies(content: &str, gems: &[String]) -> bool {
    content
        .lines()
        .map(str::trim)
        .any(|line| gems.iter().any(|gem| gem_line_names(line, gem)))
}

pub fn check_text_dependencies(content: &str, dep_names: &[String]) -> bool {
    dep_names
        .iter()
        .any(|dep| has_text_dependency(content, dep))
}

fn has_text_dependency(content: &str, dep_name: &str) -> bool {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }

        // requirements.txt style: name anchored at line start, followed by end
        // of line, a version specifier, extras, or an environment marker —
        // avoids matching substrings of other names or mentions in comments
        if let Some(rest) = line.strip_prefix(dep_name) {
            if rest.is_empty() || rest.starts_with(|c: char| "=<>!~[; ".contains(c)) {
                return true;
            }
        }
    }

    false
}

/// `gem 'name'`, `gem "name", ...` or `gem("name")`, with any spacing. The
/// quote must close right after the name (`rails-html-sanitizer` is not `rails`).
fn gem_line_names(line: &str, dep_name: &str) -> bool {
    let Some(rest) = line.strip_prefix("gem") else {
        return false;
    };
    if !rest.starts_with(|c: char| c.is_whitespace() || c == '(') {
        return false;
    }
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '(');
    let Some(quote) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) else {
        return false;
    };
    rest[1..]
        .strip_prefix(dep_name)
        .is_some_and(|after| after.starts_with(quote))
}

pub fn check_package_lock_dependencies(
    json_value: &serde_json::Value,
    dep_names: &[String],
) -> bool {
    if let Some(packages) = json_value.get("packages").and_then(|v| v.as_object()) {
        if dep_names
            .iter()
            .any(|dep| packages.contains_key(&format!("node_modules/{dep}")))
        {
            return true;
        }
    }

    let mut pending = vec![json_value];
    while let Some(value) = pending.pop() {
        let Some(dependencies) = value.get("dependencies").and_then(|v| v.as_object()) else {
            continue;
        };
        for (name, details) in dependencies {
            if dep_names.contains(name) {
                return true;
            }
            pending.push(details);
        }
    }
    false
}

pub fn check_yarn_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
    dep_names.iter().any(|dep| {
        // Entry keys start at column 0, e.g. `react@^18.2.0:` or
        // `"@babel/core@^7.0.0":`; anchoring avoids `react@` in `preact@`
        let (bare, quoted) = (format!("{dep}@"), format!("\"{dep}@"));
        content
            .lines()
            .any(|line| line.starts_with(&bare) || line.starts_with(&quoted))
    })
}

pub fn check_pnpm_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
    // YAML keys like `  react:` or `  '@angular/core':` (scoped names are
    // quoted; older lockfiles prefix `/`). Anchoring to the (indented) line
    // start avoids matching `react:` inside `preact:`
    dep_names.iter().any(|dep| {
        content.lines().any(|line| {
            line.trim_start()
                .trim_start_matches(['\'', '"', '/'])
                .strip_prefix(dep.as_str())
                .is_some_and(|after| after.starts_with([':', '@', '\'', '"']))
        })
    })
}

pub fn check_composer_lock_dependencies(
    json_value: &serde_json::Value,
    package_names: &[String],
) -> bool {
    let Some(packages) = json_value.get("packages").and_then(|v| v.as_array()) else {
        return false;
    };
    let available: HashSet<&str> = packages
        .iter()
        .filter_map(|package| package.get("name").and_then(|n| n.as_str()))
        .collect();
    package_names
        .iter()
        .any(|name| available.contains(name.as_str()))
}

pub fn check_gemfile_lock_dependencies(content: &str, gem_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    let Some(specs_start) = content.find("specs:") else {
        return false;
    };
    let specs = &content[specs_start..];
    // Specs entries are always `    name (version)`; a dash variant would
    // match prefix-named gems (rails- matching rails-html-sanitizer)
    gem_names
        .iter()
        .any(|gem| specs.contains(&format!("    {gem} (")))
}

pub fn check_poetry_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    dep_names.iter().any(|dep| {
        content.contains(&format!("name = \"{dep}\""))
            || content.contains(&format!("name = '{dep}'"))
    })
}

pub fn check_cargo_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    dep_names
        .iter()
        .any(|dep| content.contains(&format!("name = \"{dep}\"")))
}

/// Check build.sbt dependencies: the artifact is the quoted string right
/// after a `%` / `%%` operator (`"group" %% "artifact" % "version"`).
pub fn check_sbt_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_c_comments(content);
    dep_names.iter().any(|dep| {
        ["%", "%%"].iter().any(|op| {
            content.contains(&format!("{op} \"{dep}\""))
                || content.contains(&format!("{op}\"{dep}\""))
        })
    })
}

pub fn check_pubspec_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    dep_names.iter().any(|dep| {
        content.lines().any(|line| {
            line.trim_start()
                .strip_prefix(dep.as_str())
                .is_some_and(|rest| rest.starts_with(':'))
        })
    })
}

/// True when a top-level `dependencies:` / `dev_dependencies:` section has at
/// least one entry, i.e. the manifest is authoritative and the lock (which
/// also lists transitive packages) must not be consulted.
pub fn pubspec_declares_dependencies(content: &str) -> bool {
    let content = strip_hash_comments(content);
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            in_section = matches!(
                trimmed.trim_end_matches(':'),
                "dependencies" | "dev_dependencies"
            ) && trimmed.ends_with(':');
        } else if in_section {
            return true;
        }
    }
    false
}

pub fn check_pubspec_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    let Some(packages_start) = content.find("packages:") else {
        return false;
    };
    let packages = &content[packages_start..];
    dep_names
        .iter()
        .any(|dep| packages.contains(&format!("  {dep}:")))
}

/// Only `<dependency>` and `<parent>` coordinates count: a bare
/// `<artifactId>` under `<project>` names the project itself.
pub fn check_pom_xml_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_xml_comments(content);
    let blocks: Vec<&str> = ["dependency", "parent"]
        .iter()
        .flat_map(|tag| xml_blocks(&content, tag))
        .collect();
    dep_names.iter().any(|dep| {
        blocks.iter().any(|block| {
            block.contains(&format!("<artifactId>{dep}</artifactId>"))
                || block.contains(&format!("<artifactId>{dep}-"))
        })
    })
}

/// The text inside each `<tag>…</tag>` element, in document order
fn xml_blocks<'a>(content: &'a str, tag: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
    let mut blocks = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(&close) else {
            break;
        };
        blocks.push(&after[..end]);
        rest = &after[end + close.len()..];
    }
    blocks
}

pub fn check_gradle_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_c_comments(content);
    dep_names.iter().any(|dep| {
        has_gradle_coordinate(&content, dep)
            || content.contains(&format!("name: '{}'", dep))
            || content.contains(&format!("name: \"{}\"", dep))
    })
}

/// `group:artifact[:version]` with the artifact ending at a version colon, a
/// closing quote (unversioned), or a dash — the dash keeps family-prefix
/// catalog entries (spring-boot → spring-boot-starter-web) matching while
/// rejecting arbitrary substrings of longer artifact names. A quote or
/// bracket before the colon (`project(":ktor-server-core")`) means a project
/// path, not a coordinate.
fn has_gradle_coordinate(content: &str, dep_name: &str) -> bool {
    let needle = format!(":{dep_name}");
    content.match_indices(&needle).any(|(i, _)| {
        let group_end = content[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || "._-".contains(c));
        let artifact_end = content[i + needle.len()..]
            .chars()
            .next()
            .is_some_and(|c| matches!(c, ':' | '\'' | '"' | '-'));
        group_end && artifact_end
    })
}

/// NuGet package ids are case-insensitive.
pub fn check_csproj_dependencies(content: &str, package_names: &[String]) -> bool {
    let content = strip_xml_comments(content).to_lowercase();

    // The Web SDK implicitly references the ASP.NET Core shared framework, so
    // a bare `<Project Sdk="Microsoft.NET.Sdk.Web">` has no PackageReference
    let web_sdk = content.contains("sdk=\"microsoft.net.sdk.web\"")
        || content.contains("sdk='microsoft.net.sdk.web'");

    package_names.iter().any(|package_name| {
        let id = package_name.to_lowercase();
        (web_sdk && package_name.as_str() == "Microsoft.AspNetCore.App")
            || content.contains(&format!("include=\"{id}\""))
            || content.contains(&format!("include='{id}'"))
    })
}

pub fn check_packages_config_dependencies(content: &str, package_names: &[String]) -> bool {
    let content = strip_xml_comments(content).to_lowercase();
    package_names.iter().any(|name| {
        let id = name.to_lowercase();
        content.contains(&format!("id=\"{id}\"")) || content.contains(&format!("id='{id}'"))
    })
}

/// Check Package.swift dependencies (Swift Package Manager). Only
/// `.package(...)` and `.product(...)` declarations count — a target merely
/// NAMED like the dependency is not a dependency.
pub fn check_swift_package_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = &strip_c_comments(content);
    let declarations: Vec<&str> = content
        .match_indices(".package(")
        .chain(content.match_indices(".product("))
        .map(|(i, _)| {
            let rest = &content[i..];
            &rest[..rest.find(')').map_or(rest.len(), |end| end + 1)]
        })
        .collect();

    dep_names.iter().any(|dep| {
        declarations.iter().any(|decl| {
            decl.contains(&format!("/{dep}.git\""))
                || decl.contains(&format!("/{dep}\""))
                || decl.contains(&format!("name: \"{dep}\""))
                || decl.contains(&format!("name: '{dep}'"))
        })
    })
}

/// Check Mill `build.sc` dependencies: `ivy"group::artifact:version"` (or
/// `group:artifact:version`), so the artifact sits between colons.
pub fn check_mill_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_c_comments(content);
    dep_names
        .iter()
        .any(|dep| content.contains(&format!(":{dep}:")))
}

pub fn check_mix_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_hash_comments(content);
    dep_names.iter().any(|dep| {
        // {:phoenix, "~> 1.7"} / {:"phoenix", ...}: the atom must END
        // after the name (not `{:phoenix_pubsub`)
        ["{:", "{:\""].iter().any(|open| {
            let needle = format!("{open}{dep}");
            content.match_indices(&needle).any(|(i, _)| {
                content[i + needle.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
            })
        })
    })
}

/// The body of the rockspec's `dependencies = { ... }` table. Not
/// `test_dependencies` / `build_dependencies`, which end in the same text.
fn rockspec_dependency_table(content: &str) -> Option<&str> {
    const KEY: &str = "dependencies = {";
    content.match_indices(KEY).find_map(|(i, _)| {
        let standalone = content[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let rest = &content[i + KEY.len()..];
        standalone.then(|| rest.find('}').map_or(rest, |end| &rest[..end]))
    })
}

/// True when the rockspec's `dependencies` table lists a package
pub fn rockspec_declares_dependencies(content: &str) -> bool {
    let content = strip_lua_comments(content);
    rockspec_dependency_table(&content).is_some_and(|table| table.contains(['"', '\'']))
}

/// Check .rockspec dependencies. Entries are quoted constraint strings
/// (`"lapis >= 1.0"`): the package is the first word.
pub fn check_rockspec_dependencies(content: &str, package_names: &[String]) -> bool {
    let content = strip_lua_comments(content);
    let Some(table) = rockspec_dependency_table(&content) else {
        return false;
    };
    let mut declared = Vec::new();
    for quote in ['"', '\''] {
        declared.extend(
            table
                .split(quote)
                .skip(1)
                .step_by(2)
                .filter_map(|entry| entry.split_whitespace().next()),
        );
    }
    package_names
        .iter()
        .any(|name| declared.contains(&name.as_str()))
}

pub fn check_luarocks_lock_dependencies(content: &str, package_names: &[String]) -> bool {
    let content = strip_lua_comments(content);
    package_names.iter().any(|name| {
        content.contains(&format!("[\"{name}\"]"))
            || content.contains(&format!("['{name}']"))
            || content.contains(&format!("name = \"{name}\""))
    })
}

/// A text-file dependency check (`check_pom_xml_dependencies`, ...)
pub type TextCheck = fn(&str, &[String]) -> bool;

/// `file` as the evidence when `found`
pub fn hit(file: &str, found: bool) -> Option<String> {
    found.then(|| file.to_owned())
}

/// The first of `probes` that exists in `dir` and names a dependency,
/// returned as the evidence file name
pub fn first_hit(
    cache: &ParsedFileCache,
    dir: &Path,
    probes: &[(&str, TextCheck)],
    dependencies: &[String],
) -> crate::Result<Option<String>> {
    for (file, check) in probes {
        if let Some(content) = cache.get_file_content(dir.join(file))? {
            if check(&content, dependencies) {
                return Ok(Some((*file).to_owned()));
            }
        }
    }
    Ok(None)
}

/// Project files (`*.csproj`, `*.rockspec`) examined per directory. Real
/// directories hold a handful; a hostile one can hold millions, each of which
/// would be read and recorded as a cache input.
pub const MAX_PROJECT_FILES: usize = 32;

/// Sorted names of up to `MAX_PROJECT_FILES` files in `dir` ending in
/// `extension` (`.csproj`): the smallest ones, so the choice does not depend
/// on the order the file system lists them in
pub fn files_with_extension(dir: &Path, extension: &str) -> Vec<String> {
    let names = std::fs::read_dir(dir).into_iter().flatten().flatten();
    let matching = names
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.ends_with(extension));
    crate::performance::smallest(matching, MAX_PROJECT_FILES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_with_extension_keeps_the_smallest_names_up_to_the_bound(
    ) -> Result<(), std::io::Error> {
        let dir = tempfile::tempdir()?;
        for i in 0..(MAX_PROJECT_FILES * 3) {
            std::fs::write(dir.path().join(format!("p{i:04}.csproj")), "")?;
        }
        std::fs::write(dir.path().join("readme.md"), "")?;

        let names = files_with_extension(dir.path(), ".csproj");

        assert_eq!(
            names.len(),
            MAX_PROJECT_FILES,
            "a hostile directory must not be enumerated"
        );
        assert!(names.windows(2).all(|w| w[0] < w[1]), "sorted");
        assert!(names.iter().all(|n| n.ends_with(".csproj")));
        Ok(())
    }

    #[test]
    fn test_mill_ivy_dependencies_are_found() {
        let build = "def ivyDeps = Agg(\n  ivy\"com.typesafe.akka::akka-http:10.5.0\",\n  ivy\"org.scalatest::scalatest::3.2\"\n)";
        assert!(check_mill_dependencies(build, &names(&["akka-http"])));
        assert!(check_mill_dependencies(build, &names(&["scalatest"])));
        assert!(!check_mill_dependencies(build, &names(&["akka-actor"])));
    }

    #[test]
    fn test_mill_commented_dependency_is_ignored() {
        let build = "// ivy\"com.typesafe.akka::akka-http:10\"";
        assert!(!check_mill_dependencies(build, &names(&["akka-http"])));
    }

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn probes_in(
        files: &[(&str, &str)],
        probes: &[(&str, TextCheck)],
        deps: &[&str],
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content)?;
        }
        Ok(first_hit(
            &ParsedFileCache::new(),
            dir.path(),
            probes,
            &names(deps),
        )?)
    }

    #[test]
    fn test_first_hit_missing_files_find_nothing() -> TestResult {
        let probes: [(&str, TextCheck); 1] = [("mix.exs", check_mix_dependencies)];
        assert!(probes_in(&[], &probes, &["phoenix"])?.is_none());
        Ok(())
    }

    #[test]
    fn test_first_hit_empty_dependency_list_finds_nothing() -> TestResult {
        let probes: [(&str, TextCheck); 1] = [("mix.exs", check_mix_dependencies)];
        let files = [("mix.exs", "{:phoenix, \"~> 1.7\"}")];
        assert!(probes_in(&files, &probes, &[])?.is_none());
        Ok(())
    }

    #[test]
    fn test_first_hit_reports_the_file_that_matched() -> TestResult {
        let probes: [(&str, TextCheck); 1] = [("mix.exs", check_mix_dependencies)];
        let files = [("mix.exs", "{:phoenix, \"~> 1.7\"}")];
        let file =
            probes_in(&files, &probes, &["phoenix", "fake_package"])?.ok_or("expected a hit")?;
        assert_eq!(file, "mix.exs");
        Ok(())
    }

    #[test]
    fn test_first_hit_skips_probes_without_hits() -> TestResult {
        let probes: [(&str, TextCheck); 2] = [
            ("build.gradle", check_gradle_dependencies),
            ("build.gradle.kts", check_gradle_dependencies),
        ];
        let files = [
            ("build.gradle", "plugins { id 'java' }"),
            (
                "build.gradle.kts",
                "implementation(\"io.ktor:ktor-server-core:2\")",
            ),
        ];
        let file = probes_in(&files, &probes, &["ktor-server-core"])?.ok_or("expected a hit")?;
        assert_eq!(file, "build.gradle.kts");
        Ok(())
    }

    #[test]
    fn test_pom_commented_dependency_is_ignored() {
        let pom =
            "<!-- <dependency><artifactId>spring-boot-starter-web</artifactId></dependency> -->";
        assert!(!check_pom_xml_dependencies(
            pom,
            &names(&["spring-boot-starter-web"])
        ));
    }

    #[test]
    fn test_pom_real_dependency_next_to_a_comment_is_found() {
        let pom = "<!-- note --><dependency><artifactId>spring-boot-starter-web</artifactId></dependency>";
        assert!(check_pom_xml_dependencies(
            pom,
            &names(&["spring-boot-starter-web"])
        ));
    }

    #[test]
    fn test_gradle_line_comment_is_ignored() {
        let gradle = "// implementation(\"org.springframework.boot:spring-boot-starter-web\")";
        assert!(!check_gradle_dependencies(
            gradle,
            &names(&["spring-boot-starter-web"])
        ));
    }

    #[test]
    fn test_gradle_block_comment_is_ignored() {
        let gradle = "/*\nimplementation 'org.springframework.boot:spring-boot-starter-web'\n*/";
        assert!(!check_gradle_dependencies(
            gradle,
            &names(&["spring-boot-starter-web"])
        ));
    }

    #[test]
    fn test_gradle_dependency_after_a_url_string_is_found() {
        let gradle = "maven { url = uri(\"https://repo.example.com\") }\n\
                      implementation(\"io.ktor:ktor-server-core:2.3\")";
        assert!(check_gradle_dependencies(
            gradle,
            &names(&["ktor-server-core"])
        ));
    }

    #[test]
    fn test_gradle_project_path_is_not_a_dependency() {
        let gradle = "implementation(project(\":ktor-server-core\"))";
        assert!(!check_gradle_dependencies(
            gradle,
            &names(&["ktor-server-core"])
        ));
    }

    #[test]
    fn test_gradle_coordinate_forms_still_match() {
        let dep = names(&["ktor-server-core"]);
        for gradle in [
            "implementation(\"io.ktor:ktor-server-core:2.3\")",
            "implementation 'io.ktor:ktor-server-core'",
            "implementation(\"io.ktor:ktor-server-core\")",
            "implementation group: 'io.ktor', name: 'ktor-server-core'",
        ] {
            assert!(check_gradle_dependencies(gradle, &dep), "{gradle}");
        }
    }

    #[test]
    fn test_swift_commented_package_is_ignored() {
        let swift = "// .package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\"),";
        assert!(!check_swift_package_dependencies(swift, &names(&["vapor"])));
    }

    #[test]
    fn test_csproj_assembly_name_is_not_a_package() {
        let csproj = "<Project><PropertyGroup><AssemblyName>Microsoft.AspNetCore</AssemblyName></PropertyGroup></Project>";
        assert!(!check_csproj_dependencies(
            csproj,
            &names(&["Microsoft.AspNetCore"])
        ));
    }

    #[test]
    fn test_csproj_commented_package_is_ignored() {
        let csproj = "<!-- <PackageReference Include=\"Microsoft.AspNetCore\" /> -->";
        assert!(!check_csproj_dependencies(
            csproj,
            &names(&["Microsoft.AspNetCore"])
        ));
    }

    #[test]
    fn test_sbt_commented_dependency_is_ignored() {
        let sbt = "// libraryDependencies += \"com.typesafe.akka\" %% \"akka-http\" % \"10\"";
        assert!(!check_sbt_dependencies(sbt, &names(&["akka-http"])));
    }

    #[test]
    fn test_cargo_renamed_dependency_matches_the_real_package() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[dependencies]\nweb = { package = \"axum\", version = \"0.7\" }\n")?;
        assert!(check_toml_dependencies(&manifest, &names(&["axum"])));
        Ok(())
    }

    #[test]
    fn test_pep621_optional_dependency_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[project.optional-dependencies]\nweb = [\"django>=4\"]\n")?;
        assert!(check_pyproject_dependencies(&manifest, &names(&["django"])));
        Ok(())
    }

    #[test]
    fn test_poetry_main_dependency_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[tool.poetry.dependencies]\npython = \"^3.12\"\nDjango = \"^5\"\n")?;
        assert!(check_pyproject_dependencies(&manifest, &names(&["django"])));
        Ok(())
    }

    #[test]
    fn test_poetry_legacy_dev_dependency_table_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[tool.poetry.dev-dependencies]\npytest = \"*\"\n")?;
        assert!(check_pyproject_dependencies(&manifest, &names(&["pytest"])));
        Ok(())
    }

    #[test]
    fn test_pyproject_without_the_dependency_does_not_match() -> Result<(), toml::de::Error> {
        let manifest: toml::Value = toml::from_str(
            "[project]\ndependencies = [\"flask\"]\n[tool.poetry.dependencies]\nrequests = \"*\"\n",
        )?;
        assert!(!check_pyproject_dependencies(
            &manifest,
            &names(&["django"])
        ));
        Ok(())
    }

    #[test]
    fn test_poetry_group_dependency_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[tool.poetry.group.dev.dependencies]\ndjango = \"^5\"\n")?;
        assert!(check_pyproject_dependencies(&manifest, &names(&["django"])));
        Ok(())
    }

    #[test]
    fn test_pdm_dev_dependency_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[tool.pdm.dev-dependencies]\ntest = [\"flask>=3\"]\n")?;
        assert!(check_pyproject_dependencies(&manifest, &names(&["flask"])));
        Ok(())
    }

    #[test]
    fn test_uv_dev_dependency_is_found() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[tool.uv]\ndev-dependencies = [\"fastapi>=0.100\"]\n")?;
        assert!(check_pyproject_dependencies(
            &manifest,
            &names(&["fastapi"])
        ));
        Ok(())
    }

    #[test]
    fn test_gem_with_extra_spaces_is_found() {
        assert!(check_gemfile_dependencies(
            "gem   'rails', '~> 7'\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_gem_with_parentheses_is_found() {
        assert!(check_gemfile_dependencies(
            "gem(\"rails\")\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_gem_name_prefix_is_not_a_match() {
        assert!(!check_gemfile_dependencies(
            "gem 'rails-html-sanitizer'\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_gemfile_assignment_is_not_a_gem() {
        assert!(!check_gemfile_dependencies(
            "rails = \"7\"\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_unrelated_tool_table_is_not_python_dependency_evidence() -> Result<(), toml::de::Error>
    {
        let manifest: toml::Value =
            toml::from_str("[tool.example.dependencies]\ndjango = \"*\"\n")?;
        assert!(!check_pyproject_dependencies(
            &manifest,
            &names(&["django"])
        ));
        Ok(())
    }

    #[test]
    fn test_gradle_single_quoted_url_does_not_hide_a_same_line_dependency() {
        let gradle = "maven { url 'https://repo.example.com' }; implementation 'io.ktor:ktor-server-core:2.3'";
        assert!(check_gradle_dependencies(
            gradle,
            &names(&["ktor-server-core"])
        ));
    }

    #[test]
    fn test_nuget_package_id_is_case_insensitive() {
        let csproj = "<PackageReference Include=\"microsoft.aspnetcore.mvc\" Version=\"2\" />";
        assert!(check_csproj_dependencies(
            csproj,
            &names(&["Microsoft.AspNetCore.Mvc"])
        ));

        let packages = "<package id=\"MICROSOFT.ASPNETCORE.MVC\" version=\"2\" />";
        assert!(check_packages_config_dependencies(
            packages,
            &names(&["Microsoft.AspNetCore.Mvc"])
        ));
    }

    #[test]
    fn test_web_sdk_with_single_quotes_is_found() {
        let csproj = "<Project Sdk='Microsoft.NET.Sdk.Web'></Project>";
        assert!(check_csproj_dependencies(
            csproj,
            &names(&["Microsoft.AspNetCore.App"])
        ));
    }

    #[test]
    fn test_pnpm_scoped_package_is_found() {
        let lock = "importers:\n  .:\n    dependencies:\n      '@angular/core':\n        version: 17.0.0\n";
        assert!(check_pnpm_lock_dependencies(
            lock,
            &names(&["@angular/core"])
        ));
    }

    #[test]
    fn test_rockspec_versioned_dependency_is_found() {
        let rockspec = "dependencies = {\n  \"lua >= 5.1\",\n  \"lapis >= 1.0\",\n}\n";
        assert!(check_rockspec_dependencies(rockspec, &names(&["lapis"])));
    }

    #[test]
    fn test_rockspec_test_dependencies_are_not_dependencies() {
        let rockspec =
            "dependencies = {\n  \"lua >= 5.1\",\n}\ntest_dependencies = {\n  \"busted\",\n}\n";
        assert!(!check_rockspec_dependencies(rockspec, &names(&["busted"])));
    }

    #[test]
    fn test_rockspec_build_module_named_like_a_package_is_not_a_dependency() {
        let rockspec = "dependencies = {\n  \"lua >= 5.1\",\n}\nbuild = { modules = { lapis = \"src/lapis.lua\" } }\n";
        assert!(!check_rockspec_dependencies(rockspec, &names(&["lapis"])));
    }

    #[test]
    fn test_pubspec_declares_dependencies() {
        assert!(pubspec_declares_dependencies(
            "name: x\ndependencies:\n  http: ^1\n"
        ));
        assert!(pubspec_declares_dependencies(
            "dev_dependencies:\n\tlints: any\n"
        ));
        // an empty or comment-only section declares nothing
        assert!(!pubspec_declares_dependencies(
            "name: x\ndependencies:\n  # none\n"
        ));
        assert!(!pubspec_declares_dependencies(
            "name: x\nenvironment:\n  sdk: any\n"
        ));
        // an indented key under another section is not a dependency
        assert!(!pubspec_declares_dependencies(
            "flutter:\n  uses-material-design: true\n"
        ));
    }

    #[test]
    fn test_rockspec_declares_dependencies() {
        assert!(rockspec_declares_dependencies(
            "dependencies = {\n  \"lua >= 5.1\",\n}"
        ));
        assert!(rockspec_declares_dependencies("dependencies = { 'lapis' }"));
        assert!(!rockspec_declares_dependencies("dependencies = {\n}"));
        assert!(!rockspec_declares_dependencies("package = \"demo\""));
    }

    #[test]
    fn test_go_mod_matches_single_line_require() {
        // `go mod tidy` writes a lone direct dep as a single-line require
        let content =
            "module example.com/app\n\ngo 1.22\n\nrequire github.com/gin-gonic/gin v1.9.1\n";
        let deps = vec!["github.com/gin-gonic/gin".to_string()];
        assert!(check_go_mod_dependencies(content, &deps));
    }

    #[test]
    fn test_go_mod_matches_require_block() {
        let content = "module x\n\nrequire (\n    github.com/gin-gonic/gin v1.9.1\n)\n";
        let deps = vec!["github.com/gin-gonic/gin".to_string()];
        assert!(check_go_mod_dependencies(content, &deps));
    }

    #[test]
    fn test_text_dependency_does_not_apply_go_mod_grammar() {
        // Only go.mod lines carry a `require` keyword; other text manifests
        // must not treat such lines as dependency declarations
        let content = "# notes\nrequire django\n";
        assert!(!check_text_dependencies(content, &["django".to_string()]));
    }

    #[test]
    fn test_gradle_does_not_match_artifact_name_substring() {
        let content = "dependencies {\n    implementation(\"org.example:ginseng:1.0\")\n}\n";
        assert!(!check_gradle_dependencies(content, &["gin".to_string()]));
    }

    #[test]
    fn test_gradle_matches_artifact_with_terminators() {
        // versioned, family-prefix, and unversioned (quote-terminated) forms
        let content = "dependencies {\n    implementation(\"io.ktor:ktor-server-core:2.3.0\")\n    implementation 'org.springframework.boot:spring-boot-starter-web:3.0'\n    implementation(\"com.squareup.okhttp3:okhttp\")\n}\n";
        let deps = vec![
            "ktor-server-core".to_string(),
            "spring-boot".to_string(),
            "okhttp".to_string(),
        ];
        for dep in &deps {
            assert!(
                check_gradle_dependencies(content, std::slice::from_ref(dep)),
                "{dep}"
            );
        }
    }

    #[test]
    fn test_gemfile_lock_does_not_match_prefix_named_gem() {
        let content = "GEM\n  specs:\n    rails-html-sanitizer (1.4.4)\n";
        assert!(!check_gemfile_lock_dependencies(
            content,
            &["rails".to_string()]
        ));
    }

    #[test]
    fn test_yarn_lock_does_not_match_substring_package() {
        let content = "preact@^10.0.0:\n  version \"10.19.3\"\n\n\"preact-render-to-string@^6.0.0\":\n  version \"6.3.1\"\n";
        let deps = vec!["react".to_string()];
        assert!(!check_yarn_lock_dependencies(content, &deps));
    }

    #[test]
    fn test_yarn_lock_matches_exact_package() {
        let content = "react@^18.2.0:\n  version \"18.2.0\"\n\n\"@babel/core@^7.0.0\":\n  version \"7.23.0\"\n";
        let deps = vec!["react".to_string(), "@babel/core".to_string()];
        for dep in &deps {
            assert!(
                check_yarn_lock_dependencies(content, std::slice::from_ref(dep)),
                "{dep}"
            );
        }
    }

    #[test]
    fn test_pnpm_lock_does_not_match_substring_package() {
        let content = "dependencies:\n  preact: 10.19.3\n";
        let deps = vec!["react".to_string()];
        assert!(!check_pnpm_lock_dependencies(content, &deps));
    }

    #[test]
    fn test_text_dependency_ignores_comments() {
        let content = "# django is not actually used here\nflask==3.0.0\n";
        let deps = vec!["django".to_string()];
        assert!(!check_text_dependencies(content, &deps));
    }

    #[test]
    fn test_text_dependency_requires_name_boundary() {
        // `django-extensions` is a different package than `django`
        let content = "django-extensions==3.2.3\n";
        let deps = vec!["django".to_string()];
        assert!(!check_text_dependencies(content, &deps));
    }

    #[test]
    fn test_text_dependency_matches_specifiers() {
        for line in ["django", "django==4.2", "django>=4", "django[argon2]==4.2"] {
            let deps = vec!["django".to_string()];
            assert!(
                check_text_dependencies(line, &deps),
                "should match line: {line}"
            );
        }
    }

    #[test]
    fn test_poetry_lock_matches_single_quoted_name() {
        let content = "[[package]]\nname = 'django'\nversion = \"4.2\"\n";
        let deps = vec!["django".to_string()];
        assert!(check_poetry_lock_dependencies(content, &deps));
    }

    #[test]
    fn test_python_requirements_are_case_and_separator_insensitive() {
        let content = "Django==4.2\nFlask\nFastAPI[standard]>=0.100\nflask_cors ~= 3.0 ; python_version > '3'\n";
        assert!(check_python_text_dependencies(
            content,
            &names(&["django", "flask", "fastapi", "flask-cors"])
        ));
    }

    #[test]
    fn test_python_requirements_reject_lookalikes_and_comments() {
        let content = "django-extensions==3.0\n# flask\n-r base.txt\nflasky==1\n";
        assert!(!check_python_text_dependencies(
            content,
            &names(&["django", "flask"])
        ));
    }

    #[test]
    fn test_pyproject_extras_spaces_case_and_dependency_groups() {
        let toml: toml::Value = toml::from_str(
            "[project]\ndependencies = [\"fastapi[standard]>=0.100\", \"Django>=4\", \"flask ~= 3.0\"]\n\n[dependency-groups]\ndev = [\"pytest>=8\"]\n",
        )
        .unwrap_or_else(|e| panic!("toml: {e}"));
        for dep in ["fastapi", "django", "flask", "pytest"] {
            assert!(check_pyproject_dependencies(&toml, &names(&[dep])), "{dep}");
        }
        assert!(!check_pyproject_dependencies(&toml, &names(&["fast"])));
    }

    #[test]
    fn test_mix_rejects_dep_name_prefixes() {
        let content = "defp deps do\n  [{:phoenix_pubsub, \"~> 2.0\"}, {:ecto, \"~> 3\"}]\nend";
        assert!(!check_mix_dependencies(content, &names(&["phoenix"])));
        let real = "[{:phoenix, \"~> 1.7\"}]";
        assert!(check_mix_dependencies(real, &names(&["phoenix"])));
    }

    #[test]
    fn test_hash_commented_dependencies_are_ignored() {
        let pubspec = "dependencies:\n  #  flutter:\n  http: ^1 #  flutter:\n";
        assert!(!check_pubspec_dependencies(pubspec, &names(&["flutter"])));
        assert!(check_pubspec_dependencies(
            "dependencies:\n  flutter: any\n",
            &names(&["flutter"])
        ));

        let mix = "# {:phoenix, \"~> 1.7\"}\nIO.puts(:ok) # {:phoenix, \"~> 1.7\"}";
        assert!(!check_mix_dependencies(mix, &names(&["phoenix"])));
    }

    #[test]
    fn test_commented_sections_are_ignored_across_text_manifests() {
        let packages_config = r#"<!-- <package id="Microsoft.AspNetCore.Mvc" version="2" /> -->"#;
        assert!(!check_packages_config_dependencies(
            packages_config,
            &names(&["Microsoft.AspNetCore.Mvc"])
        ));

        let gemfile_lock = "GEM\n  specs:\n    #    rails (7.0)\n";
        assert!(!check_gemfile_lock_dependencies(
            gemfile_lock,
            &names(&["rails"])
        ));
        assert!(!check_poetry_lock_dependencies(
            "# name = \"django\"",
            &names(&["django"])
        ));
        assert!(!check_cargo_lock_dependencies(
            "# name = \"axum\"",
            &names(&["axum"])
        ));
        assert!(!check_pubspec_lock_dependencies(
            "packages:\n  #  flutter:\n",
            &names(&["flutter"])
        ));
        assert!(pubspec_declares_dependencies(
            "dependencies: # direct\n  http: ^1\n"
        ));

        let rockspec = r#"dependencies = {
-- "lapis >= 1.0"
--[=[
"lapis >= 1.0"
]=]
}
"#;
        assert!(!check_rockspec_dependencies(rockspec, &names(&["lapis"])));
        assert!(!rockspec_declares_dependencies(rockspec));

        let luarocks_lock = r#"--[[
return { dependencies = { ["lapis"] = "1.0" } }
]]"#;
        assert!(!check_luarocks_lock_dependencies(
            luarocks_lock,
            &names(&["lapis"])
        ));
    }

    #[test]
    fn test_swift_only_package_declarations_count() {
        let target_only =
            "let package = Package(name: \"app\", targets: [.executableTarget(name: \"vapor\")])";
        assert!(!check_swift_package_dependencies(
            target_only,
            &names(&["vapor"])
        ));
        for real in [
            ".package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\")",
            ".package(url: \"https://github.com/vapor/vapor\", from: \"4.0.0\")",
        ] {
            assert!(check_swift_package_dependencies(real, &names(&["vapor"])));
        }
    }

    #[test]
    fn test_pom_own_artifact_id_is_not_a_dependency() {
        // the project's own coordinates sit directly under <project>
        let pom = "<project><artifactId>spring-boot-starter-notes</artifactId></project>";
        assert!(!check_pom_xml_dependencies(
            pom,
            &names(&["spring-boot-starter"])
        ));
    }

    #[test]
    fn test_pom_parent_artifact_id_counts() {
        // spring-boot-starter-parent is how most Spring Boot projects say so
        let pom = "<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent></project>";
        assert!(check_pom_xml_dependencies(
            pom,
            &names(&["spring-boot-starter"])
        ));
    }

    #[test]
    fn test_pom_ignores_non_artifact_element_text() {
        let name_only = "<project><name>spring-boot-starter</name></project>";
        assert!(!check_pom_xml_dependencies(
            name_only,
            &names(&["spring-boot-starter"])
        ));
        let real = "<dependency><artifactId>spring-boot-starter</artifactId></dependency>";
        assert!(check_pom_xml_dependencies(
            real,
            &names(&["spring-boot-starter"])
        ));
    }

    #[test]
    fn test_sbt_matches_artifact_position_only() {
        let content =
            "libraryDependencies += \"com.example\" %% \"play\" % \"2.9\"\nname := \"akka-http\"";
        assert!(check_sbt_dependencies(content, &names(&["play"])));
        assert!(!check_sbt_dependencies(content, &names(&["akka-http"])));
    }

    #[test]
    fn test_go_mod_ignores_indirect_requirements() {
        let content = "module x\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1 // indirect\n\tgithub.com/labstack/echo/v4 v4.11.0\n)\n";
        assert!(!check_go_mod_dependencies(
            content,
            &names(&["github.com/gin-gonic/gin"])
        ));
        assert!(check_go_mod_dependencies(
            content,
            &names(&["github.com/labstack/echo/v4"])
        ));
    }

    #[test]
    fn test_gradle_ignores_bare_quoted_strings() {
        let content =
            "rootProject.name = \"gin\"\ndependencies { implementation(\"io.gin:gin:1.0\") }";
        // the project name alone must not count; the real coordinate does
        assert!(!check_gradle_dependencies(
            "rootProject.name = \"gin\"",
            &names(&["gin"])
        ));
        assert!(check_gradle_dependencies(content, &names(&["gin"])));
    }

    #[test]
    fn test_toml_workspace_and_target_dependencies() {
        let toml: toml::Value = toml::from_str(
            "[workspace.dependencies]\naxum = \"0.7\"\n\n[target.'cfg(unix)'.dependencies]\nrocket = \"0.5\"\n\n[workspace.dev-dependencies]\nactix-web = \"4\"\n",
        )
        .unwrap_or_else(|e| panic!("toml: {e}"));
        for dep in ["axum", "rocket", "actix-web"] {
            assert!(check_toml_dependencies(&toml, &names(&[dep])), "{dep}");
        }
        assert!(!check_toml_dependencies(&toml, &names(&["tonic"])));
    }

    #[test]
    fn test_json_optional_dependencies() {
        let json: serde_json::Value = serde_json::json!({"optionalDependencies": {"react": "18"}});
        assert!(check_json_dependencies(&json, &names(&["react"])));
    }

    #[test]
    fn test_python_requirements_inline_comments_and_bom() {
        let content = "\u{feff}django  # web framework\nflask==3.0  # micro\nrequests\t# http\n";
        assert!(check_python_text_dependencies(
            content,
            &names(&["django", "flask", "requests"])
        ));
        // a comment is not a dependency
        assert!(!check_python_text_dependencies(
            "# django\n",
            &names(&["django"])
        ));
    }
}
