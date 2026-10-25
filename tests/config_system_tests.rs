mod common;

use project_indicator::config::parser;
use std::fs;
use tempfile::TempDir;
#[test]
fn test_config_file_loading_with_detection() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");

    let config_content = "[display]
max_frameworks = 3

[detection]
require_vcs_root = true

[[indicators]]
name = \"Test Language\"
files = [\"test.file\"]
color = \"#FF0000\"
icon = \"🧪\"
";

    fs::write(&config_file, config_content)?;

    let config = parser::load_from_file(&config_file)?;

    assert!(config.detection.require_vcs_root);

    assert_eq!(config.display.max_frameworks, 3);
    assert_eq!(config.indicators.len(), 1);
    assert_eq!(config.indicators[0].name, "Test Language");

    Ok(())
}
/// 'config show' must check the same paths the parser reads, mark the
/// first-existing one active and later ones shadowed, and report the active
/// config's contents.
#[test]
fn test_config_show_marks_active_and_shadowed_configs() -> Result<(), Box<dyn std::error::Error>> {
    let valid_config = r##"
[[indicators]]
name = "Test Language"
files = ["test.file"]
color = "#FF0000"
icon = "T"
"##;
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    let xdg_config = config_dir.join("config.toml");
    fs::write(&xdg_config, valid_config)?;
    let home_config = home.path().join(".project-indicator.toml");
    fs::write(&home_config, valid_config)?;

    let output = common::cli(home.path()).args(["config", "show"]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let active_line = stdout
        .lines()
        .find(|l| l.contains("(active)"))
        .unwrap_or_default();
    assert!(
        active_line.contains(&xdg_config.display().to_string()),
        "the XDG config must be marked active, got: {stdout}"
    );
    assert!(
        stdout.contains("(shadowed)") && stdout.contains(&home_config.display().to_string()),
        "the outranked home config must be listed as shadowed, got: {stdout}"
    );
    assert!(
        stdout.contains("Active Configuration") && stdout.contains("Indicators: 1"),
        "show must report the active config's contents, got: {stdout}"
    );
    Ok(())
}

/// A repository chooses what sits in its own directory, so a config file
/// there is never read: `cd`-ing into a clone must not change your prompt.
#[test]
fn test_a_config_file_in_the_current_directory_is_ignored() -> Result<(), Box<dyn std::error::Error>>
{
    let home = TempDir::new()?;
    let workdir = home.path().join("work");
    fs::create_dir(&workdir)?;
    for name in ["project-indicator.toml", ".project-indicator.toml"] {
        fs::write(workdir.join(name), "this is not [ valid toml {{{")?;
    }

    let output = common::cli(home.path())
        .args(["config", "show"])
        .current_dir(&workdir)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("(active)"), "{stdout}");
    assert!(!stdout.contains("work"), "{stdout}");
    Ok(())
}

/// When the active config exists but fails to parse, 'config show' must say
/// so instead of silently printing the built-in template's counts (which is
/// what parser::load_default's soft fallback returns).
#[test]
fn test_config_show_surfaces_unparsable_active_config() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join("config.toml"),
        "this is not [ valid toml {{{",
    )?;

    let output = common::cli(home.path())
        .args(["config", "show"])
        .current_dir(home.path())
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

/// A config that PARSES but fails validation (an invalid hex color) must
/// soft-fall back to the built-in configuration at detection time, exactly
/// like an unparsable one — garbage values must never reach the engine.
#[test]
fn test_invalid_config_falls_back_to_the_built_in() -> Result<(), Box<dyn std::error::Error>> {
    let home = TempDir::new()?;
    let config_dir = home.path().join("xdg-config").join("project-indicator");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join("config.toml"),
        // parses under the current schema, but the color fails validation
        "[[indicators]]\nname = \"Broken\"\nfiles = [\"*.zzz\"]\ncolor = \"not-a-color\"\nicon = \"x\"\n",
    )?;
    let project = home.path().join("proj");
    fs::create_dir_all(&project)?;
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"x\"\n")?;

    let output = common::cli(home.path())
        .args(["--no-cache", "--format", "json"])
        .arg(&project)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"indicator\":\"Rust\"") || stdout.contains("\"indicator\": \"Rust\""),
        "detection must fall back to the built-in configuration, got: {stdout}"
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

    let output = common::cli(home.path())
        .args(["config", "validate"])
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
        "[detection]\nrequire_vcs_root = true\n\n[display]\n\n[[indicators]]\nname = \"T\"\nfiles = [\"t.file\"]\ncolor = \"#FF0000\"\nicon = \"T\"\n",
    )?;

    let config = parser::load_from_file(&config_file)?;
    assert!(config.detection.require_vcs_root);
    assert_eq!(config.display.max_frameworks, 5);
    Ok(())
}

/// An empty [display] section must give the same effective max_frameworks as
/// having no config file at all (the built-in configuration).
#[test]
fn test_empty_display_section_matches_the_built_in_max_frameworks(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");
    fs::write(
        &config_file,
        "[display]\n\n[[indicators]]\nname = \"T\"\nfiles = [\"t.file\"]\ncolor = \"#FF0000\"\nicon = \"T\"\n",
    )?;

    let config = parser::load_from_file(&config_file)?;
    assert_eq!(
        config.display.max_frameworks,
        project_indicator::config::built_in_config()
            .display
            .max_frameworks
    );
    assert_eq!(config.display.max_frameworks, 5);
    Ok(())
}

