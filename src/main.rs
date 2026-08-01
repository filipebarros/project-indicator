use clap::Parser;
use project_indicator::{
    cli::{Cli, Commands},
    Result,
};

mod commands;

use commands::{
    handle_benchmark_command, handle_config_command, handle_debug_command, handle_detect_command,
    handle_diff_command, handle_history_command, handle_root_indicators_command,
    handle_stats_command,
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
        Some(Commands::RootIndicators { ref action }) => {
            handle_root_indicators_command(&cli, action)
        }
        Some(Commands::History {
            path,
            limit,
            changes_only,
        }) => handle_history_command(path, limit, changes_only),
        Some(Commands::Diff { from, to }) => handle_diff_command(from, to),
        Some(Commands::Stats { path, since }) => handle_stats_command(path, since),
        None => handle_detect_command(&cli),
    }
}
