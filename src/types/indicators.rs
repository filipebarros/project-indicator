//! Project and root indicator types.
//!
//! This module defines the core types for language detection and project
//! root discovery. Pattern matching itself lives in `crate::patterns`,
//! which matches file basenames against these patterns.
//!
//! # Overview
//!
//! The indicator system consists of three main components:
//!
//! - [`Indicator`] - Language definitions with file patterns and frameworks
//! - [`RootIndicator`] - Files that indicate a project root (Cargo.toml, package.json, etc.)
//! - [`IndicatorContext`] - Context classification for root indicators
//!
//! # Project Indicators
//!
//! A [`Indicator`] defines a programming language with:
//!
//! - **File patterns** - Glob patterns to match (*.rs, *.py, tsconfig.json)
//! - **Frameworks** - Associated framework detectors
//! - **Root indicators** - Files that strongly suggest this language
//! - **Priority** - For conflict resolution when multiple languages match
//! - **Visual identity** - Icon and color for display
//!
//! ## Example: Defining a Language
//!
//! ```rust
//! use project_indicator::types::{Ecosystem, Indicator};
//!
//! let typescript = Indicator::new(
//!     "TypeScript".to_string(),
//!     vec!["*.ts".to_string(), "tsconfig.json".to_string()],
//!     "#3178C6".to_string(),
//!     "TS".to_string(),
//!     1,
//!     vec![Ecosystem::Npm],
//! );
//! assert_eq!(typescript.priority, 1);
//! ```
//!
//! # Root Indicators
//!
//! [`RootIndicator`]s are files that strongly suggest a project root:
//!
//! - `Cargo.toml` (Rust) - weight 0.95
//! - `package.json` (Node.js) - weight 0.95
//! - `.git` (Version control) - weight 1.0
//! - `pyproject.toml` (Python) - weight 0.90
//!
//! ## Indicator Context
//!
//! Root indicators are classified by context:
//!
//! - **VersionControl** (1.0) - `.git`, `.hg` - Highest priority
//! - **LanguageRoot** (0.9) - `Cargo.toml`, `package.json` - Strong indicators
//! - **RuntimeRoot** (0.85) - `deno.json` - Runtime-specific root files
//! - **FrameworkRoot** (0.8) - Framework-specific config files
//! - **BuildSystem** (0.7) - `Makefile`, `CMakeLists.txt`
//! - **ToolchainRoot** (0.65) - `flake.nix`, `main.tf` - Infra/toolchain files
//! - **Configuration** (0.6) - Generic config files
//!
//! ```rust
//! use project_indicator::types::{RootIndicator, IndicatorContext};
//!
//! let cargo_indicator = RootIndicator {
//!     pattern: "Cargo.toml".to_string(),
//!     weight: 0.95,
//!     context: IndicatorContext::LanguageRoot,
//!     alternative_group: None,
//! };
//!
//! // Context determines base priority
//! assert_eq!(cargo_indicator.context.base_priority(), 0.9);
//! ```

use crate::types::Ecosystem;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum IndicatorContext {
    VersionControl,
    #[default]
    LanguageRoot,
    RuntimeRoot,
    ToolchainRoot,
    FrameworkRoot,
    BuildSystem,
    Configuration,
}

impl IndicatorContext {
    pub fn base_priority(&self) -> f32 {
        match self {
            IndicatorContext::VersionControl => 1.0,
            IndicatorContext::LanguageRoot => 0.9,
            IndicatorContext::RuntimeRoot => 0.85,
            IndicatorContext::FrameworkRoot => 0.8,
            IndicatorContext::BuildSystem => 0.7,
            IndicatorContext::ToolchainRoot => 0.65,
            IndicatorContext::Configuration => 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RootIndicator {
    pub pattern: String,
    pub weight: f32,
    #[serde(default)]
    pub context: IndicatorContext,
    /// Tags this root indicator as one of several mutually-exclusive
    /// alternatives (e.g. Java's pom.xml vs build.gradle — a real project
    /// uses exactly one build system). Root indicators sharing the same
    /// group contribute their max weight to confidence scoring instead of
    /// being summed. `None` (the default) preserves sum behavior, which is
    /// correct for co-occurring evidence (e.g. a manifest and its lockfile).
    #[serde(default)]
    pub alternative_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Indicator {
    pub name: String,
    pub files: Vec<String>,
    pub color: String,
    pub icon: String,
    pub priority: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ecosystems: Vec<Ecosystem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_indicators: Vec<RootIndicator>,
}

impl Default for Indicator {
    fn default() -> Self {
        Self {
            name: String::new(),
            files: Vec::new(),
            color: String::new(),
            icon: String::new(),
            priority: 1,
            ecosystems: Vec::new(),
            root_indicators: Vec::new(),
        }
    }
}

impl Indicator {
    pub fn new(
        name: String,
        files: Vec<String>,
        color: String,
        icon: String,
        priority: u8,
        ecosystems: Vec<Ecosystem>,
    ) -> Self {
        Self::with_root_indicators(name, files, color, icon, priority, ecosystems, Vec::new())
    }

    pub fn with_root_indicators(
        name: String,
        files: Vec<String>,
        color: String,
        icon: String,
        priority: u8,
        ecosystems: Vec<Ecosystem>,
        root_indicators: Vec<RootIndicator>,
    ) -> Self {
        Self {
            name,
            files,
            color,
            icon,
            priority,
            ecosystems,
            root_indicators,
        }
    }
}
