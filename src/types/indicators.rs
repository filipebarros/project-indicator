//! Project-type (`Indicator`) and root-marker (`RootIndicator`) definitions.
//! Pattern matching lives in `crate::patterns`.

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
