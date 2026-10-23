use project_indicator::detection::DetectionEngine;
use project_indicator::types::DetectionConfig;
use project_indicator::Config;
use std::fs;
use tempfile::TempDir;
#[test]
fn test_detection_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let config = DetectionConfig::default();

    assert_eq!(config.max_upward_traversal, 10);
    assert!(!config.require_vcs_root);
    assert_eq!(config.max_depth, 1);
    Ok(())
}
#[test]
fn test_detection_engine_with_custom_config() -> Result<(), Box<dyn std::error::Error>> {
    let detection_config = DetectionConfig {
        max_upward_traversal: 3,
        require_vcs_root: false,
        max_depth: 3,
    };

    let engine = DetectionEngine::with_config(vec![], vec![], detection_config);

    let temp_dir = TempDir::new()?;

    let result = engine.detect(temp_dir.path())?;
    assert!(result.is_empty());
    Ok(())
}
#[test]
fn test_detection_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let original_config = DetectionConfig {
        max_upward_traversal: 8,
        require_vcs_root: true,
        max_depth: 3,
    };

    let toml_str = toml::to_string(&original_config)?;
    assert!(toml_str.contains("max_upward_traversal = 8"));
    assert!(toml_str.contains("require_vcs_root = true"));

    let deserialized_config: DetectionConfig = toml::from_str(&toml_str)?;
    assert_eq!(
        deserialized_config.max_upward_traversal,
        original_config.max_upward_traversal
    );
    assert_eq!(
        deserialized_config.require_vcs_root,
        original_config.require_vcs_root
    );
    assert_eq!(deserialized_config.max_depth, original_config.max_depth);
    Ok(())
}
#[test]
fn test_main_config_includes_detection() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new(vec![]);

    assert_eq!(config.detection.max_upward_traversal, 10);
    assert_eq!(config.detection.max_depth, 1);
    Ok(())
}
#[test]
fn test_full_config_with_detection_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new(vec![]);

    let toml_str = toml::to_string(&config)?;

    assert!(toml_str.contains("[detection]"));
    assert!(toml_str.contains("max_upward_traversal"));

    let deserialized_config: Config = toml::from_str(&toml_str)?;
    assert_eq!(
        deserialized_config.detection.max_upward_traversal,
        config.detection.max_upward_traversal
    );
    assert_eq!(
        deserialized_config.detection.max_depth,
        config.detection.max_depth
    );
    Ok(())
}
#[test]
fn test_config_file_loading_with_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");

    let config_content = "[meta]
version = \"3.0\"

[display]
show_frameworks = true
max_frameworks = 3

[detection]
max_upward_traversal = 5
require_vcs_root = true

[[indicators]]
name = \"Test Language\"
files = [\"test.file\"]
color = \"#FF0000\"
icon = \"🧪\"
priority = 1
";

    fs::write(&config_file, config_content)?;

    let config = Config::load_from_file(&config_file)?;

    assert_eq!(config.detection.max_upward_traversal, 5);
    assert!(config.detection.require_vcs_root);

    assert_eq!(config.display.max_frameworks, 3);
    assert_eq!(config.indicators.len(), 1);
    assert_eq!(config.indicators[0].name, "Test Language");

    Ok(())
}
/// 'config show' must check the same paths the parser reads, mark the
/// first-existing one active and later ones shadowed, and report the active
/// config's contents. (The old hand-rolled list used dirs::config_dir(),
/// which ignores XDG_CONFIG_HOME on macOS and pointed at
/// ~/Library/Application Support instead.) Cwd entries must be printed as
/// absolute paths so the output is meaningful outside the invoking shell.
#[test]
fn test_config_show_marks_active_and_shadowed_configs() -> Result<(), Box<dyn std::error::Error>> {
    let valid_config = r##"
[[indicators]]
name = "Test Language"
files = ["test.file"]
color = "#FF0000"
icon = "T"
priority = 1
"##;
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    let xdg_config = config_dir.join("config.toml");
    fs::write(&xdg_config, valid_config)?;
    let workdir = home.path().join("work");
    fs::create_dir(&workdir)?;
    fs::write(workdir.join("project-indicator.toml"), valid_config)?;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_project-indicator"))
        .args(["config", "show"])
        .current_dir(&workdir)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let active_line = stdout
        .lines()
        .find(|l| l.contains("(active)"))
        .unwrap_or_default();
    assert!(
        active_line.contains("project-indicator.toml")
            && active_line.contains(std::path::MAIN_SEPARATOR),
        "the cwd config must be marked active and shown as an absolute path, got: {stdout}"
    );
    assert!(
        stdout.contains("(shadowed)") && stdout.contains(&xdg_config.display().to_string()),
        "the outranked XDG config must be listed as shadowed, got: {stdout}"
    );
    assert!(
        stdout.contains("Active Configuration") && stdout.contains("Indicators: 1"),
        "show must report the active config's contents, got: {stdout}"
    );
    Ok(())
}

/// When the active config exists but fails to parse, 'config show' must say
/// so instead of silently printing the built-in template's counts (which is
/// what Config::load_default's soft fallback returns).
#[test]
fn test_config_show_surfaces_unparsable_active_config() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join("config.toml"),
        "this is not [ valid toml {{{",
    )?;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_project-indicator"))
        .args(["config", "show"])
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("fails to parse"),
        "config show must surface the parse failure, got: {stdout}"
    );
    assert!(
        !stdout.contains("Active Configuration"),
        "config show must not report template counts as the active config, got: {stdout}"
    );
    Ok(())
}

