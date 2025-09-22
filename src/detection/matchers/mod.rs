//! Package file matchers for framework detection

pub mod cargo_toml;
pub mod common;
pub mod composer_json;
pub mod gemfile;
pub mod go_mod;
pub mod package_json;
pub mod pyproject_toml;

#[cfg(test)]
pub mod test_helpers;

pub use cargo_toml::CargoTomlMatcher;
pub use common::{calculate_dependency_confidence, sort_framework_matches};
pub use composer_json::ComposerJsonMatcher;
pub use gemfile::GemfileMatcher;
pub use go_mod::GoModMatcher;
pub use package_json::PackageJsonMatcher;
pub use pyproject_toml::PyProjectTomlMatcher;
