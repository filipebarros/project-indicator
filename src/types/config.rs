//! Configuration types for project detection behavior.
//!
//! This module contains all configuration-related types that control how the detection
//! engine operates, including detection modes, thresholds, display settings, and caching.
//!
//! # Overview
//!
//! The configuration system is organized into several focused types:
//!
//! - [`DetectionConfig`] - Core detection behavior (depth, confidence, root indicators)
//! - [`DisplayConfig`] - Output formatting and display preferences
//! - [`CacheConfig`] - Cache behavior and performance tuning
//! - [`ConfigMeta`] - Configuration file metadata and versioning
//! - [`DetectionMode`] - Fast vs Thorough detection strategies
//!
//! # Detection Modes
//!
//! The engine supports two detection modes:
//!
//! - **Fast**: Quick detection using priority files only (Cargo.toml, package.json, etc.)
//! - **Thorough**: Full directory traversal with comprehensive pattern matching (default)
//!
//! # Example
//!
//! ```rust
//! use project_indicator::types::{DetectionConfig, DetectionMode, DisplayConfig, CacheConfig};
//!
//! // Create custom detection configuration
//! let mut config = DetectionConfig::default();
//! config.max_depth = 3;
//! config.confidence_threshold = 0.5;
//! config.detection_mode = DetectionMode::Fast;
//!
//! // Customize display settings
//! let display = DisplayConfig {
//!     show_frameworks: true,
//!     max_frameworks: 3,
//!     framework_separator: " + ".to_string(),
//! };
//!
//! // Configure caching
//! let cache = CacheConfig {
//!     enabled: true,
//!     max_entries: 500,
//!     ttl_seconds: 600,
//! };
//! ```
//!
//! # Configuration Hierarchy
//!
//! Configurations are typically loaded in this order:
//!
//! 1. **Built-in defaults** - Sensible defaults for all settings
//! 2. **Config file** - User's `~/.config/project-indicator/config.toml`
//! 3. **CLI arguments** - Command-line overrides (highest priority)
//!
//! # Serialization
//!
//! All config types implement [`Serialize`] and [`Deserialize`] for TOML persistence:
//!
//! ```rust
//! use project_indicator::types::DetectionConfig;
//!
//! let config = DetectionConfig::default();
//! let toml_string = toml::to_string(&config).unwrap();
//! let parsed: DetectionConfig = toml::from_str(&toml_string).unwrap();
//! assert_eq!(config, parsed);
//! ```

use crate::types::RootIndicator;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetectionMode {
    Fast,
    Thorough,
}

impl Default for DetectionMode {
    fn default() -> Self {
        Self::Thorough
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionConfig {
    pub max_upward_traversal: usize,
    pub require_vcs_root: bool,
    pub confidence_threshold: f32,
    pub root_indicators: Vec<RootIndicator>,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub detection_mode: DetectionMode,

    // Scanning thresholds - configurable magic numbers
    #[serde(default = "default_max_matches")]
    pub max_matches_per_pattern: usize,
    #[serde(default = "default_small_project_threshold")]
    pub small_project_threshold: usize,
    #[serde(default = "default_extreme_size_threshold")]
    pub extreme_size_threshold: usize,
}

fn default_max_depth() -> usize {
    1
}

fn default_max_matches() -> usize {
    15
}

fn default_small_project_threshold() -> usize {
    50
}

fn default_extreme_size_threshold() -> usize {
    500
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            root_indicators: vec![],
            max_depth: default_max_depth(),
            detection_mode: DetectionMode::default(),
            max_matches_per_pattern: default_max_matches(),
            small_project_threshold: default_small_project_threshold(),
            extreme_size_threshold: default_extreme_size_threshold(),
        }
    }
}

impl DetectionConfig {
    pub fn all_root_indicators(&self) -> Vec<&RootIndicator> {
        self.root_indicators.iter().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    pub show_frameworks: bool,
    pub max_frameworks: usize,
    pub framework_separator: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_frameworks: true,
            max_frameworks: 2,
            framework_separator: "+".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            ttl_seconds: 300,
        }
    }
}

/// Configuration for result tracking and monitoring
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TrackingConfig {
    /// Enable tracking of detection results
    #[serde(default)]
    pub enabled: bool,
    /// Custom storage path (defaults to platform cache directory if not specified)
    #[serde(default)]
    pub storage_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigMeta {
    pub version: String,
}

impl Default for ConfigMeta {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
        }
    }
}
