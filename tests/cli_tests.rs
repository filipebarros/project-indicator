use clap::Parser;
use project_indicator::cli::*;

#[test]
fn test_cli_parsing_basic() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator"])?;

    assert_eq!(cli.path, None);
    assert_eq!(cli.format, "simple");
    assert!(cli.command.is_none());
    Ok(())
}

#[test]
fn test_cli_parsing_with_path() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator", "/some/path"])?;

    assert_eq!(cli.path, Some("/some/path".into()));
    Ok(())
}

#[test]
fn test_cli_parsing_with_format() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator", "--format", "json"])?;

    assert_eq!(cli.format, "json");
    Ok(())
}

#[test]
fn test_cli_config_subcommand() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator", "config", "validate"])?;

    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Validate,
        }) => {}
        _ => panic!("Expected Config::Validate command"),
    }
    Ok(())
}

#[test]
fn test_cli_debug_subcommand() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator", "debug", "--verbose"])?;

    match cli.command {
        Some(Commands::Debug { verbose: true }) => {}
        _ => panic!("Expected Debug command with verbose=true"),
    }
    Ok(())
}

#[test]
fn test_cli_benchmark_subcommand() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(["project-indicator", "benchmark"])?;

    match cli.command {
        Some(Commands::Benchmark) => {}
        _ => panic!("Expected Benchmark command"),
    }
    Ok(())
}
