use project_indicator::{
    cli::ConfigAction,
    config::{validate_config, Config, ConfigParser, TemplateGenerator},
    Result,
};
use std::env;

/// Validates the EDITOR environment variable for security
fn validate_editor(editor: &str) -> Result<()> {
    if editor.contains(';') || editor.contains('&') || editor.contains('|') {
        anyhow::bail!("Invalid EDITOR: contains shell operators (;, &, |). Please use a direct path to an editor binary.");
    }

    const KNOWN_EDITORS: &[&str] = &[
        "vim",
        "nvim",
        "vi",
        "emacs",
        "emacsclient",
        "nano",
        "pico",
        "code",
        "code-insiders",
        "subl",
        "sublime_text",
        "atom",
        "gedit",
        "kate",
        "kwrite",
        "micro",
        "helix",
        "hx",
    ];

    let base_cmd = editor.split_whitespace().next().unwrap_or(editor);
    let base_name = std::path::Path::new(base_cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base_cmd);

    if !KNOWN_EDITORS.contains(&base_name) {
        log::warn!("⚠️  Unknown editor '{}'. Proceeding with caution.", editor);
        log::warn!("💡 Recommended editors: {}", KNOWN_EDITORS.join(", "));
        println!("⚠️  Warning: Using unknown editor '{}'", editor);
        println!("💡 Recommended: vim, nvim, emacs, nano, code, subl");
    }

    Ok(())
}

