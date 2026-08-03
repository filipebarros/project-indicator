use project_indicator::{
    cli::Cli,
    config::Config,
    detection::{DetectionEngine, DetectionEngineBuilder},
    output::{OutputFormat, OutputFormatter},
    Result,
};
use std::time::Instant;

fn setup_benchmark(cli: &Cli) -> Result<(std::path::PathBuf, Config, DetectionEngine)> {
    let path = super::resolve_and_validate_path(cli.path.as_ref())?;

    let config = Config::load_default()?;

    let engine = DetectionEngineBuilder::new(config.indicators.clone(), config.frameworks.clone())
        .with_config(config.detection.clone())
        .build();

    Ok((path, config, engine))
}

pub fn handle_benchmark_command(cli: &Cli) -> Result<()> {
    println!("Performance Benchmark");
    println!("====================");

    let (path, config, engine) = setup_benchmark(cli)?;

    println!("Benchmarking path: {}", path.display());
    println!("Indicators configured: {}", config.indicators.len());
    println!();

    println!("1. Single Detection (Cold)");
    let start = Instant::now();
    let result = engine.detect(&path)?;
    let cold_duration = start.elapsed();
    println!("   Time: {:?}", cold_duration);
    println!(
        "   Result: {} indicator, {} frameworks",
        if result.indicator.is_some() { 1 } else { 0 },
        result.frameworks.len()
    );
    println!();

    println!("2. Single Detection (Warm)");
    let start = Instant::now();
    let _result = engine.detect(&path)?;
    let warm_duration = start.elapsed();
    println!("   Time: {:?}", warm_duration);
    println!(
        "   Improvement: {:.2}x",
        cold_duration.as_nanos() as f64 / warm_duration.as_nanos() as f64
    );
    println!();

    println!("3. Rapid Detection (Shell Prompt Simulation)");
    let iterations = 10;

    let start = Instant::now();
    for _ in 0..iterations {
        let _result = engine.detect(&path)?;
    }
    let total = start.elapsed();

    println!(
        "   {} iterations: {:?} (avg: {:?})",
        iterations,
        total,
        total / iterations
    );
    println!();

    println!("4. Output Formatting");
    let display_config = config.display;
    let formatter = OutputFormatter::new(display_config);

    let formats = [
        ("Simple", OutputFormat::Simple),
        ("Full", OutputFormat::Full),
        ("JSON", OutputFormat::Json),
        ("Compact", OutputFormat::Compact),
        ("Debug", OutputFormat::Debug),
    ];

    for (name, format) in &formats {
        let start = Instant::now();
        for _ in 0..1000 {
            let _output = formatter.format(&result, format.clone());
        }
        let format_duration = start.elapsed();
        println!(
            "   {} format (1000x): {:?} (avg: {:?})",
            name,
            format_duration,
            format_duration / 1000
        );
    }
    println!();

    println!("Performance Recommendations:");

    if cold_duration.as_millis() < 10 {
        println!("✓ Detection speed is excellent (< 10ms)");
    } else if cold_duration.as_millis() < 50 {
        println!("✓ Detection speed is good (< 50ms)");
    } else {
        println!("⚠ Detection speed could be improved (> 50ms)");
    }

    Ok(())
}
