//! Framework definitions (`Framework`, `DetectionType`, `Ecosystem`) and
//! detected matches. A lower `priority` wins when picking the prompt icon.

use crate::types::RootIndicator;
use serde::{Deserialize, Serialize};

/// A package ecosystem: where dependency declarations live and how they are
/// parsed. Mirrors the ecosystem matcher functions 1:1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    Deno,
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
}

// NOTE: `detection` is an internally-tagged enum (`#[serde(tag = "type")]`),
// and serde cannot enforce deny_unknown_fields inside internally-tagged
// variant content — unknown keys under [frameworks.detection] are still
// silently ignored. Every other config struct rejects unknown keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
