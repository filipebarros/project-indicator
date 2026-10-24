//! File scanning module with adaptive performance optimization
//!
//! This module provides efficient, deterministic file system scanning:
//! - A sorted walk (stable results across runs and platforms)
//! - Root-level manifests always kept, ahead of the match cap
//! - Adaptive timeout management
//! - A match cap to bound memory and scan time
//! - GitIgnore support
//!
//! # Example
//!
//! ```no_run
//! use project_indicator::detection::scanner::ScanningEngine;
//! use project_indicator::detection::pattern_processor::PatternProcessor;
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let patterns = Arc::new(vec!["*.rs".to_string(), "Cargo.toml".to_string()]);
//! let pattern_processor = PatternProcessor::new(patterns);
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
