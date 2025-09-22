//! CLI integration tests

use clap::Parser;
use project_indicator::cli::*;

#[test]
fn test_cli_parsing_basic() {
    let cli = Cli::try_parse_from(["project-indicator"]).unwrap();

    assert_eq!(cli.path, None);
    assert_eq!(cli.format, "simple");
    assert!(!cli.frameworks_only);
    assert_eq!(cli.language, None);
    assert!(cli.command.is_none());
}

#[test]
fn test_cli_parsing_with_path() {
    let cli = Cli::try_parse_from(["project-indicator", "/some/path"]).unwrap();

    assert_eq!(cli.path, Some("/some/path".into()));
}

#[test]
fn test_cli_parsing_with_format() {
    let cli = Cli::try_parse_from(["project-indicator", "--format", "json"]).unwrap();

    assert_eq!(cli.format, "json");
}

#[test]
fn test_cli_parsing_frameworks_only() {
    let cli = Cli::try_parse_from(["project-indicator", "--frameworks-only"]).unwrap();

    assert!(cli.frameworks_only);
}

#[test]
fn test_cli_parsing_force_language() {
    let cli = Cli::try_parse_from(["project-indicator", "--language", "rust"]).unwrap();

    assert_eq!(cli.language, Some("rust".to_string()));
}

#[test]
fn test_cli_config_subcommand() {
    let cli = Cli::try_parse_from(["project-indicator", "config", "validate"]).unwrap();

    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Validate,
        }) => {}
        _ => panic!("Expected Config::Validate command"),
    }
}

#[test]
fn test_cli_debug_subcommand() {
    let cli = Cli::try_parse_from(["project-indicator", "debug", "--verbose"]).unwrap();

    match cli.command {
        Some(Commands::Debug { verbose: true }) => {}
        _ => panic!("Expected Debug command with verbose=true"),
    }
}

#[test]
fn test_cli_cache_subcommand() {
    let cli = Cli::try_parse_from(["project-indicator", "cache", "clear"]).unwrap();

    match cli.command {
        Some(Commands::Cache {
            action: CacheAction::Clear,
        }) => {}
        _ => panic!("Expected Cache::Clear command"),
    }
}

#[test]
fn test_cli_benchmark_subcommand() {
    let cli = Cli::try_parse_from(["project-indicator", "benchmark"]).unwrap();

    match cli.command {
        Some(Commands::Benchmark) => {}
        _ => panic!("Expected Benchmark command"),
    }
}
