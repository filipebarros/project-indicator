//! Deterministic file scanning: a name-sorted, gitignore-aware walk bounded
//! by a match cap and a size-scaled timeout. Root-level manifests are always
//! kept ahead of the cap, and root-level files are topped up if a busy
//! subdirectory exhausts it.
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
//! // Scan a directory for matching files; `truncated` is true when the
//! // timeout cut the walk short
//! let path = Path::new("/path/to/project");
//! let outcome = engine.scan_with_status(path)?;
//!
//! println!(
//!     "Found {} matching files (truncated: {})",
//!     outcome.matches.len(),
//!     outcome.truncated
//! );
//! # Ok(())
//! # }
//! ```

pub mod scanning_engine;
pub mod timeout_manager;
pub mod traverser;

pub use scanning_engine::{ScanOutcome, ScanningEngine};
pub use timeout_manager::TimeoutManager;
