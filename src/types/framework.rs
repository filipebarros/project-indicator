//! Framework detection types and ecosystem-specific detection methods.
//!
//! This module defines the types and mechanisms for detecting frameworks within
//! different programming language ecosystems (Node.js, Python, Rust, Go, PHP, etc.).
//!
//! # Overview
//!
//! Framework detection operates in two phases:
//!
//! 1. **Language Detection** - Identify the primary programming language
//! 2. **Framework Detection** - Check for framework-specific indicators within that language
//!
//! # Core Types
//!
//! - [`DetectionType`] - Enum defining different detection strategies per ecosystem
//! - [`Framework`] - Complete framework definition with detection rules
//! - [`FrameworkMatch`] - A detected framework with confidence score and evidence
//!
//! # Detection Strategies
//!
//! Different ecosystems use different detection approaches:
//!
//! ## Dependency-Based Detection
//!
//! Most ecosystems detect frameworks by checking dependency manifests:
//!
//! - **Node.js**: Check `package.json` dependencies for framework packages
//! - **Python**: Check `pyproject.toml`, `requirements.txt`, `Pipfile`
//! - **Rust**: Check `Cargo.toml` dependencies
//! - **Go**: Check `go.mod` module requirements
//! - **Ruby**: Check `Gemfile` for gems
//! - **PHP**: Check `composer.json` packages
//!
//! ## File-Based Detection
//!
//! Some frameworks are detected by the presence of specific files:
//!
//! - Next.js: `next.config.js`
//! - Django: `manage.py`, `settings.py`
//! - Rails: `config.ru`, `Gemfile`
//!
//! ## Config-Based Detection
//!
//! Advanced detection checks configuration file contents:
//!
//! - Poetry (Python): Check `pyproject.toml` for `[tool.poetry]`
//! - Tauri (Rust): Check `tauri.conf.json` exists
//!
//! # Example: Node.js Framework Detection
//!
//! ```rust
//! use project_indicator::types::{DetectionType, Ecosystem, Framework};
//!
//! // Define React framework detector
//! let react = Framework {
//!     name: "React".to_string(),
//!     ecosystems: vec![Ecosystem::Npm],
//!     detection: DetectionType::Dependencies {
//!         dependencies: vec!["react".to_string(), "react-dom".to_string()],
//!     },
//!     icon: Some("⚛️".to_string()),
//!     color: Some("#61DAFB".to_string()),
//!     priority: 1,
//!     files: vec![],
//!     root_indicators: vec![],
//! };
//!
//! // Next.js detector with both dependencies and files
//! let nextjs = Framework {
//!     name: "Next.js".to_string(),
//!     ecosystems: vec![Ecosystem::Npm],
//!     detection: DetectionType::Dependencies {
//!         dependencies: vec!["next".to_string()],
//!     },
//!     icon: Some("▲".to_string()),
//!     color: Some("#000000".to_string()),
//!     priority: 0, // Higher priority than React (lower number = higher priority)
//!     files: vec!["next.config.js".to_string()],
//!     root_indicators: vec![],
//! };
//! ```
//!
//! # Priority System
//!
//! Frameworks have a priority field (lower number = higher priority):
//!
//! - **0**: Meta-frameworks (Next.js, Nuxt.js, SvelteKit)
//! - **1**: Base frameworks (React, Vue, Svelte)
//! - **2+**: Libraries and utilities
//!
//! When multiple frameworks are detected, the highest priority one determines the icon/color.
//!
//! # Framework Match
//!
//! When a framework is detected, a [`FrameworkMatch`] is created containing:
//!
//! - The framework definition
//! - Confidence score (0.0 to 1.0)
//! - Evidence list (files/dependencies that matched)
//!
//! ```rust
//! use project_indicator::types::{DetectionType, Ecosystem, Framework, FrameworkMatch};
//!
//! let framework = Framework {
//!     name: "Django".to_string(),
//!     ecosystems: vec![Ecosystem::Pypi],
//!     detection: DetectionType::Dependencies {
//!         dependencies: vec!["django".to_string()],
//!     },
//!     icon: Some("🎸".to_string()),
//!     color: Some("#092E20".to_string()),
//!     priority: 1,
//!     files: vec![],
//!     root_indicators: vec![],
//! };
//!
//! let match_result = FrameworkMatch::new(
//!     framework,
//!     0.95,
//!     vec!["pyproject.toml".to_string()],
//! );
//!
//! assert_eq!(match_result.confidence, 0.95);
//! assert_eq!(match_result.framework.name, "Django");
//! ```

use crate::types::RootIndicator;
use serde::{Deserialize, Serialize};

/// A package ecosystem: where dependency declarations live and how they are
/// parsed. Mirrors the ecosystem matcher functions 1:1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    Pypi,
    Cargo,
    Go,
    Packagist,
    Rubygems,
    Maven,
    Gradle,
    Nuget,
    Sbt,
    Pub,
    Hex,
    Luarocks,
    Swiftpm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DetectionType {
    /// Match dependency names in the manifests of the framework's ecosystems
    Dependencies {
        dependencies: Vec<String>,
    },
    FileExists {
        files: Vec<String>,
    },
    ConfigFile {
        file: String,
        keys: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Framework {
    pub name: String,
    /// Ecosystems this framework belongs to. Scopes which indicators can
    /// surface it (intersection with the indicator's ecosystems) and which
    /// matchers run for `Dependencies` detection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ecosystems: Vec<Ecosystem>,
    pub detection: DetectionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub priority: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_indicators: Vec<RootIndicator>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameworkMatch {
    pub framework: Framework,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

impl FrameworkMatch {
    pub fn new(framework: Framework, confidence: f32, evidence: Vec<String>) -> Self {
        Self {
            framework,
            confidence,
            evidence,
        }
    }
}
