use project_indicator::{
    cli::ConfigAction,
    config::{validate_config, Config, TemplateGenerator},
    Result,
};
use std::env;

/// Validates the EDITOR environment variable for security
fn validate_editor(editor: &str) -> Result<()> {
    // Check for shell injection attempts
    if editor.contains(';') || editor.contains('&') || editor.contains('|') {
        anyhow::bail!("Invalid EDITOR: contains shell operators (;, &, |). Please use a direct path to an editor binary.");
    }

    // Known safe editors
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

    // Extract base command name (handle paths and arguments)
    let base_cmd = editor.split_whitespace().next().unwrap_or(editor);
    let base_name = std::path::Path::new(base_cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base_cmd);

    // Check if it's a known editor
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
            let config_path = Config::get_config_path()?;

            if !config_path.exists() {
                println!(
                    "❌ No configuration file found at: {}",
                    config_path.display()
                );
                println!(
                    "💡 Run 'project-indicator config init' to create a default configuration"
                );
                std::process::exit(1);
            }

            match Config::load_default() {
                Ok(config) => match validate_config(&config) {
                    Ok(()) => {
                        println!("✅ Configuration is valid");
                        println!("📍 Config file: {}", config_path.display());
                        println!("📊 Languages configured: {}", config.languages.len());
                        println!("🏗️  Total frameworks: {}", config.frameworks().len());
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
                    println!("❌ Failed to load configuration:");
                    println!("   {}", load_error);
                    println!("📍 Config file: {}", config_path.display());
                    println!("💡 Check the file format and syntax");
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Show => {
            println!("🔍 Project Indicator Configuration Discovery\n");

            let config_paths = vec![
                (
                    "Current directory",
                    std::path::PathBuf::from("./project-indicator.toml"),
                ),
                (
                    "XDG config home",
                    dirs::config_dir()
                        .map(|p| p.join("project-indicator").join("config.toml"))
                        .unwrap_or_else(|| std::path::PathBuf::from("")),
                ),
                (
                    "Home directory",
                    dirs::home_dir()
                        .map(|p| p.join(".project-indicator.toml"))
                        .unwrap_or_else(|| std::path::PathBuf::from("")),
                ),
            ];

            let mut found_any = false;

            for (location, path) in &config_paths {
                if path.exists() {
                    println!("✅ Found: {}", location);
                    println!("   📍 {}", path.display());

                    if let Ok(metadata) = std::fs::metadata(path) {
                        println!("   📊 Size: {} bytes", metadata.len());
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = modified.elapsed() {
                                println!("   🕐 Modified: {} seconds ago", duration.as_secs());
                            }
                        }
                    }

                    found_any = true;
                    println!();
                } else {
                    println!("❌ Not found: {}", location);
                    println!("   📍 {}", path.display());
                    println!();
                }
            }

            if found_any {
                match Config::load_default() {
                    Ok(config) => {
                        println!("📦 Active Configuration:");
                        println!("   Languages: {}", config.languages.len());
                        println!("   Total frameworks: {}", config.frameworks().len());
                        println!("   Detection mode: {:?}", config.detection.detection_mode);
                        println!();
                        println!(
                            "💡 Run 'project-indicator config validate' to check configuration"
                        );
                    }
                    Err(e) => {
                        println!("⚠️  Found config files but failed to load:");
                        println!("   {}", e);
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
            println!("🔧 Languages: {}", config.languages.len());

            if let Err(e) = validate_config(&config) {
                println!("⚠️  Warning: Generated config has validation issues: {}", e);
            }

            println!("💡 Run 'project-indicator config validate' to verify your configuration");
            Ok(())
        }
    }
}
