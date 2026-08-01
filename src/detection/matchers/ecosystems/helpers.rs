use std::collections::HashSet;

/// Check TOML dependencies (Cargo.toml, pyproject.toml)
pub fn check_toml_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    let mut available_deps = HashSet::new();

    if let Some(deps) = toml_value.get("dependencies").and_then(|v| v.as_table()) {
        available_deps.extend(deps.keys());
    }
    if let Some(dev_deps) = toml_value
        .get("dev-dependencies")
        .and_then(|v| v.as_table())
    {
        available_deps.extend(dev_deps.keys());
    }
    if let Some(build_deps) = toml_value
        .get("build-dependencies")
        .and_then(|v| v.as_table())
    {
        available_deps.extend(build_deps.keys());
    }

    for dep_name in dep_names {
        if available_deps.contains(dep_name) {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
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

fn has_pyproject_dependency(toml_value: &toml::Value, dep_name: &str) -> bool {
    if let Some(project) = toml_value.get("project") {
        if let Some(deps) = project.get("dependencies") {
            if let Some(deps_array) = deps.as_array() {
                for dep in deps_array {
                    if let Some(dep_str) = dep.as_str() {
                        if dep_str.starts_with(dep_name)
                            && (dep_str == dep_name
                                || dep_str
                                    .chars()
                                    .nth(dep_name.len())
                                    .is_some_and(|c| ">=<!=~".contains(c)))
                        {
                            return true;
                        }
                    }
                }
            }
        }

        if let Some(optional_deps) = project.get("optional-dependencies") {
            if let Some(optional_deps_table) = optional_deps.as_table() {
                for (_, deps) in optional_deps_table {
                    if let Some(deps_array) = deps.as_array() {
                        for dep in deps_array {
                            if let Some(dep_str) = dep.as_str() {
                                if dep_str.starts_with(dep_name)
                                    && (dep_str == dep_name
                                        || dep_str
                                            .chars()
                                            .nth(dep_name.len())
                                            .is_some_and(|c| ">=<!=~".contains(c)))
                                {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(tool) = toml_value.get("tool") {
        if let Some(tool_table) = tool.as_table() {
            for (_, tool_section) in tool_table {
                if let Some(deps) = tool_section.get("dependencies") {
                    if let Some(deps_table) = deps.as_table() {
                        if deps_table.contains_key(dep_name) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
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

        for gem_name in gem_names {
            if specs_content.contains(&format!("    {} (", gem_name))
                || specs_content.contains(&format!("    {}-", gem_name))
            {
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

/// Check build.sbt dependencies
pub fn check_sbt_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        if content.contains(&format!("\"{}\"", dep_name))
            || content.contains(&format!("'{}'", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
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
            || content.contains(&format!(">{}<", dep_name))
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
        if content.contains(&format!(":{}", dep_name))
            || content.contains(&format!("'{}'", dep_name))
            || content.contains(&format!("\"{}\"", dep_name))
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

    for package_name in package_names {
        if content.contains(&format!("Include=\"{}\"", package_name))
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

/// Check Package.swift dependencies (Swift Package Manager)
pub fn check_swift_package_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        // Match patterns like: .package(url: "https://github.com/vapor/vapor.git", ...)
        // or .package(name: "Vapor", ...)
        if content.contains(&format!("/{}.git", dep_name))
            || content.contains(&format!("name: \"{}\"", dep_name))
            || content.contains(&format!("name: '{}'", dep_name))
            || content.contains(&format!("\"{}\"", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
}

/// Check mix.exs dependencies (Elixir)
pub fn check_mix_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
    let mut found_deps = Vec::with_capacity(dep_names.len());

    for dep_name in dep_names {
        // Match patterns like: {:phoenix, "~> 1.7"} or {:phoenix, github: "phoenixframework/phoenix"}
        if content.contains(&format!("{{:{}", dep_name))
            || content.contains(&format!("{{:\"{}\"", dep_name))
        {
            found_deps.push(dep_name.to_string());
        }
    }

    found_deps
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
}
