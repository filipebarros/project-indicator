use project_indicator::{
    cli::ConfigAction,
    config::{validate_config, ConfigParser, TemplateGenerator},
    Result,
};
use std::env;

/// Split `$EDITOR` into program and arguments (`code --wait` → `code`,
/// `["--wait"]`). A value that is itself an existing file — a path that
/// contains spaces — is taken whole as the program.
fn split_editor(editor: &str) -> (String, Vec<String>) {
    if std::path::Path::new(editor).is_file() {
        return (editor.to_string(), Vec::new());
    }
    let mut parts = editor.split_whitespace().map(str::to_string);
    let program = parts.next().unwrap_or_else(|| editor.to_string());
    (program, parts.collect())
}

/// Where a config file sits relative to the default save path in the
/// resolution order.
enum Relation {
    IsDefault,
    /// Earlier in the order, so it outranks the default
    Shadows,
    /// Later in the order (or unknown), so the default outranks it
    Shadowed,
}

fn relation_to_default(config_path: &std::path::Path) -> Relation {
    let paths = ConfigParser::default_config_paths();
    let position = |p: &std::path::Path| paths.iter().position(|c| c == p);
    let default_pos = ConfigParser::default_save_path()
        .ok()
        .and_then(|p| position(&p));
    match (position(config_path), default_pos) {
        (Some(a), Some(d)) if a == d => Relation::IsDefault,
        (Some(a), Some(d)) if a < d => Relation::Shadows,
        _ => Relation::Shadowed,
    }
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create config directory: {}", e))?;
    }
    Ok(())
}

fn print_templates() {
    for (name, description) in TemplateGenerator::list_templates() {
        outln!("   • {}: {}", name, description);
    }
}

