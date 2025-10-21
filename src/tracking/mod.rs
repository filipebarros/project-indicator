//! Result tracking and change detection system.
//!
//! This module provides tools for tracking detection results over time,
//! comparing results, and detecting changes in project structure.

pub mod comparison;
pub mod formatting;
pub mod path_cache;
pub mod storage;
pub mod types;
pub mod utils;

pub use comparison::*;
pub use formatting::*;
pub use path_cache::PathCache;
pub use storage::ResultTracker;
pub use types::*;
pub use utils::*;
