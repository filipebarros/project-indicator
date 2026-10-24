/// `println!` that ignores write errors: a prompt may close stdout early
/// (`| head`), and a broken pipe must not panic.
macro_rules! outln {
    () => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout());
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

pub mod cache;
pub mod config;
pub mod detect;

pub use cache::handle_cache_command;
pub use config::handle_config_command;
pub use detect::handle_detect_command;

use project_indicator::Result;
use std::env;
use std::path::PathBuf;

pub fn resolve_and_validate_path(path_input: Option<&PathBuf>) -> Result<PathBuf> {
    let path = match path_input {
        Some(p) => p.clone(),
        None => env::current_dir()
            .map_err(|e| anyhow::anyhow!("Cannot access current directory: {}", e))?,
    };
    // Canonicalize in BOTH branches: the engine compares against a
    // canonical home dir, and on Windows a canonical path carries a `\\?\`
    // prefix a plain current_dir() lacks, so they would never be equal
    // (A successful canonicalize already proves the path exists)
    let path = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) if !path.exists() => {
            return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
        }
        Err(_) => path,
    };

    if !path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path is not a directory: {}",
            path.display()
        ));
    }

    Ok(path)
}
