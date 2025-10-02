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
//! - [`FrameworkDetector`] - Complete framework definition with detection rules
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
//! use project_indicator::types::{DetectionType, FrameworkDetector};
//!
//! // Define React framework detector
//! let react = FrameworkDetector {
//!     name: "React".to_string(),
//!     detection: DetectionType::NodeEcosystem {
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
//! let nextjs = FrameworkDetector {
//!     name: "Next.js".to_string(),
//!     detection: DetectionType::NodeEcosystem {
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
//! use project_indicator::types::{FrameworkMatch, FrameworkDetector, DetectionType};
//!
//! let framework = FrameworkDetector {
//!     name: "Django".to_string(),
//!     detection: DetectionType::PythonEcosystem {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DetectionType {
    NodeEcosystem { dependencies: Vec<String> },
    PythonEcosystem { dependencies: Vec<String> },
    RustEcosystem { dependencies: Vec<String> },
    GoEcosystem { modules: Vec<String> },
    PHPEcosystem { packages: Vec<String> },
    RubyEcosystem { gems: Vec<String> },
    JavaEcosystem { dependencies: Vec<String> },
    DotNetEcosystem { packages: Vec<String> },
    ScalaEcosystem { dependencies: Vec<String> },
    DartEcosystem { dependencies: Vec<String> },
    LuaEcosystem { packages: Vec<String> },
    KotlinEcosystem { dependencies: Vec<String> },
    SwiftEcosystem { dependencies: Vec<String> },
    ElixirEcosystem { dependencies: Vec<String> },
    FileExists { files: Vec<String> },
    ConfigFile { file: String, keys: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameworkDetector {
    pub name: String,
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
    pub framework: FrameworkDetector,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

impl FrameworkMatch {
    pub fn new(framework: FrameworkDetector, confidence: f32, evidence: Vec<String>) -> Self {
        Self {
            framework,
            confidence,
            evidence,
        }
    }
}
