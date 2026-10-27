use project_indicator::{
    cli::ConfigAction,
    config::{built_in_config, parser, validate_config},
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

/// Whether `config_path` is where `config edit`/`config init` write; a
/// failing file elsewhere is a fallback location the default outranks.
fn is_default_location(config_path: &std::path::Path) -> bool {
    parser::default_save_path().is_ok_and(|default| default == config_path)
}

fn ensure_parent_dir(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create config directory: {}", e))?;
    }
    Ok(())
}

pub fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Validate => {
            // Validate the file detection actually reads (same resolution
            // order as parser::load_default), and parse it directly —
            // load_default's soft fallback would swallow the parse error
            // this command exists to surface.
            let Some(config_path) = parser::active_config_path() else {
                outln!("❌ No configuration file found");
                outln!("💡 Run 'project-indicator config init' to create a default configuration");
                std::process::exit(1);
            };

            match parser::load_from_file(&config_path) {
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
                        if is_default_location(&config_path) {
                            outln!("💡 Run 'project-indicator config edit' to fix the issues")
                        } else {
                            outln!(
                                "💡 Fix this file directly — 'config edit' opens the default \
                                 config location, which is a different file"
                            )
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
                    // failing file sits in the resolution order: a fallback
                    // location is outranked by the default.
                    if is_default_location(&config_path) {
                        outln!(
                            "💡 Fix the file ('config edit') or regenerate it ('config init --force'); \
                             until then, detection uses the built-in template"
                        )
                    } else {
                        outln!(
                            "💡 Fix this file, or run 'config init' to create the default config, \
                             which takes precedence over it; until then, detection uses the \
                             built-in template"
                        )
                    }
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Show => {
            outln!("🔍 Project Indicator Configuration Discovery\n");

            // Same paths, same order, as the parser (first existing wins)
            let config_paths = parser::default_config_paths();
            let active_path = parser::active_config_path();

            for path in &config_paths {
                if path.exists() {
                    let marker = if Some(path) == active_path.as_ref() {
                        " (active)"
                    } else {
                        " (shadowed)"
                    };
                    outln!("✅ Found{}: {}", marker, path.display());

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
                    outln!("❌ Not found: {}", path.display());
                    outln!();
                }
            }

            if let Some(active) = active_path {
                // Parse the active file directly — load_default's soft
                // fallback would mask a parse error and report the built-in
                // template's counts as if they came from this file.
                match parser::load_from_file(&active) {
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
            let config_path = parser::default_save_path()?;
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
                anyhow::bail!(
                    "EDITOR environment variable is not set; set it to your editor (for example, export EDITOR=nano)"
                );
            }
            Ok(())
        }
        ConfigAction::Init { force } => {
            let config_path = parser::default_save_path()?;

            if config_path.exists() && !force {
                outln!(
                    "❗ Configuration file already exists at: {}",
                    config_path.display()
                );
                outln!("💡 Use --force to overwrite");
                std::process::exit(1);
            }

            let config = built_in_config();

            ensure_parent_dir(&config_path)?;

            let config_content = toml::to_string_pretty(&config)
                .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

            std::fs::write(&config_path, config_content)
                .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;

            outln!("✅ Configuration created successfully!");
            outln!("📍 Location: {}", config_path.display());
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
    fn test_editor_path_with_spaces_is_kept_whole() -> project_indicator::Result<()> {
        let dir = tempfile::tempdir()?;
        let spaced = dir.path().join("my ed");
        std::fs::write(&spaced, "")?;
        let spaced = spaced.to_string_lossy().to_string();
        assert_eq!(split_editor(&spaced), (spaced.clone(), vec![]));
        Ok(())
    }
}
