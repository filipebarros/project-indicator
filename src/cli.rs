use crate::output::OutputFormat;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "project-indicator")]
#[command(about = "A fast project type and framework detection tool")]
#[command(version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Directory to inspect (default: the current directory)
    pub path: Option<PathBuf>,

    /// Output format: `simple` is one colored glyph for prompts
    #[arg(long, value_enum, ignore_case = true, default_value = "simple")]
    pub format: OutputFormat,

    #[arg(short, long, global = true, help = "Enable verbose logging output")]
    pub verbose: bool,

    #[arg(long, help = "Skip the persistent result cache for this invocation")]
    pub no_cache: bool,

    #[arg(
        long,
        value_name = "SECONDS",
        conflicts_with = "no_cache",
        help = "Serve recent cache entries without revalidating watched paths"
    )]
    pub cache_ttl: Option<u64>,

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
    fn test_cli_cache_ttl_is_opt_in_and_conflicts_with_bypass() {
        let cli = Cli::try_parse_from(["project-indicator", "--cache-ttl", "30"])
            .unwrap_or_else(|error| panic!("valid TTL failed to parse: {error}"));
        assert_eq!(cli.cache_ttl, Some(30));
        assert!(
            Cli::try_parse_from(["project-indicator"]).is_ok_and(|cli| { cli.cache_ttl.is_none() })
        );
        assert!(
            Cli::try_parse_from(["project-indicator", "--cache-ttl", "30", "--no-cache"]).is_err()
        );
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
    fn test_management_subcommands_reject_detection_arguments() {
        for args in [
            &["project-indicator", "/custom/path", "cache", "stats"][..],
            &["project-indicator", "--format", "json", "cache", "stats"],
            &["project-indicator", "--no-cache", "config", "show"],
        ] {
            assert!(Cli::try_parse_from(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn test_verbose_still_works_with_subcommands() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["project-indicator", "cache", "stats", "-v"])?;
        assert!(cli.verbose);
        Ok(())
    }
}
