//! Detection, display and file-metadata configuration. Serde derives back the
//! TOML config file; unknown keys are rejected (`deny_unknown_fields`).

use serde::{Deserialize, Serialize};

// Unknown keys fail loudly (deny_unknown_fields); OMITTED keys take the
// Default value, so a partial [detection]/[display] section is valid
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct DetectionConfig {
    pub max_upward_traversal: usize,
    pub require_vcs_root: bool,
    pub max_depth: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_upward_traversal: 10,
            require_vcs_root: false,
            max_depth: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
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
#[serde(deny_unknown_fields, default)]
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
