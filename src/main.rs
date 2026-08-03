use clap::Parser;
use project_indicator::{
    cli::{Cli, Commands},
    Result,
};

mod commands;

use commands::{
    handle_benchmark_command, handle_cache_command, handle_config_command, handle_debug_command,
    handle_detect_command, handle_root_indicators_command,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }

    match cli.command {
        Some(Commands::Config { action }) => handle_config_command(action),
        Some(Commands::Debug { verbose }) => handle_debug_command(&cli, verbose),
        Some(Commands::Benchmark) => handle_benchmark_command(&cli),
        Some(Commands::Cache { action }) => handle_cache_command(action),
        Some(Commands::RootIndicators { ref action }) => {
            handle_root_indicators_command(&cli, action)
        }
        None => handle_detect_command(&cli),
    }
}
