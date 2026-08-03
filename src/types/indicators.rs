//! Project and root indicator types with pattern matching capabilities.
//!
//! This module defines the core types for language detection and project root discovery,
//! including optimized pattern matching with regex compilation and caching.
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
//! ## Pattern Matching
//!
//! The module uses sophisticated pattern matching with:
//!
//! - **Wildcard support** - `*.rs`, `*.py`, `src/**/*.ts`
//! - **Exact matches** - `Cargo.toml`, `package.json`
//! - **Regex compilation** - Patterns are compiled once and cached in `OnceLock`
//! - **Performance optimization** - Thread-safe lazy compilation
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
//!
//! // Test pattern matching
//! let files = vec![
//!     "src/App.tsx".to_string(),
//!     "tsconfig.json".to_string(),
//! ];
//! assert!(typescript.matches_files(&files));
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
//! - **FrameworkRoot** (0.8) - Framework-specific config files
//! - **BuildSystem** (0.7) - `Makefile`, `CMakeLists.txt`
//! - **Configuration** (0.6) - Generic config files
//!
//! ```rust
//! use project_indicator::types::{RootIndicator, IndicatorContext};
//!
//! let cargo_indicator = RootIndicator {
//!     pattern: "Cargo.toml".to_string(),
//!     weight: 0.95,
//!     context: IndicatorContext::LanguageRoot,
//! };
//!
//! // Context determines base priority
//! assert_eq!(cargo_indicator.context.base_priority(), 0.9);
//! ```
//!
//! # Pattern Compilation & Caching
//!
//! For performance, patterns are compiled once and cached:
//!
//! ```rust
//! use project_indicator::types::Indicator;
//!
//! let rust = Indicator::new(
//!     "Rust".to_string(),
//!     vec!["*.rs".to_string(), "Cargo.toml".to_string()],
//!     "#DEA584".to_string(),
//!     "".to_string(),
//!     1,
//!     vec![],
//! );
//!
//! // First call compiles patterns and caches them
//! assert!(rust.matches_files(&["main.rs".to_string()]));
//!
//! // Subsequent calls use cached compiled patterns
//! assert!(rust.matches_files(&["lib.rs".to_string()]));
//! ```
//!
//! The `OnceLock<HashMap<String, Option<Regex>>>` ensures:
//! - Patterns are compiled exactly once
//! - Thread-safe access without locks after initialization
//! - Zero runtime overhead after first compilation

use crate::patterns::{pattern_to_regex, simple_wildcard_match};
use crate::types::Ecosystem;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum IndicatorContext {
    VersionControl,
    #[default]
    LanguageRoot,
    FrameworkRoot,
    BuildSystem,
    Configuration,
}

impl IndicatorContext {
    pub fn base_priority(&self) -> f32 {
        match self {
            IndicatorContext::VersionControl => 1.0,
            IndicatorContext::LanguageRoot => 0.9,
            IndicatorContext::FrameworkRoot => 0.8,
            IndicatorContext::BuildSystem => 0.7,
            IndicatorContext::Configuration => 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RootIndicator {
    pub pattern: String,
    pub weight: f32,
    #[serde(default)]
    pub context: IndicatorContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip)]
    compiled_patterns: OnceLock<HashMap<String, Option<Regex>>>,
}

impl PartialEq for Indicator {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.files == other.files
            && self.color == other.color
            && self.icon == other.icon
            && self.priority == other.priority
            && self.ecosystems == other.ecosystems
            && self.root_indicators == other.root_indicators
    }
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
            compiled_patterns: OnceLock::new(),
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
        Self {
            name,
            files,
            color,
            icon,
            priority,
            ecosystems,
            root_indicators: Vec::new(),
            compiled_patterns: OnceLock::new(),
        }
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
            compiled_patterns: OnceLock::new(),
        }
    }
    fn get_compiled_patterns(&self) -> &HashMap<String, Option<Regex>> {
        self.compiled_patterns.get_or_init(|| {
            let mut patterns = HashMap::new();
            for pattern in &self.files {
                let regex_opt = if pattern.contains('*') {
                    pattern_to_regex(pattern).and_then(|s| Regex::new(&s).ok())
                } else {
                    None
                };
                patterns.insert(pattern.clone(), regex_opt);
            }
            patterns
        })
    }
    pub fn matches_files(&self, files: &[String]) -> bool {
        let compiled_patterns = self.get_compiled_patterns();

        files.iter().any(|file| {
            self.files.iter().any(|pattern| {
                if let Some(Some(re)) = compiled_patterns.get(pattern) {
                    re.is_match(file)
                } else if pattern.contains('*') {
                    simple_wildcard_match(file, pattern)
                } else {
                    file == pattern
                }
            })
        })
    }
}
