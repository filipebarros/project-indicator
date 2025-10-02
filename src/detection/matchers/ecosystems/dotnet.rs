use super::helpers::{
    check_csproj_dependencies, check_packages_config_dependencies, try_config_file_deps,
};
use crate::constants::PACKAGES_CONFIG;
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
        if try_config_file_deps(
            parsed_cache,
            &path,
            csproj_name,
            packages,
            check_csproj_dependencies,
            &mut found_deps,
            &mut evidence,
        )? {
            return Ok((found_deps, evidence));
        }
    }

    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                if filename.ends_with(".csproj")
                    && try_config_file_deps(
                        parsed_cache,
                        &path,
                        filename,
                        packages,
                        check_csproj_dependencies,
                        &mut found_deps,
                        &mut evidence,
                    )?
                {
                    return Ok((found_deps, evidence));
                }
            }
        }
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        PACKAGES_CONFIG,
        packages,
        check_packages_config_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    if try_config_file_deps(
        parsed_cache,
        &path,
        "Directory.Build.props",
        packages,
        check_csproj_dependencies,
        &mut found_deps,
        &mut evidence,
    )? {
        return Ok((found_deps, evidence));
    }

    Ok((found_deps, evidence))
}