pub fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Validate => {
            // Validate the file detection actually reads (same resolution
            // order as Config::load_default), and parse it directly —
            // load_default's soft fallback would swallow the parse error
            // this command exists to surface.
            let Some(config_path) = ConfigParser::active_config_path() else {
                println!("❌ No configuration file found");
                println!(
                    "💡 Run 'project-indicator config init' to create a default configuration"
                );
                std::process::exit(1);
            };

            match Config::load_from_file(&config_path) {
                Ok(config) => match validate_config(&config) {
                    Ok(()) => {
                        println!("✅ Configuration is valid");
                        println!("📍 Config file: {}", config_path.display());
                        println!("📊 Indicators configured: {}", config.indicators.len());
                        println!("🏗️  Total frameworks: {}", config.frameworks.len());
                        Ok(())
                    }
                    Err(validation_error) => {
                        println!("❌ Configuration validation failed:");
                        println!("   {}", validation_error);
                        println!("📍 Config file: {}", config_path.display());
                        println!("💡 Run 'project-indicator config edit' to fix the issues");
                        std::process::exit(1);
                    }
                },
                Err(load_error) => {
                    println!("❌ Failed to parse configuration:");
                    println!("   {:#}", load_error);
                    println!("📍 Config file: {}", config_path.display());
                    // 'config edit' and 'config init --force' both target the
                    // default XDG path. Whether that helps depends on where the
                    // failing file sits in the resolution order: a cwd file
                    // outranks (shadows) the default, a home-dir fallback is
                    // outranked by it.
                    let paths = ConfigParser::default_config_paths();
                    let position = |p: &std::path::Path| paths.iter().position(|c| c == p);
                    let default_pos = ConfigParser::default_save_path()
                        .ok()
                        .and_then(|p| position(&p));
                    let active_pos = position(&config_path);
                    match (active_pos, default_pos) {
                        (Some(a), Some(d)) if a == d => println!(
                            "💡 Fix the file ('config edit') or regenerate it ('config init --force'); \
                             until then, detection uses the built-in template"
                        ),
                        (Some(a), Some(d)) if a < d => println!(
                            "💡 Fix this file directly — it shadows the default config location, so \
                             'config edit'/'config init' won't touch it; until then, detection uses \
                             the built-in template"
                        ),
                        _ => println!(
                            "💡 Fix this file, or run 'config init' to create the default config, \
                             which takes precedence over it; until then, detection uses the \
                             built-in template"
                        ),
                    }
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Show => {
            println!("🔍 Project Indicator Configuration Discovery\n");

            // Same paths, same order, as the parser (first existing wins)
            let config_paths = ConfigParser::default_config_paths();
            let active_path = ConfigParser::active_config_path();
            // The cwd entries are relative; print them absolute so the output
            // is meaningful outside the invoking shell
            let absolute = |path: &std::path::Path| {
                if path.is_relative() {
                    std::env::current_dir()
                        .map(|d| d.join(path))
                        .unwrap_or_else(|_| path.to_path_buf())
                } else {
                    path.to_path_buf()
                }
            };

            for path in &config_paths {
                if path.exists() {
                    let marker = if Some(path) == active_path.as_ref() {
                        " (active)"
                    } else {
                        " (shadowed)"
                    };
                    println!("✅ Found{}: {}", marker, absolute(path).display());

                    if let Ok(metadata) = std::fs::metadata(path) {
                        println!("   📊 Size: {} bytes", metadata.len());
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = modified.elapsed() {
                                println!("   🕐 Modified: {} seconds ago", duration.as_secs());
                            }
                        }
                    }

                    println!();
                } else {
                    println!("❌ Not found: {}", absolute(path).display());
                    println!();
                }
            }

            if let Some(active) = active_path {
                // Parse the active file directly — load_default's soft
                // fallback would mask a parse error and report the built-in
                // template's counts as if they came from this file.
                match Config::load_from_file(&active) {
                    Ok(config) => {
                        println!("📦 Active Configuration:");
                        println!("   Indicators: {}", config.indicators.len());
                        println!("   Total frameworks: {}", config.frameworks.len());
                        println!();
                        println!(
                            "💡 Run 'project-indicator config validate' to check configuration"
                        );
                    }
                    Err(e) => {
                        println!(
                            "⚠️  Active config fails to parse; detection uses the built-in template:"
                        );
                        println!("   {:#}", e);
                        println!("💡 Run 'project-indicator config validate' for details");
                    }
                }
            } else {
                println!("ℹ️  No configuration files found. Using fallback configuration.");
                println!("💡 Run 'project-indicator config init' to create a configuration file");
            }

            Ok(())
        }
        ConfigAction::Edit => {
            let config_path = Config::get_config_path()?;
            println!("📍 Configuration file location: {}", config_path.display());

            if let Ok(editor) = env::var("EDITOR") {
                validate_editor(&editor)?;

                std::process::Command::new(&editor)
                    .arg(&config_path)
                    .status()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor: {}", e))?;
            } else {
                println!("❗ Set EDITOR environment variable to edit configuration");
                println!("💡 Example: export EDITOR=nano");
            }
            Ok(())
        }
        ConfigAction::Init {
            template,
            force,
            path,
        } => {
            let config_path = if let Some(custom_path) = path {
                std::path::PathBuf::from(custom_path)
            } else {
                Config::get_config_path()?
            };

            if config_path.exists() && !force {
                println!(
                    "❗ Configuration file already exists at: {}",
                    config_path.display()
                );
                println!("💡 Use --force to overwrite or try a different template");
                println!("💡 Available templates:");
                for (name, description) in TemplateGenerator::list_templates() {
                    println!("   • {}: {}", name, description);
                }
                std::process::exit(1);
            }

            if template.is_none() {
                println!("📋 Available templates:");
                for (name, description) in TemplateGenerator::list_templates() {
                    println!("   • {}: {}", name, description);
                }
                println!("💡 Use --template <name> to select a template, or run without to use 'minimal'");
                println!();
            }

            let config = TemplateGenerator::generate_template(template.as_deref())?;

            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("Failed to create config directory: {}", e))?;
            }

            let config_content = toml::to_string_pretty(&config)
                .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

            std::fs::write(&config_path, config_content)
                .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;

            let template_name = template.as_deref().unwrap_or("minimal");
            println!("✅ Configuration created successfully!");
            println!("📍 Location: {}", config_path.display());
            println!("📋 Template: {}", template_name);
            println!("🔧 Indicators: {}", config.indicators.len());

            if let Err(e) = validate_config(&config) {
                println!("⚠️  Warning: Generated config has validation issues: {}", e);
            }

            println!("💡 Run 'project-indicator config validate' to verify your configuration");
            Ok(())
        }
    }
}
