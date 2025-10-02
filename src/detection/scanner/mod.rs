//! File scanning module with adaptive performance optimization
//!
//! This module provides efficient file system scanning with:
//! - Adaptive timeout management
//! - Sequential vs parallel scanning based on project size
//! - Priority file fast path
//! - Early termination heuristics
//! - GitIgnore support
//!
//! # Performance Optimizations
//!
//! - **Small projects (< 50 files):** Sequential scan avoids thread overhead
//! - **Large projects (≥ 50 files):** Parallel scan leverages multiple cores
//! - **Arc pattern sharing:** Eliminates Vec cloning in parallel workers
//! - **Pre-allocated capacity:** Reduces memory reallocations
//! - **Arc::try_unwrap:** Avoids final Vec clone when possible
//!
//! # Example
//!
//! ```no_run
//! use project_indicator::detection::scanner::ScanningEngine;
//! use project_indicator::detection::pattern_matching::PatternMatcher;
//! use project_indicator::detection::pattern_processor::PatternProcessor;
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Setup pattern matching infrastructure
//! let pattern_matcher = Arc::new(PatternMatcher::new());
//! let patterns = Arc::new(vec!["*.rs".to_string(), "Cargo.toml".to_string()]);
//! let pattern_processor = PatternProcessor::new(pattern_matcher, patterns, vec![]);
//!
//! // Create scanning engine with max depth of 3
//! let engine = ScanningEngine::new(pattern_processor, 3);
//!
//! // Scan a directory for matching files
//! let path = Path::new("/path/to/project");
//! let matches = engine.scan(path)?;
//!
//! println!("Found {} matching files", matches.len());
//! # Ok(())
//! # }
//! ```

pub mod scanning_engine;
pub mod timeout_manager;
pub mod traverser;

pub use scanning_engine::ScanningEngine;
pub use timeout_manager::TimeoutManager;
pub use traverser::FileSystemTraverser;
