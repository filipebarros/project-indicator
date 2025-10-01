pub mod common;
pub mod dependency_matcher;
pub mod ecosystems;

#[cfg(test)]
pub mod test_helpers;

pub use common::{calculate_dependency_confidence, sort_framework_matches};
pub use dependency_matcher::DependencyMatcher;