pub fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Validate => {
            // Validate the file detection actually reads (same resolution
            // order as ConfigParser::load_default), and parse it directly —
            // load_default's soft fallback would swallow the parse error
            // this command exists to surface.
            let Some(config_path) = ConfigParser::active_config_path() else {
                outln!("❌ No configuration file found");
                outln!("💡 Run 'project-indicator config init' to create a default configuration");
                std::process::exit(1);
            };

            match ConfigParser::load_from_file(&config_path) {
                Ok(config) => match validate_config(&config) {
                    Ok(()) => {
                        outln!("✅ Configuration is valid");
                        outln!("📍 Config file: {}", config_path.display());
                        outln!("📊 Indicators configured: {}", config.indicators.len());
                        outln!("🏗️  Total frameworks: {}", config.frameworks.len());
                        Ok(())
                    }
                    Err(validation_error) => {
                        outln!("❌ Configuration validation failed:");
                        outln!("   {}", validation_error);
                        outln!("📍 Config file: {}", config_path.display());
                        match relation_to_default(&config_path) {
                            Relation::IsDefault => {
                                outln!("💡 Run 'project-indicator config edit' to fix the issues")
                            }
                            _ => outln!(
                                "💡 Fix this file directly — 'config edit' opens the default \
                                 config location, which is a different file"
                            ),
                        }
                        std::process::exit(1);
                    }
                },
                Err(load_error) => {
                    outln!("❌ Failed to parse configuration:");
                    outln!("   {:#}", load_error);
                    outln!("📍 Config file: {}", config_path.display());
                    // 'config edit' and 'config init --force' both target the
                    // default XDG path. Whether that helps depends on where the
                    // failing file sits in the resolution order: a cwd file
                    // outranks (shadows) the default, a home-dir fallback is
                    // outranked by it.
                    match relation_to_default(&config_path) {
                        Relation::IsDefault => outln!(
                            "💡 Fix the file ('config edit') or regenerate it ('config init --force'); \
                             until then, detection uses the built-in template"
                        ),
                        Relation::Shadows => outln!(
                            "💡 Fix this file directly — it shadows the default config location, so \
                             'config edit'/'config init' won't touch it; until then, detection uses \
                             the built-in template"
                        ),
                        Relation::Shadowed => outln!(
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
            outln!("🔍 Project Indicator Configuration Discovery\n");

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
                    outln!("✅ Found{}: {}", marker, absolute(path).display());

                    if let Ok(metadata) = std::fs::metadata(path) {
                        outln!("   📊 Size: {} bytes", metadata.len());
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = modified.elapsed() {
                                outln!("   🕐 Modified: {} seconds ago", duration.as_secs());
                            }
                        }
                    }

                    outln!();
                } else {
                    outln!("❌ Not found: {}", absolute(path).display());
                    outln!();
                }
            }

            if let Some(active) = active_path {
                // Parse the active file directly — load_default's soft
                // fallback would mask a parse error and report the built-in
                // template's counts as if they came from this file.
                match ConfigParser::load_from_file(&active) {
                    Ok(config) => {
                        outln!("📦 Active Configuration:");
                        outln!("   Indicators: {}", config.indicators.len());
                        outln!("   Total frameworks: {}", config.frameworks.len());
                        outln!();
                        outln!("💡 Run 'project-indicator config validate' to check configuration");
                    }
                    Err(e) => {
                        outln!(
                            "⚠️  Active config fails to parse; detection uses the built-in template:"
                        );
                        outln!("   {:#}", e);
                        outln!("💡 Run 'project-indicator config validate' for details");
                    }
                }
            } else {
                outln!("ℹ️  No configuration files found. Using fallback configuration.");
                outln!("💡 Run 'project-indicator config init' to create a configuration file");
            }

            Ok(())
        }
        ConfigAction::Edit => {
            let config_path = ConfigParser::default_save_path()?;
            outln!("📍 Configuration file location: {}", config_path.display());

            if let Ok(editor) = env::var("EDITOR") {
                let (program, editor_args) = split_editor(&editor);
                ensure_parent_dir(&config_path)?;
                let status = std::process::Command::new(&program)
                    .args(&editor_args)
                    .arg(&config_path)
                    .status()
                    .map_err(|e| anyhow::anyhow!("Failed to open editor: {}", e))?;
                if !status.success() {
                    anyhow::bail!("Editor exited with {}", status);
                }
            } else {
                outln!("❗ Set EDITOR environment variable to edit configuration");
                outln!("💡 Example: export EDITOR=nano");
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
                ConfigParser::default_save_path()?
            };

            if config_path.is_dir() {
                anyhow::bail!(
                    "--path must be a file, but {} is a directory",
                    config_path.display()
                );
            }

            if config_path.exists() && !force {
                outln!(
                    "❗ Configuration file already exists at: {}",
                    config_path.display()
                );
                outln!("💡 Use --force to overwrite or try a different template");
                outln!("💡 Available templates:");
                print_templates();
                std::process::exit(1);
            }

            if template.is_none() {
                outln!("📋 Available templates:");
                print_templates();
                outln!("💡 Use --template <name> to select a template, or run without to use 'minimal'");
                outln!();
            }

            let config = TemplateGenerator::generate_template(template.as_deref())?;

            ensure_parent_dir(&config_path)?;

            let config_content = toml::to_string_pretty(&config)
                .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

            std::fs::write(&config_path, config_content)
                .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;

            let template_name = template.as_deref().unwrap_or("minimal");
            outln!("✅ Configuration created successfully!");
            outln!("📍 Location: {}", config_path.display());
            outln!("📋 Template: {}", template_name);
            outln!("🔧 Indicators: {}", config.indicators.len());

            if let Err(e) = validate_config(&config) {
                outln!("⚠️  Warning: Generated config has validation issues: {}", e);
            }

            outln!("💡 Run 'project-indicator config validate' to verify your configuration");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_editor;

    #[test]
    fn test_editor_with_arguments_is_split() {
        assert_eq!(
            split_editor("code --wait"),
            ("code".to_string(), vec!["--wait".to_string()])
        );
        assert_eq!(split_editor("vim"), ("vim".to_string(), vec![]));
    }

    #[test]
    fn test_editor_path_with_spaces_is_kept_whole() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let spaced = dir.path().join("my ed");
        std::fs::write(&spaced, "").unwrap_or_else(|e| panic!("write: {e}"));
        let spaced = spaced.to_string_lossy().to_string();
        assert_eq!(split_editor(&spaced), (spaced.clone(), vec![]));
    }
}