/// A broken config at a path RANKED BELOW the default save location (e.g.
/// ~/.project-indicator.toml) is outranked, not shadowed: 'config init'
/// creates the default file, which takes precedence and fixes detection.
/// The hint must say that, not claim the broken file shadows the default.
#[test]
fn test_validate_hint_for_outranked_config() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    fs::write(
        home.path().join(".project-indicator.toml"),
        "this is not [ valid toml {{{",
    )?;
    // Run from an empty workdir: a cwd config would outrank everything and
    // turn this into the shadowing case.
    let workdir = home.path().join("work");
    fs::create_dir(&workdir)?;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_project-indicator"))
        .args(["config", "validate"])
        .current_dir(&workdir)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("xdg-config"))
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("shadows the default"),
        "a lower-priority config does not shadow the default, got: {stdout}"
    );
    assert!(
        stdout.contains("takes precedence"),
        "hint must say 'config init' creates a default that takes precedence, got: {stdout}"
    );
    Ok(())
}

/// Omitting fields of [detection]/[display] must not discard the config:
/// unknown keys fail loudly, missing keys take defaults.
#[test]
fn test_partial_sections_take_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");
    fs::write(
        &config_file,
        "[detection]\nmax_depth = 2\n\n[display]\nmax_frameworks = 5\n\n[[indicators]]\nname = \"T\"\nfiles = [\"t.file\"]\ncolor = \"#FF0000\"\nicon = \"T\"\npriority = 1\n",
    )?;

    let config = Config::load_from_file(&config_file)?;
    assert_eq!(config.detection.max_depth, 2);
    assert_eq!(config.detection.max_upward_traversal, 10);
    assert!(!config.detection.require_vcs_root);
    assert_eq!(config.display.max_frameworks, 5);
    assert!(config.display.show_frameworks);
    Ok(())
}

#[test]
fn test_removed_config_keys_fail_parse() -> Result<(), Box<dyn std::error::Error>> {
    // Removed schema keys must be rejected loudly, not silently ignored:
    // a stale config gets surfaced ('config validate' names the unknown
    // key) instead of half-applying. At prompt time Config::load_default
    // still soft-falls-back to the built-in template, so a stale config
    // can never break the shell (covered by
    // persistent_cache_tests::test_unparsable_config_soft_fallback).
    let indicators = r##"
[[indicators]]
name = "Test Language"
files = ["test.file"]
color = "#FF0000"
icon = "T"
priority = 1
"##;

    let cases = [
        (
            "framework_separator",
            format!(
                "[display]\nshow_frameworks = true\nmax_frameworks = 3\nframework_separator = \" | \"\n{indicators}"
            ),
        ),
        (
            "detection_mode",
            format!("[detection]\nmax_upward_traversal = 5\nrequire_vcs_root = false\ndetection_mode = \"Fast\"\n{indicators}"),
        ),
        (
            "confidence_threshold",
            format!("[detection]\nmax_upward_traversal = 5\nrequire_vcs_root = false\nconfidence_threshold = 0.3\n{indicators}"),
        ),
        (
            "max_matches_per_pattern",
            format!("[detection]\nmax_upward_traversal = 5\nrequire_vcs_root = false\nmax_matches_per_pattern = 15\n{indicators}"),
        ),
        (
            "root_indicators",
            format!("[detection]\nmax_upward_traversal = 5\nrequire_vcs_root = false\n\n[[detection.root_indicators]]\npattern = \".git\"\nweight = 1.0\n{indicators}"),
        ),
        (
            "files",
            format!(
                "{indicators}\n[[frameworks]]\nname = \"F\"\necosystems = [\"npm\"]\npriority = 1\nfiles = [\"legacy.marker\"]\n\n[frameworks.detection]\ntype = \"Dependencies\"\ndependencies = [\"x\"]\n"
            ),
        ),
        // Unknown (never-existed) keys on the remaining strict structs:
        // top level, [meta], [[indicators]], and a root-indicator entry.
        (
            "legacy_cache",
            format!("legacy_cache = true\n{indicators}"),
        ),
        (
            "schema",
            format!("[meta]\nversion = \"3.0\"\nschema = \"v3\"\n{indicators}"),
        ),
        (
            "confidence",
            format!("{indicators}confidence = 0.9\n"),
        ),
        (
            "certainty",
            format!(
                "{indicators}\n[[indicators.root_indicators]]\npattern = \"test.file\"\nweight = 0.9\ncertainty = 0.9\n"
            ),
        ),
    ];

    for (removed_key, stale_config) in cases {
        let temp_dir = TempDir::new()?;
        let config_file = temp_dir.path().join("config.toml");
        fs::write(&config_file, &stale_config)?;

        let err = match Config::load_from_file(&config_file) {
            Err(err) => format!("{err:#}"),
            Ok(_) => panic!("config with removed key '{removed_key}' must fail to parse"),
        };
        assert!(
            err.contains(removed_key),
            "parse error for removed key '{removed_key}' should name it, got: {err}"
        );
    }

    Ok(())
}
