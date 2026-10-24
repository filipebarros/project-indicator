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

pub fn check_toml_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> bool {
    let mut available_deps: HashSet<&String> = HashSet::new();

    add_dependency_sections(toml_value, &mut available_deps);
    // Cargo workspaces: [workspace.dependencies] / [workspace.dev-dependencies]
    if let Some(workspace) = toml_value.get("workspace") {
        add_dependency_sections(workspace, &mut available_deps);
    }
    // Platform-specific: [target.'cfg(unix)'.dependencies]
    if let Some(targets) = toml_value.get("target").and_then(|v| v.as_table()) {
        for target in targets.values() {
            add_dependency_sections(target, &mut available_deps);
        }
    }

    dep_names.iter().any(|dep| available_deps.contains(dep))
}

fn add_dependency_sections<'a>(parent: &'a toml::Value, into: &mut HashSet<&'a String>) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = parent.get(section).and_then(|v| v.as_table()) {
            for (name, spec) in table {
                into.insert(name);
                // `web = { package = "axum" }` renames the real package
                if let Some(toml::Value::String(real)) = spec.get("package") {
                    into.insert(real);
                }
            }
        }
    }
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
    let available: HashSet<&String> = SECTIONS
        .iter()
        .filter_map(|section| json_value.get(section).and_then(|v| v.as_object()))
        .flat_map(|deps| deps.keys())
        .collect();

    dep_names.iter().any(|dep| available.contains(dep))
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

