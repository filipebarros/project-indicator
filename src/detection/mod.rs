pub mod caches;
pub mod confidence_scorer;
pub mod conflict_resolver;
pub mod engine;
pub mod framework_detector;
pub mod language_resolver;
pub mod matchers;
pub mod pattern_compiler;
pub mod pattern_matching;
pub mod pattern_processor;
pub mod root_indicators;
pub mod scanner;

pub use engine::{DetectionEngine, DetectionEngineBuilder};
