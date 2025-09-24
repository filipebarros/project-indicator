use clap::Parser;
use project_indicator::{
    cli::{CacheAction, Cli, Commands, ConfigAction},
    config::Config,
    detection::{CachedDetection, DetectionCache, DetectionEngine},
    output::{OutputFormat, OutputFormatter},
    Result,
};
use std::env;
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { action }) => handle_config_command(action),
        Some(Commands::Debug { verbose }) => handle_debug_command(&cli, verbose),
        Some(Commands::Benchmark) => handle_benchmark_command(&cli),
        Some(Commands::Cache { action }) => handle_cache_command(action),
        None => handle_detect_command(&cli),
    }
}

fn handle_detect_command(cli: &Cli) -> Result<()> {
    // Determine the path to analyze with proper error handling
    let path = if let Some(provided_path) = &cli.path {
        // Validate that the provided path exists and is accessible
        if !provided_path.exists() {
            return Err(anyhow::anyhow!(
                "Path does not exist: {}",
                provided_path.display()
            ));
        }
        provided_path.clone()
    } else {
        // Fallback to current directory with proper error handling
        env::current_dir().map_err(|e| anyhow::anyhow!("Cannot access current directory: {}", e))?
    };

    // Load configuration
    let config = Config::load_default()?;

    // Create detection engine with configuration
    let engine = DetectionEngine::with_config(config.languages, config.detection);

    // Run detection (root discovery controlled by configuration)
    let result = engine.detect(&path)?;

    // Parse output format
    let format: OutputFormat = cli
        .format
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid format: {}", e))?;

    // Create formatter with config display settings and theme
    let display_config = config.display;
    let theme = project_indicator::output::themes::create_theme();
    let formatter = OutputFormatter::new(display_config, theme);

    // Format and output result
    let output = formatter.format(&result, format);
    println!("{}", output);

    Ok(())
}

fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Validate => match Config::load_default() {
            Ok(_) => {
                println!("Configuration is valid");
                Ok(())
            }
            Err(e) => {
                eprintln!("Configuration validation failed: {}", e);
                std::process::exit(1);
            }
        },
        ConfigAction::Edit => {
            let config_path = Config::get_config_path()?;
            println!("Configuration file location: {}", config_path.display());

            // Try to open with editor
            if let Ok(editor) = env::var("EDITOR") {
                std::process::Command::new(editor)
                    .arg(&config_path)
                    .status()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor: {}", e))?;
            } else {
                println!("Set EDITOR environment variable to edit configuration");
            }
            Ok(())
        }
    }
}

fn handle_debug_command(cli: &Cli, verbose: bool) -> Result<()> {
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

    println!("Debug mode for path: {}", path.display());

    // Load configuration
    let config = Config::load_default()?;
    println!(
        "Configuration loaded from: {:?}",
        Config::get_config_path()?
    );

    if verbose {
        println!("Languages: {}", config.languages.len());
        println!("Frameworks: {}", config.frameworks().len());
    }

    // Create detection engine with configuration
    let engine = DetectionEngine::with_config(config.languages.clone(), config.detection.clone());

    // Run detection
    let result = engine.detect(&path)?;

    // Always use debug format for debug command
    let display_config = config.display;
    let theme = project_indicator::output::themes::create_theme();
    let formatter = OutputFormatter::new(display_config, theme);

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

fn setup_benchmark(
    cli: &Cli,
) -> Result<(std::path::PathBuf, Config, DetectionEngine, DetectionCache)> {
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
    let engine = DetectionEngine::with_config(config.languages.clone(), config.detection.clone());
    let cache = DetectionCache::new(config.cache.clone());

    Ok((path, config, engine, cache))
}

fn handle_benchmark_command(cli: &Cli) -> Result<()> {
    println!("Performance Benchmark");
    println!("====================");

    let (path, config, engine, cache) = setup_benchmark(cli)?;

    println!("Benchmarking path: {}", path.display());
    println!("Languages configured: {}", config.languages.len());
    println!();

    // Benchmark 1: Single detection (cold)
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

    // Benchmark 2: Single detection (warm - same data)
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

    // Benchmark 3: Cached detection
    println!("3. Cached Detection");

    // First run to populate cache
    let start = Instant::now();
    let _result = engine.detect_cached(&path, &cache)?;
    let cache_populate_duration = start.elapsed();

    // Second run from cache
    let start = Instant::now();
    let _result = engine.detect_cached(&path, &cache)?;
    let cache_hit_duration = start.elapsed();

    println!("   Cache populate: {:?}", cache_populate_duration);
    println!("   Cache hit: {:?}", cache_hit_duration);
    println!(
        "   Cache improvement: {:.2}x",
        cache_populate_duration.as_nanos() as f64 / cache_hit_duration.as_nanos() as f64
    );
    println!();

    // Benchmark 4: Multiple rapid detections (simulating shell prompt usage)
    println!("4. Rapid Detection (Shell Prompt Simulation)");
    let iterations = 10;

    // Without cache
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = engine.detect(&path)?;
    }
    let uncached_total = start.elapsed();

    // With cache
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = engine.detect_cached(&path, &cache)?;
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
    println!();

    // Benchmark 5: Output formatting
    println!("5. Output Formatting");
    let display_config = config.display;
    let theme = project_indicator::output::themes::create_theme();
    let formatter = OutputFormatter::new(display_config, theme);

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

    // Cache statistics
    println!("6. Cache Statistics");
    let stats = cache.stats();
    println!("   Cache entries: {}", stats.entries);
    println!("   Cache hits: {}", stats.hits);
    println!("   Cache misses: {}", stats.misses);
    println!();

    // Performance recommendations
    println!("Performance Recommendations:");
    if cache_hit_duration.as_micros() < 100 {
        println!("✓ Cache performance is excellent (< 100μs)");
    } else if cache_hit_duration.as_micros() < 1000 {
        println!("✓ Cache performance is good (< 1ms)");
    } else {
        println!("⚠ Cache performance could be improved (> 1ms)");
    }

    if cold_duration.as_millis() < 10 {
        println!("✓ Detection speed is excellent (< 10ms)");
    } else if cold_duration.as_millis() < 50 {
        println!("✓ Detection speed is good (< 50ms)");
    } else {
        println!("⚠ Detection speed could be improved (> 50ms)");
    }

    Ok(())
}

fn handle_cache_command(action: CacheAction) -> Result<()> {
    let config = Config::load_default()?;
    let cache = DetectionCache::new(config.cache);

    match action {
        CacheAction::Clear => {
            cache.clear();
            println!("✅ Cache cleared successfully");
            Ok(())
        }
        CacheAction::Stats => {
            let stats = cache.stats();
            println!("Cache Statistics");
            println!("================");
            println!("📊 Entries: {}", stats.entries);
            println!("✅ Cache hits: {}", stats.hits);
            println!("❌ Cache misses: {}", stats.misses);
            println!("🔄 Invalidations: {}", stats.invalidations);

            if stats.hits + stats.misses > 0 {
                let hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64 * 100.0;
                println!("📈 Hit rate: {:.1}%", hit_rate);
            }

            Ok(())
        }
    }
}
