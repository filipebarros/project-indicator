use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "project-indicator")]
#[command(about = "A fast project type and framework detection tool")]
#[command(version)]
pub struct Cli {
    pub path: Option<PathBuf>,

    #[arg(
        long,
        default_value = "simple",
        help = "Output format: simple, full, json, debug"
    )]
    pub format: String,

    #[arg(
        long,
        help = "Maximum depth to scan directories (default: 1; 0 scans nothing)"
    )]
    pub max_depth: Option<usize>,

    #[arg(short, long, help = "Enable verbose logging output")]
    pub verbose: bool,

    #[arg(long, help = "Skip the persistent result cache for this invocation")]
    pub no_cache: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Inspect, validate, or generate the configuration file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage the persistent detection result cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Parse the active config file and report the exact error if any
    Validate,
    /// Open the default config file in $EDITOR
    Edit,
    /// List config file locations and the active configuration
    Show,
    /// Write a config file from a built-in template
    Init {
        #[arg(long, help = "Use a specific template")]
        template: Option<String>,
        #[arg(long, help = "Force overwrite existing config")]
        force: bool,
        #[arg(
            long,
            help = "Specify config file path (default: $XDG_CONFIG_HOME/project-indicator/config.toml, or ~/.config/...)"
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_cli_no_cache_flag() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "--no-cache"])?;
        assert!(cli.no_cache);
        let cli = Cli::try_parse_from(["project-indicator"])?;
        assert!(!cli.no_cache);
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
            "cache",
            "stats",
        ])?;

        assert_eq!(cli.path, Some(PathBuf::from("/custom/path")));
        assert_eq!(cli.format, "full");
        assert!(matches!(
            cli.command,
            Some(Commands::Cache {
                action: CacheAction::Stats
            })
        ));
        Ok(())
    }
}
