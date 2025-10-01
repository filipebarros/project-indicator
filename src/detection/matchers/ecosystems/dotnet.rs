use super::helpers::{check_csproj_dependencies, check_packages_config_dependencies};
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

/// Detect .NET ecosystem dependencies
pub fn check_dotnet_ecosystem<P: AsRef<Path>>(
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
            let csproj_deps = check_csproj_dependencies(&content, packages);
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
                        let csproj_deps = check_csproj_dependencies(&content, packages);
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
        let package_config_deps = check_packages_config_dependencies(&content, packages);
        if !package_config_deps.is_empty() {
            found_deps.extend(package_config_deps);
            evidence.push("packages.config".to_owned());
            return Ok((found_deps, evidence));
        }
    }

    if let Some(content) = parsed_cache.get_config_file(&path, "Directory.Build.props")? {
        let props_deps = check_csproj_dependencies(&content, packages);
        if !props_deps.is_empty() {
            found_deps.extend(props_deps);
            evidence.push("Directory.Build.props".to_owned());
        }
    }

    Ok((found_deps, evidence))
}
