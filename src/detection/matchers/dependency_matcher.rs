use super::common::{calculate_dependency_confidence, sort_framework_matches};
use crate::constants::{
    CARGO_LOCK, CARGO_TOML, COMPOSER_JSON, COMPOSER_LOCK, GEMFILE, GEMFILE_LOCK, GO_MOD,
    PACKAGE_JSON, PACKAGE_LOCK_JSON, PNPM_LOCK_YAML, POETRY_LOCK, PYPROJECT_TOML, YARN_LOCK,
};
use crate::detection::parsed_file_cache::ParsedFileCache;
use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use std::collections::HashSet;
use std::path::Path;

pub struct DependencyMatcher;

impl DependencyMatcher {
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        let mut matches = Vec::with_capacity(frameworks.len());

        for framework in frameworks {
            if let Some(framework_match) =
                Self::try_detect_ecosystem_framework(&path, framework, parsed_cache)?
            {
                matches.push(framework_match);
            }
        }

        sort_framework_matches(&mut matches);
        Ok(matches)
    }

    fn try_detect_ecosystem_framework<P: AsRef<Path>>(
        path: P,
        framework: &FrameworkDetector,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Option<FrameworkMatch>> {
        let (found_deps, evidence) = match &framework.detection {
            DetectionType::NodeEcosystem { dependencies } => {
                Self::check_node_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::PythonEcosystem { dependencies } => {
                Self::check_python_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::RustEcosystem { dependencies } => {
                Self::check_rust_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::GoEcosystem { modules } => {
                Self::check_go_ecosystem(&path, modules, parsed_cache)?
            }
            DetectionType::PHPEcosystem { packages } => {
                Self::check_php_ecosystem(&path, packages, parsed_cache)?
            }
            DetectionType::RubyEcosystem { gems } => {
                Self::check_ruby_ecosystem(&path, gems, parsed_cache)?
            }
            DetectionType::JavaEcosystem { dependencies } => {
                Self::check_java_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::DotNetEcosystem { packages } => {
                Self::check_dotnet_ecosystem(&path, packages, parsed_cache)?
            }
            DetectionType::ScalaEcosystem { dependencies } => {
                Self::check_scala_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::DartEcosystem { dependencies } => {
                Self::check_dart_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::LuaEcosystem { packages } => {
                Self::check_lua_ecosystem(&path, packages, parsed_cache)?
            }
            _ => return Ok(None),
        };

        if found_deps.is_empty() {
            return Ok(None);
        }

        let target_deps = Self::get_target_dependencies(&framework.detection);
        let confidence = calculate_dependency_confidence(&target_deps, &found_deps);
        Ok(Some(FrameworkMatch::new(
            framework.clone(),
            confidence,
            evidence,
        )))
    }

    fn get_target_dependencies(detection: &DetectionType) -> Vec<String> {
        match detection {
            DetectionType::NodeEcosystem { dependencies }
            | DetectionType::PythonEcosystem { dependencies }
            | DetectionType::RustEcosystem { dependencies }
            | DetectionType::JavaEcosystem { dependencies }
            | DetectionType::ScalaEcosystem { dependencies }
            | DetectionType::DartEcosystem { dependencies } => dependencies.clone(),
            DetectionType::GoEcosystem { modules } => modules.clone(),
            DetectionType::PHPEcosystem { packages }
            | DetectionType::DotNetEcosystem { packages }
            | DetectionType::LuaEcosystem { packages } => packages.clone(),
            DetectionType::RubyEcosystem { gems } => gems.clone(),
            _ => Vec::new(),
        }
    }

    fn check_node_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(json_value) = parsed_cache.get_package_json(&path)? {
            let package_deps = Self::check_json_dependencies(&json_value, dependencies);
            if !package_deps.is_empty() {
                found_deps.extend(package_deps);
                evidence.push(PACKAGE_JSON.to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(json_value) = parsed_cache.get_package_lock_json(&path)? {
            let lock_deps = Self::check_package_lock_dependencies(&json_value, dependencies);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push(PACKAGE_LOCK_JSON.to_owned());
            }
        } else if let Some(content) = parsed_cache.get_yarn_lock(&path)? {
            let yarn_deps = Self::check_yarn_lock_dependencies(&content, dependencies);
            if !yarn_deps.is_empty() {
                found_deps.extend(yarn_deps);
                evidence.push(YARN_LOCK.to_owned());
            }
        } else if let Some(content) = parsed_cache.get_pnpm_lock_yaml(&path)? {
            let pnpm_deps = Self::check_pnpm_lock_dependencies(&content, dependencies);
            if !pnpm_deps.is_empty() {
                found_deps.extend(pnpm_deps);
                evidence.push(PNPM_LOCK_YAML.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_python_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(toml_value) = parsed_cache.get_pyproject_toml(&path)? {
            let pyproject_deps = Self::check_pyproject_dependencies(&toml_value, dependencies);
            if !pyproject_deps.is_empty() {
                found_deps.extend(pyproject_deps);
                evidence.push(PYPROJECT_TOML.to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "requirements.txt")? {
            let req_deps = Self::check_text_dependencies(&content, dependencies);
            if !req_deps.is_empty() {
                found_deps.extend(req_deps);
                evidence.push("requirements.txt".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "Pipfile")? {
            let pipfile_deps = Self::check_text_dependencies(&content, dependencies);
            if !pipfile_deps.is_empty() {
                found_deps.extend(pipfile_deps);
                evidence.push("Pipfile".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_poetry_lock(&path)? {
            let poetry_deps = Self::check_poetry_lock_dependencies(&content, dependencies);
            if !poetry_deps.is_empty() {
                found_deps.extend(poetry_deps);
                evidence.push(POETRY_LOCK.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_rust_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(toml_value) = parsed_cache.get_cargo_toml(&path)? {
            let cargo_deps = Self::check_toml_dependencies(&toml_value, dependencies);
            if !cargo_deps.is_empty() {
                found_deps.extend(cargo_deps);
                evidence.push(CARGO_TOML.to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_cargo_lock(&path)? {
            let lock_deps = Self::check_cargo_lock_dependencies(&content, dependencies);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push(CARGO_LOCK.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_go_ecosystem<P: AsRef<Path>>(
        path: P,
        modules: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(content) = parsed_cache.get_go_mod(&path)? {
            let go_deps = Self::check_text_dependencies(&content, modules);
            if !go_deps.is_empty() {
                found_deps.extend(go_deps);
                evidence.push(GO_MOD.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_php_ecosystem<P: AsRef<Path>>(
        path: P,
        packages: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(json_value) = parsed_cache.get_composer_json(&path)? {
            let composer_deps = Self::check_json_dependencies(&json_value, packages);
            if !composer_deps.is_empty() {
                found_deps.extend(composer_deps);
                evidence.push(COMPOSER_JSON.to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(json_value) = parsed_cache.get_composer_lock(&path)? {
            let lock_deps = Self::check_composer_lock_dependencies(&json_value, packages);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push(COMPOSER_LOCK.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_ruby_ecosystem<P: AsRef<Path>>(
        path: P,
        gems: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(content) = parsed_cache.get_config_file(&path, "Gemfile")? {
            let gemfile_deps = Self::check_text_dependencies(&content, gems);
            if !gemfile_deps.is_empty() {
                found_deps.extend(gemfile_deps);
                evidence.push(GEMFILE.to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_gemfile_lock(&path)? {
            let lock_deps = Self::check_gemfile_lock_dependencies(&content, gems);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push(GEMFILE_LOCK.to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_java_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(xml_content) = parsed_cache.get_config_file(&path, "pom.xml")? {
            let maven_deps = Self::check_pom_xml_dependencies(&xml_content, dependencies);
            if !maven_deps.is_empty() {
                found_deps.extend(maven_deps);
                evidence.push("pom.xml".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(gradle_content) = parsed_cache.get_config_file(&path, "build.gradle")? {
            let gradle_deps = Self::check_gradle_dependencies(&gradle_content, dependencies);
            if !gradle_deps.is_empty() {
                found_deps.extend(gradle_deps);
                evidence.push("build.gradle".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(gradle_kts_content) = parsed_cache.get_config_file(&path, "build.gradle.kts")? {
            let gradle_kts_deps =
                Self::check_gradle_dependencies(&gradle_kts_content, dependencies);
            if !gradle_kts_deps.is_empty() {
                found_deps.extend(gradle_kts_deps);
                evidence.push("build.gradle.kts".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_dotnet_ecosystem<P: AsRef<Path>>(
        path: P,
        packages: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        let common_csproj_names = [
            "app.csproj",
            "web.csproj",
            "api.csproj",
            "main.csproj",
            "server.csproj",
            "client.csproj",
        ];

        for csproj_name in &common_csproj_names {
            if let Some(content) = parsed_cache.get_config_file(&path, csproj_name)? {
                let csproj_deps = Self::check_csproj_dependencies(&content, packages);
                if !csproj_deps.is_empty() {
                    found_deps.extend(csproj_deps);
                    evidence.push(csproj_name.to_string());
                    return Ok((found_deps, evidence));
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.ends_with(".csproj") {
                        if let Some(content) = parsed_cache.get_config_file(&path, filename)? {
                            let csproj_deps = Self::check_csproj_dependencies(&content, packages);
                            if !csproj_deps.is_empty() {
                                found_deps.extend(csproj_deps);
                                evidence.push(filename.to_string());
                                return Ok((found_deps, evidence));
                            }
                        }
                    }
                }
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "packages.config")? {
            let package_config_deps = Self::check_packages_config_dependencies(&content, packages);
            if !package_config_deps.is_empty() {
                found_deps.extend(package_config_deps);
                evidence.push("packages.config".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "Directory.Build.props")? {
            let props_deps = Self::check_csproj_dependencies(&content, packages);
            if !props_deps.is_empty() {
                found_deps.extend(props_deps);
                evidence.push("Directory.Build.props".to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_scala_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(content) = parsed_cache.get_config_file(&path, "build.sbt")? {
            let sbt_deps = Self::check_sbt_dependencies(&content, dependencies);
            if !sbt_deps.is_empty() {
                found_deps.extend(sbt_deps);
                evidence.push("build.sbt".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "build.sc")? {
            let mill_deps = Self::check_text_dependencies(&content, dependencies);
            if !mill_deps.is_empty() {
                found_deps.extend(mill_deps);
                evidence.push("build.sc".to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_dart_ecosystem<P: AsRef<Path>>(
        path: P,
        dependencies: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(yaml_content) = parsed_cache.get_config_file(&path, "pubspec.yaml")? {
            let pubspec_deps = Self::check_pubspec_dependencies(&yaml_content, dependencies);
            if !pubspec_deps.is_empty() {
                found_deps.extend(pubspec_deps);
                evidence.push("pubspec.yaml".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(content) = parsed_cache.get_config_file(&path, "pubspec.lock")? {
            let lock_deps = Self::check_pubspec_lock_dependencies(&content, dependencies);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push("pubspec.lock".to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_lua_ecosystem<P: AsRef<Path>>(
        path: P,
        packages: &[String],
        parsed_cache: &ParsedFileCache,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut found_deps = Vec::new();
        let mut evidence = Vec::new();

        if let Some(rockspec_content) = parsed_cache.get_config_file(&path, "*.rockspec")? {
            let rockspec_deps = Self::check_rockspec_dependencies(&rockspec_content, packages);
            if !rockspec_deps.is_empty() {
                found_deps.extend(rockspec_deps);
                evidence.push("rockspec".to_owned());
                return Ok((found_deps, evidence));
            }
        }

        if let Some(lock_content) = parsed_cache.get_config_file(&path, "luarocks.lock")? {
            let lock_deps = Self::check_luarocks_lock_dependencies(&lock_content, packages);
            if !lock_deps.is_empty() {
                found_deps.extend(lock_deps);
                evidence.push("luarocks.lock".to_owned());
            }
        }

        Ok((found_deps, evidence))
    }

    fn check_toml_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> Vec<String> {
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
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_json_dependencies(
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
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_pyproject_dependencies(toml_value: &toml::Value, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::new();

        for dep_name in dep_names {
            if Self::has_pyproject_dependency(toml_value, dep_name) {
                found_deps.push(dep_name.clone());
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

    fn check_text_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        if content.is_empty() {
            return found_deps;
        }

        if dep_names.len() == 1 {
            if Self::has_text_dependency(content, &dep_names[0]) {
                found_deps.push(dep_names[0].clone());
            }
            return found_deps;
        }

        for dep_name in dep_names {
            if Self::has_text_dependency(content, dep_name) {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn has_text_dependency(content: &str, dep_name: &str) -> bool {
        if content.contains(&format!("gem '{}'", dep_name))
            || content.contains(&format!("gem \"{}\"", dep_name))
        {
            return true;
        }

        if content.contains(dep_name) {
            return true;
        }

        false
    }

    fn check_package_lock_dependencies(
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
                    found_deps.push(dep_name.clone());
                }
            }
        }

        found_deps
    }

    fn check_yarn_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        if content.is_empty() {
            return found_deps;
        }

        for dep_name in dep_names {
            if content.contains(&format!("{}@", dep_name))
                || content.contains(&format!("\"{}@", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_pnpm_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        if content.is_empty() {
            return found_deps;
        }

        for dep_name in dep_names {
            if content.contains(&format!("{}:", dep_name))
                || content.contains(&format!("  {}:", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_composer_lock_dependencies(
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
                    found_deps.push(package_name.clone());
                }
            }
        }

        found_deps
    }

    fn check_gemfile_lock_dependencies(content: &str, gem_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(gem_names.len());

        let specs_section = content.find("specs:");
        if let Some(specs_start) = specs_section {
            let specs_content = &content[specs_start..];

            for gem_name in gem_names {
                if specs_content.contains(&format!("    {} (", gem_name))
                    || specs_content.contains(&format!("    {}-", gem_name))
                {
                    found_deps.push(gem_name.clone());
                }
            }
        }

        found_deps
    }

    fn check_poetry_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!("name = \"{}\"", dep_name))
                || content.contains(&format!("name = '{}']", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_cargo_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!("name = \"{}\"", dep_name)) {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_sbt_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!("\"{}\"", dep_name))
                || content.contains(&format!("'{}'", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_pubspec_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!("  {}:", dep_name))
                || content.contains(&format!("  {}: ", dep_name))
                || content.contains(&format!("\n{}:", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_pubspec_lock_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        if let Some(packages_start) = content.find("packages:") {
            let packages_section = &content[packages_start..];

            for dep_name in dep_names {
                if packages_section.contains(&format!("  {}:", dep_name)) {
                    found_deps.push(dep_name.clone());
                }
            }
        }

        found_deps
    }

    fn check_pom_xml_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!("<artifactId>{}</artifactId>", dep_name))
                || content.contains(&format!("<artifactId>{}-", dep_name))
                || content.contains(&format!(">{}<", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_gradle_dependencies(content: &str, dep_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(dep_names.len());

        for dep_name in dep_names {
            if content.contains(&format!(":{}", dep_name))
                || content.contains(&format!("'{}'", dep_name))
                || content.contains(&format!("\"{}\"", dep_name))
                || content.contains(&format!("name: '{}'", dep_name))
                || content.contains(&format!("name: \"{}\"", dep_name))
            {
                found_deps.push(dep_name.clone());
            }
        }

        found_deps
    }

    fn check_csproj_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(package_names.len());

        for package_name in package_names {
            if content.contains(&format!("Include=\"{}\"", package_name))
                || content.contains(&format!("Include='{}'", package_name))
                || content.contains(&format!(">{}<", package_name))
            {
                found_deps.push(package_name.clone());
            }
        }

        found_deps
    }

    fn check_packages_config_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(package_names.len());

        for package_name in package_names {
            if content.contains(&format!("id=\"{}\"", package_name))
                || content.contains(&format!("id='{}'", package_name))
            {
                found_deps.push(package_name.clone());
            }
        }

        found_deps
    }

    fn check_rockspec_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(package_names.len());

        for package_name in package_names {
            if content.contains("dependencies = {")
                && (content.contains(&format!("\"{}\"", package_name))
                    || content.contains(&format!("'{}'", package_name))
                    || content.contains(&format!("{} =", package_name)))
            {
                found_deps.push(package_name.clone());
            }
        }

        found_deps
    }

    fn check_luarocks_lock_dependencies(content: &str, package_names: &[String]) -> Vec<String> {
        let mut found_deps = Vec::with_capacity(package_names.len());

        for package_name in package_names {
            if content.contains(&format!("[\"{}\"]", package_name))
                || content.contains(&format!("['{}']", package_name))
                || content.contains(&format!("name = \"{}\"", package_name))
            {
                found_deps.push(package_name.clone());
            }
        }

        found_deps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionType;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_framework(name: &str, detection: DetectionType) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection,
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }
    }

    #[test]
    fn test_unified_cargo_toml_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let cargo_content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
tokio = "1.0"
serde = "1.0"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content)?;

        let framework = create_test_framework(
            "Tokio",
            DetectionType::RustEcosystem {
                dependencies: vec!["tokio".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Tokio");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Cargo.toml"]);
        Ok(())
    }

    #[test]
    fn test_unified_package_json_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_content = r#"{
  "name": "test",
  "dependencies": {
    "react": "^18.0.0",
    "express": "^4.18.0"
  }
}"#;
        fs::write(temp_dir.path().join("package.json"), package_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["package.json"]);
        Ok(())
    }

    #[test]
    fn test_multiple_frameworks_single_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_content = r#"{
  "dependencies": {
    "react": "^18.0.0",
    "vue": "^3.0.0"
  }
}"#;
        fs::write(temp_dir.path().join("package.json"), package_content)?;

        let frameworks = vec![
            create_test_framework(
                "React",
                DetectionType::NodeEcosystem {
                    dependencies: vec!["react".to_string()],
                },
            ),
            create_test_framework(
                "Vue",
                DetectionType::NodeEcosystem {
                    dependencies: vec!["vue".to_string()],
                },
            ),
        ];

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &frameworks, &cache)?;

        assert_eq!(matches.len(), 2);
        let names: Vec<&str> = matches.iter().map(|m| m.framework.name.as_str()).collect();
        assert!(names.contains(&"React"));
        assert!(names.contains(&"Vue"));
        Ok(())
    }

    #[test]
    fn test_package_lock_json_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_lock_content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "packages": {
    "": {
      "name": "test-app",
      "version": "1.0.0"
    },
    "node_modules/react": {
      "version": "18.2.0",
      "resolved": "https://registry.npmjs.org/react/-/react-18.2.0.tgz"
    },
    "node_modules/react-dom": {
      "version": "18.2.0",
      "resolved": "https://registry.npmjs.org/react-dom/-/react-18.2.0.tgz"
    }
  }
}"#;
        fs::write(
            temp_dir.path().join("package-lock.json"),
            package_lock_content,
        )?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["package-lock.json"]);
        Ok(())
    }

    #[test]
    fn test_yarn_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let yarn_lock_content = r#"# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1

react@^18.2.0:
  version "18.2.0"
  resolved "https://registry.yarnpkg.com/react/-/react-18.2.0.tgz#555bd98592883255fa00de14f1151a917b5d77d5"
  integrity sha512-/3IjMdb2L9QbBdWiW5e3P2/npwMBaU9mHCSCUzNln0ZCYbcfTsGbTJrU/kGemdH2IWmB2ioZ+zkxtmq6g09fGQ==

"react-dom@^18.2.0":
  version "18.2.0"
  resolved "https://registry.yarnpkg.com/react-dom/-/react-dom-18.2.0.tgz#22aaf38708db2674ed9ada224ca4aa708d821e37"
  integrity sha512-6IMTriUmvsjHUjNtEDudZfuDQUoWXVxKHhlEGSk81n4YFS+r/Kl99wXiwlVXtPBtJenozv2P+hxDsw9eA7Xo6g==
"#;
        fs::write(temp_dir.path().join("yarn.lock"), yarn_lock_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["yarn.lock"]);
        Ok(())
    }

    #[test]
    fn test_pnpm_lock_yaml_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let pnpm_lock_content = r#"lockfileVersion: 5.4

specifiers:
  react: ^18.2.0
  react-dom: ^18.2.0

dependencies:
  react: 18.2.0
  react-dom: 18.2.0_react@18.2.0

packages:
  /react/18.2.0:
    resolution: {integrity: sha512-/3IjMdb2L9QbBdWiW5e3P2/npwMBaU9mHCSCUzNln0ZCYbcfTsGbTJrU/kGemdH2IWmB2ioZ+zkxtmq6g09fGQ==}
    engines: {node: '>=0.10.0'}
    dev: false
"#;
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), pnpm_lock_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["pnpm-lock.yaml"]);
        Ok(())
    }

    #[test]
    fn test_composer_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let composer_lock_content = r#"{
    "_readme": [
        "This file locks the dependencies of your project to a known state",
        "Read more about it at https://getcomposer.org/doc/01-basic-usage.md#installing-dependencies"
    ],
    "content-hash": "5e6a10e1ec8e7e70e1d8f6a4e5a7e8f6f6a10e1e",
    "packages": [
        {
            "name": "laravel/framework",
            "version": "v10.15.0",
            "source": {
                "type": "git",
                "url": "https://github.com/laravel/framework.git",
                "reference": "4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b"
            },
            "dist": {
                "type": "zip",
                "url": "https://api.github.com/repos/laravel/framework/zipball/4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b",
                "reference": "4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b",
                "shasum": ""
            },
            "require": {
                "php": "^8.1"
            },
            "type": "library"
        }
    ],
    "packages-dev": [],
    "aliases": [],
    "minimum-stability": "dev",
    "stability-flags": [],
    "prefer-stable": false,
    "prefer-lowest": false,
    "platform": {
        "php": "^8.1"
    },
    "platform-dev": [],
    "plugin-api-version": "2.3.0"
}"#;
        fs::write(temp_dir.path().join("composer.lock"), composer_lock_content)?;

        let framework = create_test_framework(
            "Laravel",
            DetectionType::PHPEcosystem {
                packages: vec!["laravel/framework".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Laravel");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["composer.lock"]);
        Ok(())
    }

    #[test]
    fn test_gemfile_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let gemfile_lock_content = r#"GEM
  remote: https://rubygems.org/
  specs:
    actioncable (7.0.5)
      actionpack (= 7.0.5)
      activesupport (= 7.0.5)
      nio4r (~> 2.0)
      websocket-driver (>= 0.6.1)
    actionmailbox (7.0.5)
      actionpack (= 7.0.5)
      activejob (= 7.0.5)
      activerecord (= 7.0.5)
      activestorage (= 7.0.5)
      activesupport (= 7.0.5)
      mail (>= 2.7.1)
    rails (7.0.5)
      actioncable (= 7.0.5)
      actionmailbox (= 7.0.5)
      actionmailer (= 7.0.5)
      actionpack (= 7.0.5)
      actiontext (= 7.0.5)
      actionview (= 7.0.5)
      activejob (= 7.0.5)
      activemodel (= 7.0.5)
      activerecord (= 7.0.5)
      activestorage (= 7.0.5)
      activesupport (= 7.0.5)
      bootsnap (>= 1.4.4)
      bundler (>= 1.15.0)
      railties (= 7.0.5)

PLATFORMS
  ruby

DEPENDENCIES
  rails (~> 7.0.0)

BUNDLED WITH
   2.4.13
"#;
        fs::write(temp_dir.path().join("Gemfile.lock"), gemfile_lock_content)?;

        let framework = create_test_framework(
            "Rails",
            DetectionType::RubyEcosystem {
                gems: vec!["rails".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Rails");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Gemfile.lock"]);
        Ok(())
    }

    #[test]
    fn test_poetry_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let poetry_lock_content = r#"# This file is automatically @generated by Poetry and should not be changed by hand.

[[package]]
name = "asgiref"
version = "3.7.2"
description = "ASGI specs, helper code, and adapters"
optional = false
python-versions = ">=3.7"
files = [
    {file = "asgiref-3.7.2-py3-none-any.whl", hash = "sha256:89b2ef2247e3b562a16eef663bc0e2e703ec6468e2fa8a5cd61cd449786d4f6e"},
    {file = "asgiref-3.7.2.tar.gz", hash = "sha256:9e0ce3aa93a819ba5b45120216b23878cf6e8525eb3848653452b4192b92afed"},
]

[[package]]
name = "django"
version = "4.2.3"
description = "A high-level Python Web framework that encourages rapid development and clean, pragmatic design."
optional = false
python-versions = ">=3.8"
files = [
    {file = "Django-4.2.3-py3-none-any.whl", hash = "sha256:f7c7852a5ac5a3da5a8d5b35cc6168f31b605971441798dac845f17ca8028039"},
    {file = "Django-4.2.3.tar.gz", hash = "sha256:45a747e1c5b3d6df1b141b1481e193b033fd1fdbda3ff52677dc81afdaacbaed"},
]

[package.dependencies]
asgiref = ">=3.6.0,<4"
sqlparse = ">=0.3.1"
tzdata = {version = "*", markers = "sys_platform == \"win32\""}

[metadata]
lock-version = "2.0"
python-versions = "^3.8"
content-hash = "f7c7852a5ac5a3da5a8d5b35cc6168f31b605971441798dac845f17ca8028039"
"#;
        fs::write(temp_dir.path().join("poetry.lock"), poetry_lock_content)?;

        let framework = create_test_framework(
            "Django",
            DetectionType::PythonEcosystem {
                dependencies: vec!["django".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Django");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["poetry.lock"]);
        Ok(())
    }

    #[test]
    fn test_cargo_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let cargo_lock_content = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "axum"
version = "0.6.18"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "f8175979259124331c1d7bf6586ee7e0da434155e4b2d48ec2c8386281d8df39"
dependencies = [
 "async-trait",
 "axum-core",
 "bitflags 1.3.2",
 "bytes",
 "futures-util",
 "http",
 "http-body",
 "hyper",
 "itoa",
 "matchit",
 "memchr",
 "mime",
 "percent-encoding",
 "pin-project-lite",
 "rustversion",
 "serde",
 "serde_json",
 "serde_path_to_error",
 "serde_urlencoded",
 "sync_wrapper",
 "tokio",
 "tower",
 "tower-layer",
 "tower-service",
]

[[package]]
name = "test-app"
version = "0.1.0"
dependencies = [
 "axum",
 "tokio",
]
"#;
        fs::write(temp_dir.path().join("Cargo.lock"), cargo_lock_content)?;

        let framework = create_test_framework(
            "Axum",
            DetectionType::RustEcosystem {
                dependencies: vec!["axum".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Axum");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Cargo.lock"]);
        Ok(())
    }

    #[test]
    fn test_java_maven_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let pom_xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <groupId>com.example</groupId>
    <artifactId>spring-app</artifactId>
    <version>1.0.0</version>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter</artifactId>
            <version>2.7.0</version>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>2.7.0</version>
        </dependency>
    </dependencies>
</project>"#;

        fs::write(temp_dir.path().join("pom.xml"), pom_xml_content)?;

        let framework = create_test_framework(
            "Spring Boot",
            DetectionType::JavaEcosystem {
                dependencies: vec![
                    "spring-boot-starter".to_string(),
                    "spring-boot-starter-web".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Spring Boot");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["pom.xml"]);
        Ok(())
    }

    #[test]
    fn test_java_gradle_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let build_gradle_content = r#"
plugins {
    id 'org.springframework.boot' version '2.7.0'
    id 'io.spring.dependency-management' version '1.0.11.RELEASE'
    id 'java'
}

dependencies {
    implementation 'org.springframework.boot:spring-boot-starter'
    implementation 'org.springframework.boot:spring-boot-starter-web'
    testImplementation 'org.springframework.boot:spring-boot-starter-test'
}
"#;

        fs::write(temp_dir.path().join("build.gradle"), build_gradle_content)?;

        let framework = create_test_framework(
            "Spring Boot",
            DetectionType::JavaEcosystem {
                dependencies: vec![
                    "spring-boot-starter".to_string(),
                    "spring-boot-starter-web".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Spring Boot");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["build.gradle"]);
        Ok(())
    }

    #[test]
    fn test_dotnet_csproj_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let csproj_content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.AspNetCore.App" Version="2.1.1" />
    <PackageReference Include="Microsoft.EntityFrameworkCore" Version="6.0.0" />
    <PackageReference Include="Microsoft.EntityFrameworkCore.SqlServer" Version="6.0.0" />
  </ItemGroup>
</Project>"#;

        fs::write(temp_dir.path().join("web.csproj"), csproj_content)?;

        let framework = create_test_framework(
            "ASP.NET Core",
            DetectionType::DotNetEcosystem {
                packages: vec![
                    "Microsoft.AspNetCore".to_string(),
                    "Microsoft.EntityFrameworkCore".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "ASP.NET Core");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["web.csproj"]);
        Ok(())
    }
}
