use std::collections::HashSet;

/// Check TOML dependencies (Cargo.toml, pyproject.toml)
pub fn check_toml_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> Vec<String> {
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

    dep_names
        .iter()
        .filter(|dep| available_deps.contains(dep))
        .cloned()
        .collect()
}

fn add_dependency_sections<'a>(parent: &'a toml::Value, into: &mut HashSet<&'a String>) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = parent.get(section).and_then(|v| v.as_table()) {
            into.extend(table.keys());
        }
    }
}

/// Check JSON dependencies (package.json, composer.json)
pub fn check_json_dependencies(
    json_value: &serde_json::Value,
    dep_names: &[String],
) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    let mut available_deps = HashSet::new();

    if let Some(deps) = json_value.get("dependencies").and_then(|v| v.as_object()) {
        available_deps.extend(deps.keys());
    }
    if let Some(dev_deps) = json_value
        .get("devDependencies")
        .and_then(|v| v.as_object())
    {
        available_deps.extend(dev_deps.keys());
    }
    if let Some(optional_deps) = json_value
        .get("optionalDependencies")
        .and_then(|v| v.as_object())
    {
        available_deps.extend(optional_deps.keys());
    }
    if let Some(peer_deps) = json_value
        .get("peerDependencies")
        .and_then(|v| v.as_object())
    {
        available_deps.extend(peer_deps.keys());
    }
    if let Some(require) = json_value.get("require").and_then(|v| v.as_object()) {
        available_deps.extend(require.keys());
    }
    if let Some(require_dev) = json_value.get("require-dev").and_then(|v| v.as_object()) {
        available_deps.extend(require_dev.keys());
    }

    for dep_name in dep_names {
        if available_deps.contains(dep_name) {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check Python pyproject.toml dependencies
pub fn check_pyproject_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::new();

    for dep_name in dep_names {
        if has_pyproject_dependency(toml_value, dep_name) {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
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
pub fn check_python_text_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    dep_names
        .iter()
        .filter(|dep| {
            content.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#')
                    && !line.starts_with('-')
                    && python_requirement_names(line, dep)
            })
        })
        .cloned()
        .collect()
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
            for (_, tool_section) in tool_table {
                if let Some(deps) = tool_section.get("dependencies") {
                    if let Some(deps_table) = deps.as_table() {
                        if deps_table
                            .keys()
                            .any(|k| normalize_py_name(k) == normalize_py_name(dep_name))
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
pub fn check_go_mod_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
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

/// Check text-based dependencies (Gemfile, requirements.txt)
pub fn check_text_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    if content.is_empty() {
        return found_deps;
    }

    for dep_name in dep_names {
        if has_text_dependency(content, dep_name) {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

fn has_text_dependency(content: &str, dep_name: &str) -> bool {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }

        if line.contains(&format!("gem '{}'", dep_name))
            || line.contains(&format!("gem \"{}\"", dep_name))
        {
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

/// Check package-lock.json dependencies
pub fn check_package_lock_dependencies(
    json_value: &serde_json::Value,
    dep_names: &[String],
) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    if let Some(packages) = json_value.get("packages").and_then(|v| v.as_object()) {
        let mut available_deps = HashSet::new();

        for (key, _) in packages {
            if key.starts_with("node_modules/") {
                let package_name = key.strip_prefix("node_modules/").unwrap_or(key);
                available_deps.insert(package_name);
            }
        }

        for dep_name in dep_names {
            if available_deps.contains(dep_name.as_str()) {
                found_deps.push(dep_name.to_string());
            }
        }
    }

    found_deps
}

/// Check yarn.lock dependencies
pub fn check_yarn_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    if content.is_empty() {
        return found_deps;
    }

    for dep_name in dep_names {
        // Entry keys start at column 0, e.g. `react@^18.2.0:` or `"@babel/core@^7.0.0":`
        // Anchoring to the line start avoids matching `react@` inside `preact@`
        let bare = format!("{}@", dep_name);
        let quoted = format!("\"{}@", dep_name);
        if content
            .lines()
            .any(|line| line.starts_with(&bare) || line.starts_with(&quoted))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check pnpm-lock.yaml dependencies
pub fn check_pnpm_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    if content.is_empty() {
        return found_deps;
    }

    for dep_name in dep_names {
        // YAML keys like `  react:` — anchoring to the (indented) line start
        // avoids matching `react:` inside `preact:`
        let key = format!("{}:", dep_name);
        if content
            .lines()
            .any(|line| line.trim_start().starts_with(&key))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check composer.lock dependencies
pub fn check_composer_lock_dependencies(
    json_value: &serde_json::Value,
    package_names: &[String],
) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(package_names.len());

    if let Some(packages) = json_value.get("packages").and_then(|v| v.as_array()) {
        let mut available_packages = HashSet::new();

        for package in packages {
            if let Some(name) = package.get("name").and_then(|n| n.as_str()) {
                available_packages.insert(name);
            }
        }

        for package_name in package_names {
            if available_packages.contains(package_name.as_str()) {
                found_deps.push(package_name.to_string());
            }
        }
    }

    found_deps
}

/// Check Gemfile.lock dependencies
pub fn check_gemfile_lock_dependencies(content: &str, gem_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(gem_names.len());

    let specs_section = content.find("specs:");
    if let Some(specs_start) = specs_section {
        let specs_content = &content[specs_start..];

        // Specs entries are always `    name (version)`; a dash variant would
        // match prefix-named gems (rails- matching rails-html-sanitizer)
        for gem_name in gem_names {
            if specs_content.contains(&format!("    {} (", gem_name)) {
                found_deps.push(gem_name.to_string());
            }
        }
    }

    found_deps
}

/// Check poetry.lock dependencies
pub fn check_poetry_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        if content.contains(&format!("name = \"{}\"", dep_name))
            || content.contains(&format!("name = '{}'", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check Cargo.lock dependencies
pub fn check_cargo_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        if content.contains(&format!("name = \"{}\"", dep_name)) {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check build.sbt dependencies: the artifact is the quoted string right
/// after a `%` / `%%` operator (`"group" %% "artifact" % "version"`).
pub fn check_sbt_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    dep_names
        .iter()
        .filter(|dep| {
            ["%", "%%"].iter().any(|op| {
                content.contains(&format!("{op} \"{dep}\""))
                    || content.contains(&format!("{op}\"{dep}\""))
            })
        })
        .cloned()
        .collect()
}

/// Check pubspec.yaml dependencies
pub fn check_pubspec_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        if content.contains(&format!("  {}:", dep_name))
            || content.contains(&format!("  {}: ", dep_name))
            || content.contains(&format!("\n{}:", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check pubspec.lock dependencies
pub fn check_pubspec_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    if let Some(packages_start) = content.find("packages:") {
        let packages_section = &content[packages_start..];

        for dep_name in dep_names {
            if packages_section.contains(&format!("  {}:", dep_name)) {
                found_deps.push(dep_name.to_string());
            }
        }
    }

    found_deps
}

/// Check pom.xml dependencies
pub fn check_pom_xml_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        if content.contains(&format!("<artifactId>{}</artifactId>", dep_name))
            || content.contains(&format!("<artifactId>{}-", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check build.gradle dependencies
pub fn check_gradle_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        // After `group:artifact`, the name ends at a version colon, a closing
        // quote (unversioned), or a dash — the dash keeps family-prefix
        // catalog entries (spring-boot → spring-boot-starter-web) matching
        // while rejecting arbitrary substrings of longer artifact names
        if content.contains(&format!(":{}:", dep_name))
            || content.contains(&format!(":{}'", dep_name))
            || content.contains(&format!(":{}\"", dep_name))
            || content.contains(&format!(":{}-", dep_name))
            || content.contains(&format!("name: '{}'", dep_name))
            || content.contains(&format!("name: \"{}\"", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check .csproj dependencies
pub fn check_csproj_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(package_names.len());

    // The Web SDK implicitly references the ASP.NET Core shared framework, so
    // a bare `<Project Sdk="Microsoft.NET.Sdk.Web">` has no PackageReference
    let web_sdk = content.contains("Sdk=\"Microsoft.NET.Sdk.Web\"");

    for package_name in package_names {
        if (web_sdk && package_name == "Microsoft.AspNetCore.App")
            || content.contains(&format!("Include=\"{}\"", package_name))
            || content.contains(&format!("Include='{}'", package_name))
            || content.contains(&format!(">{}<", package_name))
        {
            found_deps.push(package_name.to_string());
        }
    }

    found_deps
}

/// Check packages.config dependencies
pub fn check_packages_config_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(package_names.len());

    for package_name in package_names {
        if content.contains(&format!("id=\"{}\"", package_name))
            || content.contains(&format!("id='{}'", package_name))
        {
            found_deps.push(package_name.to_string());
        }
    }

    found_deps
}

/// Check Package.swift dependencies (Swift Package Manager). Only
/// `.package(...)` and `.product(...)` declarations count — a target merely
/// NAMED like the dependency is not a dependency.
pub fn check_swift_package_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let declarations: Vec<&str> = content
        .match_indices(".package(")
        .chain(content.match_indices(".product("))
        .map(|(i, _)| {
            let rest = &content[i..];
            &rest[..rest.find(')').map_or(rest.len(), |end| end + 1)]
        })
        .collect();

    dep_names
        .iter()
        .filter(|dep| {
            declarations.iter().any(|decl| {
                decl.contains(&format!("/{dep}.git\""))
                    || decl.contains(&format!("/{dep}\""))
                    || decl.contains(&format!("name: \"{dep}\""))
                    || decl.contains(&format!("name: '{dep}'"))
            })
        })
        .cloned()
        .collect()
}

/// Check mix.exs dependencies (Elixir)
pub fn check_mix_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    dep_names
        .iter()
        .filter(|dep| {
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
        .cloned()
        .collect()
}

/// Check .rockspec dependencies
pub fn check_rockspec_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(package_names.len());

    for package_name in package_names {
        if content.contains("dependencies = {")
            && (content.contains(&format!("\"{}\"", package_name))
                || content.contains(&format!("'{}'", package_name))
                || content.contains(&format!("{} =", package_name)))
        {
            found_deps.push(package_name.to_string());
        }
    }

    found_deps
}

/// Check luarocks.lock dependencies
pub fn check_luarocks_lock_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(package_names.len());

    for package_name in package_names {
        if content.contains(&format!("[\"{}\"]", package_name))
            || content.contains(&format!("['{}']", package_name))
            || content.contains(&format!("name = \"{}\"", package_name))
        {
            found_deps.push(package_name.to_string());
        }
    }

    found_deps
}

/// Generic helper to check and collect dependencies from a config file
///
/// This reduces duplication across ecosystem matchers by providing a common pattern:
/// 1. Try to get config file from the cache
/// 2. Check for dependencies in that file using the provided check function
/// 3. If found, collect them and add the filename to evidence
/// 4. Return true to indicate early termination
///
/// # Parameters
/// - `parsed_cache`: Cache for parsed config files
/// - `path`: Directory path to search in
/// - `file_name`: Name of the config file to check
/// - `dependencies`: List of dependency names to search for
/// - `check_fn`: Function that checks file content for dependencies
/// - `found_deps`: Accumulator for found dependency names
/// - `evidence`: Accumulator for evidence file names
///
/// # Returns
/// - `Ok(true)` if dependencies were found and added
/// - `Ok(false)` if file doesn't exist or no dependencies found
///
/// # Example
/// Within an ecosystem matcher, you would use this as:
/// ```text
/// if try_config_file_deps(
///     parsed_cache,
///     &path,
///     BUILD_GRADLE,
///     dependencies,
///     check_gradle_dependencies,
///     &mut found_deps,
///     &mut evidence,
/// )? {
///     return Ok((found_deps, evidence));
/// }
/// ```
pub fn try_config_file_deps<P, F>(
    parsed_cache: &crate::detection::caches::ParsedFileCache,
    path: P,
    file_name: &str,
    dependencies: &[String],
    check_fn: F,
    found_deps: &mut Vec<String>,
    evidence: &mut Vec<String>,
) -> crate::Result<bool>
where
    P: AsRef<std::path::Path>,
    F: FnOnce(&str, &[String]) -> Vec<String>,
{
    let file_path = path.as_ref().join(file_name);
    if let Some(content) = parsed_cache.get_file_content(&file_path)? {
        let deps = check_fn(&content, dependencies);
        if !deps.is_empty() {
            found_deps.extend(deps);
            evidence.push(file_name.to_owned());
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_mod_matches_single_line_require() {
        // `go mod tidy` writes a lone direct dep as a single-line require
        let content =
            "module example.com/app\n\ngo 1.22\n\nrequire github.com/gin-gonic/gin v1.9.1\n";
        let deps = vec!["github.com/gin-gonic/gin".to_string()];
        assert_eq!(check_go_mod_dependencies(content, &deps), deps);
    }

    #[test]
    fn test_go_mod_matches_require_block() {
        let content = "module x\n\nrequire (\n    github.com/gin-gonic/gin v1.9.1\n)\n";
        let deps = vec!["github.com/gin-gonic/gin".to_string()];
        assert_eq!(check_go_mod_dependencies(content, &deps), deps);
    }

    #[test]
    fn test_text_dependency_does_not_apply_go_mod_grammar() {
        // Only go.mod lines carry a `require` keyword; other text manifests
        // must not treat such lines as dependency declarations
        let content = "# notes\nrequire django\n";
        assert!(check_text_dependencies(content, &["django".to_string()]).is_empty());
    }

    #[test]
    fn test_gradle_does_not_match_artifact_name_substring() {
        let content = "dependencies {\n    implementation(\"org.example:ginseng:1.0\")\n}\n";
        assert!(check_gradle_dependencies(content, &["gin".to_string()]).is_empty());
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
        assert_eq!(check_gradle_dependencies(content, &deps), deps);
    }

    #[test]
    fn test_gemfile_lock_does_not_match_prefix_named_gem() {
        let content = "GEM\n  specs:\n    rails-html-sanitizer (1.4.4)\n";
        assert!(check_gemfile_lock_dependencies(content, &["rails".to_string()]).is_empty());
    }

    #[test]
    fn test_gemfile_lock_matches_exact_gem() {
        let content = "GEM\n  specs:\n    rails (7.0.4)\n";
        assert_eq!(
            check_gemfile_lock_dependencies(content, &["rails".to_string()]),
            vec!["rails".to_string()]
        );
    }

    #[test]
    fn test_yarn_lock_does_not_match_substring_package() {
        let content = "preact@^10.0.0:\n  version \"10.19.3\"\n\n\"preact-render-to-string@^6.0.0\":\n  version \"6.3.1\"\n";
        let deps = vec!["react".to_string()];
        assert!(check_yarn_lock_dependencies(content, &deps).is_empty());
    }

    #[test]
    fn test_yarn_lock_matches_exact_package() {
        let content = "react@^18.2.0:\n  version \"18.2.0\"\n\n\"@babel/core@^7.0.0\":\n  version \"7.23.0\"\n";
        let deps = vec!["react".to_string(), "@babel/core".to_string()];
        let found = check_yarn_lock_dependencies(content, &deps);
        assert_eq!(found, vec!["react".to_string(), "@babel/core".to_string()]);
    }

    #[test]
    fn test_pnpm_lock_does_not_match_substring_package() {
        let content = "dependencies:\n  preact: 10.19.3\n";
        let deps = vec!["react".to_string()];
        assert!(check_pnpm_lock_dependencies(content, &deps).is_empty());
    }

    #[test]
    fn test_pnpm_lock_matches_exact_package() {
        let content = "dependencies:\n  react: 18.2.0\n";
        let deps = vec!["react".to_string()];
        let found = check_pnpm_lock_dependencies(content, &deps);
        assert_eq!(found, vec!["react".to_string()]);
    }

    #[test]
    fn test_text_dependency_ignores_comments() {
        let content = "# django is not actually used here\nflask==3.0.0\n";
        let deps = vec!["django".to_string()];
        assert!(check_text_dependencies(content, &deps).is_empty());
    }

    #[test]
    fn test_text_dependency_requires_name_boundary() {
        // `django-extensions` is a different package than `django`
        let content = "django-extensions==3.2.3\n";
        let deps = vec!["django".to_string()];
        assert!(check_text_dependencies(content, &deps).is_empty());
    }

    #[test]
    fn test_text_dependency_matches_specifiers() {
        for line in ["django", "django==4.2", "django>=4", "django[argon2]==4.2"] {
            let deps = vec!["django".to_string()];
            assert_eq!(
                check_text_dependencies(line, &deps),
                vec!["django".to_string()],
                "should match line: {line}"
            );
        }
    }

    #[test]
    fn test_text_dependency_matches_gemfile() {
        let content = "source 'https://rubygems.org'\ngem 'rails', '~> 7.1'\n";
        let deps = vec!["rails".to_string()];
        assert_eq!(
            check_text_dependencies(content, &deps),
            vec!["rails".to_string()]
        );
    }

    #[test]
    fn test_poetry_lock_matches_single_quoted_name() {
        let content = "[[package]]\nname = 'django'\nversion = \"4.2\"\n";
        let deps = vec!["django".to_string()];
        assert_eq!(
            check_poetry_lock_dependencies(content, &deps),
            vec!["django".to_string()]
        );
    }

    fn d(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_python_requirements_are_case_and_separator_insensitive() {
        let content = "Django==4.2\nFlask\nFastAPI[standard]>=0.100\nflask_cors ~= 3.0 ; python_version > '3'\n";
        assert_eq!(
            check_python_text_dependencies(
                content,
                &d(&["django", "flask", "fastapi", "flask-cors"])
            ),
            d(&["django", "flask", "fastapi", "flask-cors"])
        );
    }

    #[test]
    fn test_python_requirements_reject_lookalikes_and_comments() {
        let content = "django-extensions==3.0\n# flask\n-r base.txt\nflasky==1\n";
        assert!(check_python_text_dependencies(content, &d(&["django", "flask"])).is_empty());
    }

    #[test]
    fn test_pyproject_extras_spaces_case_and_dependency_groups() {
        let toml: toml::Value = toml::from_str(
            "[project]\ndependencies = [\"fastapi[standard]>=0.100\", \"Django>=4\", \"flask ~= 3.0\"]\n\n[dependency-groups]\ndev = [\"pytest>=8\"]\n",
        )
        .unwrap_or_else(|e| panic!("toml: {e}"));
        assert_eq!(
            check_pyproject_dependencies(
                &toml,
                &d(&["fastapi", "django", "flask", "pytest", "fast"])
            ),
            d(&["fastapi", "django", "flask", "pytest"])
        );
    }

    #[test]
    fn test_mix_rejects_dep_name_prefixes() {
        let content = "defp deps do\n  [{:phoenix_pubsub, \"~> 2.0\"}, {:ecto, \"~> 3\"}]\nend";
        assert!(check_mix_dependencies(content, &d(&["phoenix"])).is_empty());
        let real = "[{:phoenix, \"~> 1.7\"}]";
        assert_eq!(
            check_mix_dependencies(real, &d(&["phoenix"])),
            d(&["phoenix"])
        );
    }

    #[test]
    fn test_swift_only_package_declarations_count() {
        let target_only =
            "let package = Package(name: \"app\", targets: [.executableTarget(name: \"vapor\")])";
        assert!(check_swift_package_dependencies(target_only, &d(&["vapor"])).is_empty());
        for real in [
            ".package(url: \"https://github.com/vapor/vapor.git\", from: \"4.0.0\")",
            ".package(url: \"https://github.com/vapor/vapor\", from: \"4.0.0\")",
        ] {
            assert_eq!(
                check_swift_package_dependencies(real, &d(&["vapor"])),
                d(&["vapor"])
            );
        }
    }

    #[test]
    fn test_pom_ignores_non_artifact_element_text() {
        let name_only = "<project><name>spring-boot-starter</name></project>";
        assert!(check_pom_xml_dependencies(name_only, &d(&["spring-boot-starter"])).is_empty());
        let real = "<dependency><artifactId>spring-boot-starter</artifactId></dependency>";
        assert_eq!(
            check_pom_xml_dependencies(real, &d(&["spring-boot-starter"])),
            d(&["spring-boot-starter"])
        );
    }

    #[test]
    fn test_sbt_matches_artifact_position_only() {
        let content =
            "libraryDependencies += \"com.example\" %% \"play\" % \"2.9\"\nname := \"akka-http\"";
        assert_eq!(check_sbt_dependencies(content, &d(&["play"])), d(&["play"]));
        assert!(check_sbt_dependencies(content, &d(&["akka-http"])).is_empty());
    }

    #[test]
    fn test_go_mod_ignores_indirect_requirements() {
        let content = "module x\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1 // indirect\n\tgithub.com/labstack/echo/v4 v4.11.0\n)\n";
        assert!(check_go_mod_dependencies(content, &d(&["github.com/gin-gonic/gin"])).is_empty());
        assert_eq!(
            check_go_mod_dependencies(content, &d(&["github.com/labstack/echo/v4"])),
            d(&["github.com/labstack/echo/v4"])
        );
    }

    #[test]
    fn test_gradle_ignores_bare_quoted_strings() {
        let content =
            "rootProject.name = \"gin\"\ndependencies { implementation(\"io.gin:gin:1.0\") }";
        // the project name alone must not count; the real coordinate does
        assert!(check_gradle_dependencies("rootProject.name = \"gin\"", &d(&["gin"])).is_empty());
        assert_eq!(
            check_gradle_dependencies(content, &d(&["gin"])),
            d(&["gin"])
        );
    }

    #[test]
    fn test_toml_workspace_and_target_dependencies() {
        let toml: toml::Value = toml::from_str(
            "[workspace.dependencies]\naxum = \"0.7\"\n\n[target.'cfg(unix)'.dependencies]\nrocket = \"0.5\"\n\n[workspace.dev-dependencies]\nactix-web = \"4\"\n",
        )
        .unwrap_or_else(|e| panic!("toml: {e}"));
        assert_eq!(
            check_toml_dependencies(&toml, &d(&["axum", "rocket", "actix-web", "tonic"])),
            d(&["axum", "rocket", "actix-web"])
        );
    }

    #[test]
    fn test_json_optional_dependencies() {
        let json: serde_json::Value = serde_json::json!({"optionalDependencies": {"react": "18"}});
        assert_eq!(
            check_json_dependencies(&json, &d(&["react"])),
            d(&["react"])
        );
    }

    #[test]
    fn test_python_requirements_inline_comments_and_bom() {
        let content = "\u{feff}django  # web framework\nflask==3.0  # micro\nrequests\t# http\n";
        assert_eq!(
            check_python_text_dependencies(content, &d(&["django", "flask", "requests"])),
            d(&["django", "flask", "requests"])
        );
        // a comment is not a dependency
        assert!(check_python_text_dependencies("# django\n", &d(&["django"])).is_empty());
    }
}
