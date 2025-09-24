pub mod cache;
pub mod confidence_scorer;
pub mod conflict_resolver;
pub mod engine;
pub mod file_scanner;
pub mod framework_detector;
pub mod language_resolver;
pub mod matchers;
pub mod parsed_file_cache;
pub mod pattern_matching;
pub mod pattern_processor;
pub mod root_indicators;
pub mod scanning_engine;

pub use cache::{CachedDetection, DetectionCache};
pub use engine::DetectionEngine;
