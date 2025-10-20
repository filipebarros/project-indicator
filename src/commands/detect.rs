use project_indicator::{
    cli::Cli,
    config::Config,
    detection::DetectionEngineBuilder,
    output::{OutputFormat, OutputFormatter},
    types::DetectionMode,
    Result,
};

use super::resolve_and_validate_path;

pub fn handle_detect_command(cli: &Cli) -> Result<()> {
    let path = resolve_and_validate_path(cli.path.as_ref())?;

    let config = Config::load_default()?;
    let mut detection_config = config.detection;

    if let Some(max_depth) = cli.max_depth {
        detection_config.max_depth = max_depth;
    }

    if let Some(mode_str) = &cli.mode {
        detection_config.detection_mode = match mode_str.to_lowercase().as_str() {
            "fast" => DetectionMode::Fast,
            "thorough" => DetectionMode::Thorough,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid detection mode: '{}'. Valid options are 'fast' or 'thorough'",
                    mode_str
                ));
            }
        };
    }

    let engine = DetectionEngineBuilder::new(config.languages)
        .with_config(detection_config)
        .build();

    let result = engine.detect(&path)?;

    let format: OutputFormat = cli
        .format
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid format: {}", e))?;

    let display_config = config.display;
    let formatter = OutputFormatter::new(display_config);

    let output = formatter.format(&result, format);
    println!("{}", output);

    Ok(())
}
