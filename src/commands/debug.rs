use project_indicator::{
    cli::Cli,
    config::Config,
    detection::DetectionEngineBuilder,
    output::{OutputFormat, OutputFormatter},
    tracking::ResultTracker,
    Result,
};
use std::sync::Arc;

pub fn handle_debug_command(cli: &Cli, verbose: bool) -> Result<()> {
    let path = super::resolve_and_validate_path(cli.path.as_ref())?;

    println!("Debug mode for path: {}", path.display());

    let config = Config::load_default()?;
    println!(
        "Configuration loaded from: {:?}",
        Config::get_config_path()?
    );

    if verbose {
        println!("Languages: {}", config.languages.len());
        println!("Frameworks: {}", config.frameworks().len());
    }

    // Create tracker from config (respects user's tracking settings)
    let tracker = Arc::new(ResultTracker::from_config(&config.tracking)?);

    let engine = DetectionEngineBuilder::new(config.languages.clone())
        .with_config(config.detection.clone())
        .with_result_tracker(tracker)
        .build();
    let result = engine.detect(&path)?;

    let display_config = config.display;
    let formatter = OutputFormatter::new(display_config);

    let output = formatter.format(&result, OutputFormat::Debug);
    println!("\nDetection Results:");
    println!("{}", output);

    if verbose {
        println!("\nAdditional Debug Information:");
        println!("  - Detection confidence: {:.2}", result.confidence);
        println!(
            "  - Languages detected: {}",
            if result.language.is_some() { 1 } else { 0 }
        );
        println!("  - Frameworks detected: {}", result.frameworks.len());
        println!(
            "  - Best framework: {:?}",
            result.best_framework().map(|f| &f.framework.name)
        );
    }

    Ok(())
}
