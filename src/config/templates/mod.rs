mod frameworks;
mod languages;
pub mod shared;

use crate::config::Config;

/// The built-in configuration: every supported language and framework.
/// Used when no config file exists (or the existing one fails to parse) and
/// written verbatim by `config init`.
pub fn built_in_config() -> Config {
    Config {
        frameworks: frameworks::framework_catalog(),
        indicators: languages::language_indicators(),
        ..Config::default()
    }
}
