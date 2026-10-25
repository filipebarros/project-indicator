use crate::output::OutputFormat;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "project-indicator")]
#[command(about = "A fast project type and framework detection tool")]
#[command(version)]
pub struct Cli {
    /// Directory to inspect (default: the current directory)
    pub path: Option<PathBuf>,

    /// Output format: `simple` is one colored glyph for prompts
    #[arg(long, value_enum, ignore_case = true, default_value = "simple")]
    pub format: OutputFormat,

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

#[derive(Subcommand, Clone, Copy)]
pub enum ConfigAction {
    /// Parse the active config file and report the exact error if any
    Validate,
    /// Open the default config file in $EDITOR
    Edit,
    /// List config file locations and the active configuration
    Show,
    /// Write the built-in configuration to a config file
    Init {
        #[arg(long, help = "Force overwrite existing config")]
        force: bool,
    },
}

#[derive(Subcommand, Clone, Copy)]
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
    fn test_format_is_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "--format", "JSON"])?;
        assert_eq!(cli.format, OutputFormat::Json);
        Ok(())
    }

    #[test]
    fn test_unknown_format_error_lists_the_valid_ones() {
        // the removed "compact" must be rejected, naming what is accepted
        let err = match Cli::try_parse_from(["project-indicator", "--format", "compact"]) {
            Err(err) => err.to_string(),
            Ok(_) => panic!("removed format must not parse"),
        };
        assert!(err.contains("simple") && err.contains("json"), "{err}");
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
    fn test_cli_config_init_rejects_the_removed_template_flag() {
        assert!(Cli::try_parse_from([
            "project-indicator",
            "config",
            "init",
            "--template",
            "web-dev",
        ])
        .is_err());
    }

    #[test]
    fn test_cli_config_init_with_force() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "config", "init", "--force"])?;
        assert!(matches!(
            cli.command,
            Some(Commands::Config {
                action: ConfigAction::Init { force: true }
            })
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
        assert_eq!(cli.format, OutputFormat::Full);
        assert!(matches!(
            cli.command,
            Some(Commands::Cache {
                action: CacheAction::Stats
            })
        ));
        Ok(())
    }
}
