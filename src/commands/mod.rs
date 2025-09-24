pub mod benchmark;
pub mod cache;
pub mod config;
pub mod debug;
pub mod detect;
pub mod root_indicators;

pub use benchmark::handle_benchmark_command;
pub use cache::handle_cache_command;
pub use config::handle_config_command;
pub use debug::handle_debug_command;
pub use detect::handle_detect_command;
pub use root_indicators::handle_root_indicators_command;

use project_indicator::Result;
use std::env;
use std::path::PathBuf;

pub fn resolve_and_validate_path(path_input: Option<&PathBuf>) -> Result<PathBuf> {
    let path = if let Some(provided_path) = path_input {
        match provided_path.canonicalize() {
            Ok(canonical_path) => canonical_path,
            Err(_) => {
                if !provided_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Path does not exist: {}",
                        provided_path.display()
                    ));
                }
                provided_path.clone()
            }
        }
    } else {
        env::current_dir().map_err(|e| anyhow::anyhow!("Cannot access current directory: {}", e))?
    };

    if !path.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path is not a directory: {}",
            path.display()
        ));
    }

    Ok(path)
}
