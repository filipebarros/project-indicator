use project_indicator::{
    cli::Cli,
    config::Config,
    detection::DetectionEngine,
    output::{OutputFormat, OutputFormatter},
    Result,
};

pub fn handle_debug_command(cli: &Cli, verbose: bool) -> Result<()> {
    let path = super::resolve_and_validate_path(cli.path.as_ref())?;

    println!("Debug mode for path: {}", path.display());

    let config = Config::load_default()?;
    println!(
        "Configuration loaded from: {:?}",
        Config::get_config_path()?
    );

    if verbose {
        println!("Indicators: {}", config.indicators.len());
        println!("Frameworks: {}", config.frameworks.len());
    }

    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );
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
            "  - Indicators detected: {}",
            if result.indicator.is_some() { 1 } else { 0 }
        );
        println!("  - Frameworks detected: {}", result.frameworks.len());
        println!(
            "  - Best framework: {:?}",
            result.best_framework().map(|f| &f.framework.name)
        );
    }

    Ok(())
}
