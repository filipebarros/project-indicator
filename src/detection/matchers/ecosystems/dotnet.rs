use super::helpers::{
    check_csproj_dependencies, check_packages_config_dependencies, files_with_extension, first_hit,
    TextCheck,
};
use crate::constants::PACKAGES_CONFIG;
use crate::detection::caches::ParsedFileCache;
use crate::Result;
use std::path::Path;

pub fn check_dotnet_ecosystem(
    path: &Path,
    packages: &[String],
    parsed_cache: &ParsedFileCache,
) -> Result<Option<String>> {
    let csprojs = files_with_extension(path, ".csproj");
    let mut probes: Vec<(&str, TextCheck)> = csprojs
        .iter()
        .map(|name| (name.as_str(), check_csproj_dependencies as TextCheck))
        .collect();
    probes.push((PACKAGES_CONFIG, check_packages_config_dependencies));
    probes.push(("Directory.Build.props", check_csproj_dependencies));

    first_hit(parsed_cache, path, &probes, packages)
}
