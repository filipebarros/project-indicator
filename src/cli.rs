//! Command-line interface

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Project Indicator - Fast project type and framework detection
#[derive(Parser)]
#[command(name = "project-indicator")]
#[command(about = "A fast project type and framework detection tool")]
pub struct Cli {
    /// The directory to analyze (defaults to current directory)
    pub path: Option<PathBuf>,

    /// Output format
    #[arg(long, default_value = "simple")]
    pub format: String,

    /// Show only frameworks, not languages
    #[arg(long)]
    pub frameworks_only: bool,

    /// Force detection of a specific language
    #[arg(long)]
    pub language: Option<String>,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Development and debugging tools
    Debug {
        /// Show verbose detection information
        #[arg(long)]
        verbose: bool,
    },
    /// Performance benchmarking
    Benchmark,
    /// Clear detection cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Validate configuration file
    Validate,
    /// Edit configuration file
    Edit,
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Clear the detection cache
    Clear,
    /// Show cache statistics
    Stats,
}