fn has_pyproject_dependency(toml_value: &toml::Value, dep_name: &str) -> bool {
    if let Some(project) = toml_value.get("project") {
        if let Some(deps) = project.get("dependencies") {
            if deps_array_matches(deps, dep_name) {
                return true;
            }
        }

        if let Some(optional_deps) = project.get("optional-dependencies") {
            if let Some(optional_deps_table) = optional_deps.as_table() {
                for deps in optional_deps_table.values() {
                    if deps_array_matches(deps, dep_name) {
                        return true;
                    }
                }
            }
        }
    }

    if let Some(groups) = toml_value
        .get("dependency-groups")
        .and_then(|g| g.as_table())
    {
        if groups
            .values()
            .any(|deps| deps_array_matches(deps, dep_name))
        {
            return true;
        }
    }

    if let Some(tool) = toml_value.get("tool") {
        if let Some(tool_table) = tool.as_table() {
            let is_dep = |k: &String| normalize_py_name(k) == normalize_py_name(dep_name);
            for (_, tool_section) in tool_table {
                // [tool.poetry.dependencies]
                if let Some(deps_table) =
                    tool_section.get("dependencies").and_then(|d| d.as_table())
                {
                    if deps_table.keys().any(is_dep) {
                        return true;
                    }
                }
                // [tool.poetry.group.<g>.dependencies]
                if let Some(groups) = tool_section.get("group").and_then(|g| g.as_table()) {
                    if groups.values().any(|group| {
                        group
                            .get("dependencies")
                            .and_then(|d| d.as_table())
                            .is_some_and(|deps| deps.keys().any(is_dep))
                    }) {
                        return true;
                    }
                }
                // dev-dependencies: poetry's legacy table of names, pdm's
                // table of requirement arrays, uv's plain array
                if let Some(dev) = tool_section.get("dev-dependencies") {
                    if deps_array_matches(dev, dep_name) {
                        return true;
                    }
                    if let Some(table) = dev.as_table() {
                        if table.keys().any(is_dep)
                            || table.values().any(|v| deps_array_matches(v, dep_name))
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Check go.mod dependencies. The single-line form puts the `require`
/// keyword before the module path; strip it so module paths anchor at line
/// start like the indented block form, then reuse the text matcher.
pub fn check_go_mod_dependencies(content: &str, dep_names: &[String]) -> bool {
    let normalized: String = content
        .lines()
        // `// indirect` requirements are transitive, not declared dependencies
        .filter(|line| !line.contains("// indirect"))
        .map(|line| {
            line.trim_start()
                .strip_prefix("require ")
                .map_or(line, str::trim_start)
        })
        .collect::<Vec<_>>()
        .join("\n");
    check_text_dependencies(&normalized, dep_names)
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

        if gem_line_names(line, dep_name) {
            return true;
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
    let Some(packages) = json_value.get("packages").and_then(|v| v.as_object()) else {
        return false;
    };
    dep_names
        .iter()
        .any(|dep| packages.contains_key(&format!("node_modules/{dep}")))
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
    dep_names.iter().any(|dep| {
        content.contains(&format!("name = \"{dep}\""))
            || content.contains(&format!("name = '{dep}'"))
    })
}

pub fn check_cargo_lock_dependencies(content: &str, dep_names: &[String]) -> bool {
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
    dep_names
        .iter()
        .any(|dep| content.contains(&format!("  {dep}:")) || content.contains(&format!("\n{dep}:")))
}

/// True when a top-level `dependencies:` / `dev_dependencies:` section has at
/// least one entry, i.e. the manifest is authoritative and the lock (which
/// also lists transitive packages) must not be consulted.
pub fn pubspec_declares_dependencies(content: &str) -> bool {
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
    let Some(packages_start) = content.find("packages:") else {
        return false;
    };
    let packages = &content[packages_start..];
    dep_names
        .iter()
        .any(|dep| packages.contains(&format!("  {dep}:")))
}

pub fn check_pom_xml_dependencies(content: &str, dep_names: &[String]) -> bool {
    let content = strip_xml_comments(content);
    dep_names.iter().any(|dep| {
        content.contains(&format!("<artifactId>{dep}</artifactId>"))
            || content.contains(&format!("<artifactId>{dep}-"))
    })
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
    package_names.iter().any(|name| {
        content.contains(&format!("id=\"{name}\"")) || content.contains(&format!("id='{name}'"))
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
    rockspec_dependency_table(content).is_some_and(|table| table.contains(['"', '\'']))
}

/// Check .rockspec dependencies. Entries are quoted constraint strings
/// (`"lapis >= 1.0"`): the package is the first word.
pub fn check_rockspec_dependencies(content: &str, package_names: &[String]) -> bool {
    let Some(table) = rockspec_dependency_table(content) else {
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

/// Sorted names of the files in `dir` ending in `extension` (`.csproj`)
pub fn files_with_extension(dir: &Path, extension: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.ends_with(extension))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // ---- false positives: comments and look-alikes ----

    #[test]
    fn test_pom_commented_dependency_is_ignored() {
        let pom = "<!-- <artifactId>spring-boot-starter-web</artifactId> -->";
        assert!(!check_pom_xml_dependencies(
            pom,
            &names(&["spring-boot-starter-web"])
        ));
    }

    #[test]
    fn test_pom_real_dependency_next_to_a_comment_is_found() {
        let pom = "<!-- note --><artifactId>spring-boot-starter-web</artifactId>";
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

    // ---- false negatives ----

    #[test]
    fn test_cargo_renamed_dependency_matches_the_real_package() -> Result<(), toml::de::Error> {
        let manifest: toml::Value =
            toml::from_str("[dependencies]\nweb = { package = \"axum\", version = \"0.7\" }\n")?;
        assert!(check_toml_dependencies(&manifest, &names(&["axum"])));
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
        assert!(check_text_dependencies(
            "gem   'rails', '~> 7'\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_gem_with_parentheses_is_found() {
        assert!(check_text_dependencies(
            "gem(\"rails\")\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_gem_name_prefix_is_not_a_match() {
        assert!(!check_text_dependencies(
            "gem 'rails-html-sanitizer'\n",
            &names(&["rails"])
        ));
    }

    #[test]
    fn test_nuget_package_id_is_case_insensitive() {
        let csproj = "<PackageReference Include=\"microsoft.aspnetcore.mvc\" Version=\"2\" />";
        assert!(check_csproj_dependencies(
            csproj,
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
    fn test_gemfile_lock_matches_exact_gem() {
        let content = "GEM\n  specs:\n    rails (7.0.4)\n";
        assert!(check_gemfile_lock_dependencies(
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
    fn test_pnpm_lock_matches_exact_package() {
        let content = "dependencies:\n  react: 18.2.0\n";
        let deps = vec!["react".to_string()];
        assert!(check_pnpm_lock_dependencies(content, &deps));
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
    fn test_text_dependency_matches_gemfile() {
        let content = "source 'https://rubygems.org'\ngem 'rails', '~> 7.1'\n";
        let deps = vec!["rails".to_string()];
        assert!(check_text_dependencies(content, &deps));
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
