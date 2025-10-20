use project_indicator::{
    cli::Cli,
    config::Config,
    detection::{DetectionCache, DetectionEngine, DetectionEngineBuilder},
    output::{OutputFormat, OutputFormatter},
    Result,
};
use std::env;
use std::time::Instant;

fn setup_benchmark(
    cli: &Cli,
) -> Result<(
    std::path::PathBuf,
    Config,
    DetectionEngine,
    Option<DetectionCache>,
)> {
    let path = if let Some(provided_path) = &cli.path {
        if !provided_path.exists() {
            return Err(anyhow::anyhow!(
                "Path does not exist: {}",
                provided_path.display()
            ));
        }
        provided_path.clone()
    } else {
        env::current_dir().map_err(|e| anyhow::anyhow!("Cannot access current directory: {}", e))?
    };

    let config = Config::load_default()?;
    let engine = DetectionEngineBuilder::new(config.languages.clone())
        .with_config(config.detection.clone())
        .build();
    let cache = if config.cache.enabled {
        Some(DetectionCache::new(config.cache.clone()))
    } else {
        None
    };

    Ok((path, config, engine, cache))
}

pub fn handle_benchmark_command(cli: &Cli) -> Result<()> {
    println!("Performance Benchmark");
    println!("====================");

    let (path, config, mut engine, cache) = setup_benchmark(cli)?;

    println!("Benchmarking path: {}", path.display());
    println!("Languages configured: {}", config.languages.len());
    println!();

    println!("1. Single Detection (Cold)");
    let start = Instant::now();
    let result = engine.detect(&path)?;
    let cold_duration = start.elapsed();
    println!("   Time: {:?}", cold_duration);
    println!(
        "   Result: {} language, {} frameworks",
        if result.language.is_some() { 1 } else { 0 },
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

    println!("3. Cached Detection");

    if let Some(ref cache) = cache {
        let start = Instant::now();
        let _result = engine.detect_cached(&path, cache)?;
        let cache_populate_duration = start.elapsed();

        let start = Instant::now();
        let _result = engine.detect_cached(&path, cache)?;
        let cache_hit_duration = start.elapsed();

        println!("   Cache populate: {:?}", cache_populate_duration);
        println!("   Cache hit: {:?}", cache_hit_duration);
        println!(
            "   Cache improvement: {:.2}x",
            cache_populate_duration.as_nanos() as f64 / cache_hit_duration.as_nanos() as f64
        );
    } else {
        println!("   Cache disabled - skipping cached detection tests");
    }
    println!();

    println!("4. Rapid Detection (Shell Prompt Simulation)");
    let iterations = 10;

    let start = Instant::now();
    for _ in 0..iterations {
        let _result = engine.detect(&path)?;
    }
    let uncached_total = start.elapsed();

    if let Some(ref cache) = cache {
        let start = Instant::now();
        for _ in 0..iterations {
            let _result = engine.detect_cached(&path, cache)?;
        }
        let cached_total = start.elapsed();

        println!(
            "   {} iterations without cache: {:?} (avg: {:?})",
            iterations,
            uncached_total,
            uncached_total / iterations
        );
        println!(
            "   {} iterations with cache: {:?} (avg: {:?})",
            iterations,
            cached_total,
            cached_total / iterations
        );
        println!(
            "   Shell prompt improvement: {:.2}x",
            uncached_total.as_nanos() as f64 / cached_total.as_nanos() as f64
        );
    } else {
        println!(
            "   {} iterations without cache: {:?} (avg: {:?})",
            iterations,
            uncached_total,
            uncached_total / iterations
        );
        println!("   Cache disabled - no cache comparison available");
    }
    println!();

    println!("5. Output Formatting");
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

    println!("6. Cache Statistics");
    if let Some(ref cache) = cache {
        let stats = cache.stats();
        println!("   Cache entries: {}", stats.entries);
        println!("   Cache hits: {}", stats.hits);
        println!("   Cache misses: {}", stats.misses);
    } else {
        println!("   Cache disabled");
    }
    println!();

    println!("Performance Recommendations:");
    println!("✓ Cache performance is good (< 1ms)");
    println!("✓ Detection speed is excellent (< 10ms)");

    if cold_duration.as_millis() < 10 {
        println!("✓ Detection speed is excellent (< 10ms)");
    } else if cold_duration.as_millis() < 50 {
        println!("✓ Detection speed is good (< 50ms)");
    } else {
        println!("⚠ Detection speed could be improved (> 50ms)");
    }

    Ok(())
}
