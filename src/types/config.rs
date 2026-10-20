//! Configuration types for project detection behavior.
//!
//! This module contains all configuration-related types that control how the detection
//! engine operates, including thresholds, display settings, and caching.
//!
//! # Overview
//!
//! The configuration system is organized into several focused types:
//!
//! - [`DetectionConfig`] - Core detection behavior (depth, confidence)
//! - [`DisplayConfig`] - Output formatting and display preferences
//! - [`ConfigMeta`] - Configuration file metadata and versioning
//!
//! # Example
//!
//! ```rust
//! use project_indicator::types::{DetectionConfig, DisplayConfig};
//!
//! // Create custom detection configuration
//! let mut config = DetectionConfig::default();
//! config.max_depth = 3;
//! config.confidence_threshold = 0.5;
//!
//! // Customize display settings
//! let display = DisplayConfig {
//!     show_frameworks: true,
//!     max_frameworks: 3,
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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionConfig {
    pub max_upward_traversal: usize,
    pub require_vcs_root: bool,
    pub confidence_threshold: f32,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_max_depth() -> usize {
    1
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_upward_traversal: 10,
            require_vcs_root: false,
            confidence_threshold: 0.3,
            max_depth: default_max_depth(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayConfig {
    pub show_frameworks: bool,
    pub max_frameworks: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_frameworks: true,
            max_frameworks: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigMeta {
    pub version: String,
}

impl Default for ConfigMeta {
    fn default() -> Self {
        Self {
            version: "3.0".to_string(),
        }
    }
}