fn framework_with_detection(detection: &str) -> String {
    format!(
        r##"
[[indicators]]
name = "Test Language"
files = ["test.file"]
color = "#FF0000"
icon = "T"

[[frameworks]]
name = "Poetry"
ecosystems = ["pypi"]
priority = 1

[frameworks.detection]
{detection}
"##
    )
}

fn parse_error(config: &str) -> Result<String, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");
    fs::write(&config_file, config)?;
    match parser::load_from_file(&config_file) {
        Err(err) => Ok(format!("{err:#}")),
        Ok(_) => panic!("config must fail to parse: {config}"),
    }
}

/// `configfile` is not a detection method: a config using one must fail
/// loudly, naming the unknown variant.
#[test]
fn test_unknown_detection_method_fails_parse() -> Result<(), Box<dyn std::error::Error>> {
    let err = parse_error(&framework_with_detection("configfile = \"pyproject.toml\""))?;
    assert!(err.contains("configfile"), "error should name it: {err}");
    Ok(())
}

/// The pre-v7 internally-tagged shape (`type = "..."` plus the payload key)
/// must fail loudly: `[frameworks.detection]` now holds exactly the one key
/// that names the method.
#[test]
fn test_old_internally_tagged_detection_shape_fails_parse() -> Result<(), Box<dyn std::error::Error>>
{
    let err = parse_error(&framework_with_detection(
        "type = \"Dependencies\"\ndependencies = [\"react\"]",
    ))?;
    assert!(
        err.contains("exactly 1 element"),
        "error should name the one-key rule: {err}"
    );
    Ok(())
}

/// A stray key next to the method must fail parse, not be silently ignored —
/// the internally-tagged enum this replaced could not enforce that.
#[test]
fn test_unknown_key_inside_detection_fails_parse() -> Result<(), Box<dyn std::error::Error>> {
    let err = parse_error(&framework_with_detection(
        "dependencies = [\"react\"]\nmode = \"all\"",
    ))?;
    assert!(
        err.contains("exactly 1 element"),
        "error should name the one-key rule: {err}"
    );
    Ok(())
}

#[test]
fn test_removed_config_keys_fail_parse() -> Result<(), Box<dyn std::error::Error>> {
    // Removed schema keys must be rejected loudly, not silently ignored:
    // a stale config gets surfaced ('config validate' names the unknown
    // key) instead of half-applying. At prompt time parser::load_default
    // still soft-falls-back to the built-in template, so a stale config
    // can never break the shell (covered by
    // persistent_cache_tests::test_unparsable_config_soft_fallback).
    let indicators = r##"
[[indicators]]
name = "Test Language"
files = ["test.file"]
color = "#FF0000"
icon = "T"
"##;

    let cases = [
        (
            "framework_separator",
            format!(
                "[display]\nmax_frameworks = 3\nframework_separator = \" | \"\n{indicators}"
            ),
        ),
        (
            "detection_mode",
            format!("[detection]\nrequire_vcs_root = false\ndetection_mode = \"Fast\"\n{indicators}"),
        ),
        (
            "confidence_threshold",
            format!("[detection]\nrequire_vcs_root = false\nconfidence_threshold = 0.3\n{indicators}"),
        ),
        (
            "max_matches_per_pattern",
            format!("[detection]\nrequire_vcs_root = false\nmax_matches_per_pattern = 15\n{indicators}"),
        ),
        (
            "root_indicators",
            format!("[detection]\nrequire_vcs_root = false\n\n[[detection.root_indicators]]\npattern = \".git\"\nweight = 1.0\n{indicators}"),
        ),
        (
            "files",
            format!(
                "{indicators}\n[[frameworks]]\nname = \"F\"\necosystems = [\"npm\"]\npriority = 1\nfiles = [\"legacy.marker\"]\n\n[frameworks.detection]\ndependencies = [\"x\"]\n"
            ),
        ),
        (
            "max_depth",
            format!("[detection]\nmax_depth = 2\n{indicators}"),
        ),
        (
            "max_upward_traversal",
            format!("[detection]\nmax_upward_traversal = 5\n{indicators}"),
        ),
        (
            "nested_projects",
            format!("[detection]\nnested_projects = false\n{indicators}"),
        ),
        (
            "show_frameworks",
            format!("[display]\nshow_frameworks = true\n{indicators}"),
        ),
        (
            "meta",
            format!("[meta]\nversion = \"3.0\"\n{indicators}"),
        ),
        (
            "priority",
            format!("{indicators}priority = 1\n"),
        ),
        // Unknown (never-existed) keys on the remaining strict structs:
        // top level, [[indicators]], and a root-indicator entry.
        (
            "legacy_cache",
            format!("legacy_cache = true\n{indicators}"),
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

        let err = match parser::load_from_file(&config_file) {
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
