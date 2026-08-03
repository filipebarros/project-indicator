use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "project-indicator")]
#[command(about = "A fast project type and framework detection tool")]
#[command(version)]
pub struct Cli {
    pub path: Option<PathBuf>,

    #[arg(long, default_value = "simple")]
    pub format: String,

    #[arg(long, help = "Maximum depth to scan directories (default: 3)")]
    pub max_depth: Option<usize>,

    #[arg(short, long, help = "Enable verbose logging output")]
    pub verbose: bool,

    #[arg(long, help = "Skip the persistent result cache for this invocation")]
    pub no_cache: bool,

    #[arg(
        long,
        help = "Detection mode: 'thorough' (default, full analysis) or 'fast' (early termination with secondary check)"
    )]
    pub mode: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Debug {
        #[arg(long)]
        verbose: bool,
    },
    Benchmark,
    /// Manage the persistent detection result cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    RootIndicators {
        #[command(subcommand)]
        action: RootIndicatorAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Validate,
    Edit,
    Show,
    Init {
        #[arg(long, help = "Use a specific template")]
        template: Option<String>,
        #[arg(long, help = "Force overwrite existing config")]
        force: bool,
        #[arg(
            long,
            help = "Specify config file path (default: system config directory)"
        )]
        path: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Delete all cached detection results
    Clear,
    /// Show cache entry count and disk usage
    Stats,
}

#[derive(Subcommand)]
pub enum RootIndicatorAction {
    Conflicts {
        #[arg(long, help = "Show detailed conflict analysis")]
        detailed: bool,
        #[arg(long, help = "Use legacy resolution strategy for comparison")]
        compare_legacy: bool,
        #[arg(long, help = "Show resolution strategies used")]
        show_strategies: bool,
    },
    List {
        #[arg(long, help = "Filter by indicator name")]
        indicator: Option<String>,
        #[arg(long, help = "Filter by framework name")]
        framework: Option<String>,
        #[arg(long, help = "Show only conflicting indicators")]
        conflicts_only: bool,
    },
    Validate {
        #[arg(long, help = "Check for common configuration issues")]
        strict: bool,
        #[arg(long, help = "Suggest improvements")]
        suggest: bool,
    },
    Stats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_values() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator"])?;
        assert_eq!(cli.path, None);
        assert_eq!(cli.format, "simple");
        assert!(cli.command.is_none());
        Ok(())
    }

    #[test]
    fn test_cli_with_path() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "/path/to/project"])?;
        assert_eq!(cli.path, Some(PathBuf::from("/path/to/project")));
        assert_eq!(cli.format, "simple");
        Ok(())
    }

    #[test]
    fn test_cli_with_format() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "--format", "json"])?;
        assert_eq!(cli.format, "json");
        Ok(())
    }

    #[test]
    fn test_cli_config_subcommand() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "config", "validate"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Validate
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_debug_subcommand() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "debug", "--verbose"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Debug { verbose: true })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_debug_subcommand_no_verbose() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "debug"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Debug { verbose: false })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_benchmark_subcommand() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "benchmark"])?;
        assert!(matches!(cli.command, Some(Commands::Benchmark)));
        Ok(())
    }

    #[test]
    fn test_cli_cache_clear() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "cache", "clear"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Cache {
                action: CacheAction::Clear
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_cache_stats() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "cache", "stats"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Cache {
                action: CacheAction::Stats
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_no_cache_flag() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "--no-cache"])?;
        assert!(cli.no_cache);
        let cli = Cli::try_parse_from(["project-indicator"])?;
        assert!(!cli.no_cache);
        Ok(())
    }

    #[test]
    fn test_cli_config_edit() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "config", "edit"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Edit
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_config_init() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "config", "init"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Init {
                    template: None,
                    force: false,
                    path: None
                }
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_config_init_with_template() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "project-indicator",
            "config",
            "init",
            "--template",
            "web-dev",
        ])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Init { template: Some(ref t), force: false, path: None }
            }) if t == "web-dev"
        ));
        Ok(())
    }

    #[test]
    fn test_cli_config_init_with_force() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "config", "init", "--force"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Init {
                    template: None,
                    force: true,
                    path: None
                }
            })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_config_init_with_path() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "project-indicator",
            "config",
            "init",
            "--path",
            "./my-config.toml",
        ])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Init {
                    template: None,
                    force: false,
                    path: Some(ref p)
                }
            }) if p == "./my-config.toml"
        ));
        Ok(())
    }

    #[test]
    fn test_cli_combined_options() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "project-indicator",
            "/custom/path",
            "--format",
            "full",
            "debug",
            "--verbose",
        ])?;

        assert_eq!(cli.path, Some(PathBuf::from("/custom/path")));
        assert_eq!(cli.format, "full");
        assert!(matches!(
            cli.command,
            Some(Commands::Debug { verbose: true })
        ));
        Ok(())
    }

    #[test]
    fn test_cli_help_text() -> Result<(), Box<dyn std::error::Error>> {
        let result = Cli::try_parse_from(["project-indicator", "--help"]);
        assert!(result.is_err());

        let result = Cli::try_parse_from(["project-indicator", "config", "--help"]);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_cli_version_flag() -> Result<(), Box<dyn std::error::Error>> {
        let result = Cli::try_parse_from(["project-indicator", "--version"]);
        // Version flag causes early exit, so try_parse_from will return an error
        // but it's a "DisplayVersion" error which is expected behavior
        assert!(result.is_err());
        Ok(())
    }
}
