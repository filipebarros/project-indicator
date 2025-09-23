//! Project Indicator - Fast project type and framework detection
//!
//! A high-performance CLI tool for detecting programming languages and frameworks
//! in projects, designed for shell prompts and scripting.

pub mod cli;
pub mod config;
pub mod detection;
pub mod output;
pub mod patterns;
pub mod types;

// Re-export main types for convenience
pub use types::{
    DetectionResult, DetectionType, FrameworkDetector, FrameworkMatch, ProjectIndicator,
};

pub use config::Config;
pub use detection::DetectionEngine;

/// The main library result type
pub type Result<T> = std::result::Result<T, anyhow::Error>;
