use super::Config;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
fn first_existing(paths: Vec<PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.exists())
}

pub fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Never the current directory: a repository chooses what sits there
    if let Some(config_dir) = xdg_config_dir() {
        paths.push(config_dir.join("project-indicator").join("config.toml"));
        paths.push(config_dir.join("project-indicator.toml"));
    }

    if let Some(home_dir) = std::env::home_dir() {
        paths.push(home_dir.join(".project-indicator.toml"));
    }

    paths
}
fn xdg_config_dir() -> Option<PathBuf> {
    // Per the XDG spec a relative value is invalid and ignored
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".config")))
}
/// The config file `load_default` would read, without parsing it.
/// `None` means detection runs on the built-in fallback config.
pub fn active_config_path() -> Option<PathBuf> {
    first_existing(default_config_paths())
}
pub fn load_default() -> Result<Config> {
    let Some(path) = active_config_path() else {
        return Ok(load_fallback_config());
    };
    log::debug!("Found config file at: {}", path.display());
    // Soft fallback: a config that fails to parse (old schema, typos) or
    // fails validation (a bad hex color) must never break a shell prompt.
    // `config validate` still surfaces the exact error.
    load_from_file(&path)
        .and_then(|config| {
            crate::config::validate_config(&config)?;
            Ok(config)
        })
        .or_else(|e| {
            log::warn!(
                "Ignoring unusable config at {} ({}); using the built-in configuration. \
                     Run 'project-indicator config validate' to see the error, or \
                     'project-indicator config init --force' to regenerate",
                path.display(),
                e
            );
            Ok(load_fallback_config())
        })
}
pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let content = crate::detection::caches::parsed_file::read_text(
        path,
        crate::detection::caches::parsed_file::MAX_TEXT_BYTES,
    )
    .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    parse_toml_content(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))
}
pub fn parse_toml_content(content: &str) -> Result<Config> {
    toml::from_str(content).with_context(|| "Configuration does not match the current schema")
}
fn load_fallback_config() -> Config {
    log::info!("No configuration file found, using the built-in configuration");
    crate::config::built_in_config()
}
pub fn default_save_path() -> Result<PathBuf> {
    if let Some(config_dir) = xdg_config_dir() {
        Ok(config_dir.join("project-indicator").join("config.toml"))
    } else {
        anyhow::bail!("Could not determine config directory");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_current_schema_config() -> Result<(), Box<dyn std::error::Error>> {
        let toml_content = r##"
[display]
max_frameworks = 2

[[indicators]]
name = "TypeScript"
files = ["package.json", "tsconfig.json"]
color = "#3178C6"
icon = "TS"
ecosystems = ["npm"]

[[frameworks]]
name = "React"
ecosystems = ["npm"]
icon = "React"
priority = 1

[frameworks.detection]
dependencies = ["react"]
"##;

        let config = parse_toml_content(toml_content)?;

        assert_eq!(config.display.max_frameworks, 2);
        assert_eq!(config.indicators.len(), 1);
        assert_eq!(config.indicators[0].name, "TypeScript");
        assert_eq!(
            config.indicators[0].ecosystems,
            vec![crate::types::Ecosystem::Npm]
        );
        assert_eq!(config.frameworks.len(), 1);
        assert_eq!(config.frameworks[0].name, "React");
        Ok(())
    }

    #[test]
    fn test_utf16_config_file_loads() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let path = dir.path().join("config.toml");
        let text = "[display]\nmax_frameworks = 2\n\n[[indicators]]\nname = \"TypeScript\"\nfiles = [\"package.json\"]\ncolor = \"#3178C6\"\nicon = \"TS\"\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        std::fs::write(&path, bytes)?;
        let config = load_from_file(&path)?;
        assert_eq!(config.indicators.len(), 1);
        Ok(())
    }

    #[test]
    fn test_old_config_with_meta_table_fails_to_parse() {
        let err = parse_toml_content(
            r##"
[meta]
version = "3.0"

[[indicators]]
name = "Rust"
files = ["Cargo.toml"]
color = "#dea584"
icon = "R"
"##,
        )
        .err()
        .map(|e| format!("{e:#}"));
        assert!(
            err.as_deref().is_some_and(|m| m.contains("meta")),
            "{err:?}"
        );
    }

    #[test]
    fn test_invalid_toml() {
        let invalid_toml = "this is not valid toml [[[";
        let result = parse_toml_content(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_every_candidate_config_path_is_absolute() {
        // a relative path would be keyed (and cached) against whichever
        // directory the next invocation happens to run in
        for path in default_config_paths() {
            assert!(path.is_absolute(), "{}", path.display());
        }
    }

    #[test]
    fn test_first_existing_picks_earliest_present_path() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let missing = dir.path().join("missing.toml");
        let present_a = dir.path().join("a.toml");
        let present_b = dir.path().join("b.toml");
        fs::write(&present_a, "x")?;
        fs::write(&present_b, "x")?;

        assert_eq!(
            first_existing(vec![missing.clone(), present_a.clone(), present_b]),
            Some(present_a)
        );
        assert_eq!(first_existing(vec![missing]), None);
        Ok(())
    }

    #[test]
    fn test_fallback_config() {
        let config = load_fallback_config();
        assert!(!config.indicators.is_empty());

        let language_names: Vec<&String> = config.indicators.iter().map(|l| &l.name).collect();
        assert!(language_names.contains(&&"Rust".to_string()));
        assert!(language_names.contains(&&"TypeScript".to_string()));
        assert!(language_names.contains(&&"Python".to_string()));

        assert!(
            !config.frameworks.is_empty(),
            "fallback config should define frameworks"
        );
    }
}
